//! Trusts same-euid processes with access to the private control directory, not executable identity.
//! NAS root and Docker administrators are outside this boundary.
use std::{path::{Path,PathBuf},os::unix::{fs::{DirBuilderExt,FileTypeExt,MetadataExt,PermissionsExt},ffi::OsStrExt},time::Duration};
use tokio::{io::{AsyncRead,AsyncReadExt,AsyncWriteExt},net::{UnixListener,UnixStream}};
use serde_json::{Value,json};
use crate::{App,core_host::{CoreHost,Session}};
type R<T>=Result<T,String>;
// JSON escaping of the existing 1024-byte password limit needs more than the pipe's 32 bytes.
const REQUEST_LIMIT:usize=8192;
const RESPONSE_LIMIT:usize=65536; // devices can exceed a single session's 4096 bytes.
fn euid()->u32{unsafe{libc::geteuid()}}
fn socket_path(dir:&Path)->PathBuf{dir.join("control").join("core.sock")}
fn private_metadata(path:&Path,directory:bool)->R<std::fs::Metadata>{
 let m=std::fs::symlink_metadata(path).map_err(|e|e.to_string())?;let allowed=if directory{0o700}else{0o600};
 if m.uid()!=euid()||m.mode()&0o7777&!allowed!=0||if directory{!m.is_dir()}else{!m.file_type().is_socket()}{return Err("Control endpoint has an unsafe owner, type or permissions".into());}Ok(m)
}
fn verify_peer(stream:&UnixStream)->R<()>{
 // Both client and server call this before reading or writing any application data.
 if stream.peer_cred().map_err(|e|e.to_string())?.uid()!=euid(){return Err("Control peer uid does not match effective uid".into());}Ok(())
}
struct Control{listener:UnixListener,path:PathBuf,identity:(u64,u64)}
impl Control{
 fn bind(app:&App)->R<Self>{
  // An initialized App holds core.lock. Never unlink a socket before initialize succeeds.
  let path=socket_path(&app.state_dir);let parent=path.parent().unwrap();
  match std::fs::DirBuilder::new().mode(0o700).create(parent){Ok(())=>{},Err(e)if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e.to_string())}
  private_metadata(parent,true)?;
  match std::fs::symlink_metadata(&path){
   Ok(m)=>{if m.uid()!=euid()||!m.file_type().is_socket(){return Err("Refusing to remove a control socket with a different owner or type".into());}std::fs::remove_file(&path).map_err(|e|e.to_string())?;},
   Err(e)if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.to_string())
  }
  let listener=UnixListener::bind(&path).map_err(|e|e.to_string())?;
  let m=std::fs::symlink_metadata(&path).map_err(|e|e.to_string())?;let control=Self{listener,path,identity:(m.dev(),m.ino())};
  // The 0700 parent protects the bind-to-chmod interval, regardless of process umask.
  std::fs::set_permissions(&control.path,std::fs::Permissions::from_mode(0o600)).map_err(|e|e.to_string())?;
  private_metadata(&control.path,false)?;Ok(control)
 }
}
impl Drop for Control{fn drop(&mut self){if std::fs::symlink_metadata(&self.path).is_ok_and(|m|(m.dev(),m.ino())==self.identity&&m.uid()==euid()&&m.file_type().is_socket()){let _=std::fs::remove_file(&self.path);}}}
async fn read_line(stream:&mut (impl AsyncRead+Unpin),limit:usize)->R<Vec<u8>>{
 let mut bytes=Vec::new();loop{let b=stream.read_u8().await.map_err(|e|e.to_string())?;if b==b'\n'{return Ok(bytes);}
  if bytes.len()==limit{return Err("Control message exceeded limit".into());}bytes.push(b);}
}
pub async fn request(dir:&Path,action:&str)->R<Value>{
 if action.len()>REQUEST_LIMIT||action.contains(['\n','\r']){return Err("Invalid control request".into());}
 let path=socket_path(dir);private_metadata(path.parent().unwrap(),true)?;private_metadata(&path,false)?;
 let mut stream=tokio::time::timeout(Duration::from_secs(4),UnixStream::connect(&path)).await.map_err(|_|"Control connection timed out")?.map_err(|e|e.to_string())?;
 verify_peer(&stream)?;
 let exchange=async{
  stream.write_all(format!("{action}\n").as_bytes()).await.map_err(|e|e.to_string())?;
  let bytes=read_line(&mut stream,RESPONSE_LIMIT).await?;stream.write_all(b"!").await.map_err(|e|e.to_string())?;
  // Do not echo malformed response bodies: they may contain credentials.
  let value:Value=serde_json::from_slice(&bytes).map_err(|_|"Invalid control response")?;
  if let Some(error)=value["error"].as_str(){return Err(error.into());}Ok(value)
 };
 tokio::time::timeout(Duration::from_secs(5),exchange).await.map_err(|_|"Local Core did not respond".to_string())?
}
async fn owner_command(app:&App,session:&Session,action:&[u8])->R<Value>{
 if action==b"session"{return serde_json::to_value(session).map_err(|e|e.to_string());}
 let command:crate::owner_cli::OwnerCommand=serde_json::from_slice(action).map_err(|_|"Invalid owner command")?;
 crate::owner_cli::execute(app,command).await
}
async fn handle(mut stream:UnixStream,app:&App,session:&Session)->R<()>{
 verify_peer(&stream)?; // On mismatch close silently: not even an error response.
 let result=match tokio::time::timeout(Duration::from_secs(3),read_line(&mut stream,REQUEST_LIMIT)).await{
  Ok(Ok(action))=>owner_command(app,session,&action).await,
  Ok(Err(e))=>Err(e),Err(_)=>Err("Control request timed out".into())
 };
 let mut payload=match result{Ok(v)=>v,Err(e)=>json!({"error":e})}.to_string();
 if payload.len()>RESPONSE_LIMIT{payload=json!({"error":"Control response exceeded limit"}).to_string();}payload.push('\n');
 tokio::time::timeout(Duration::from_secs(3),stream.write_all(payload.as_bytes())).await.map_err(|_|"Control write timed out")?.map_err(|e|e.to_string())?;
 let _=tokio::time::timeout(Duration::from_secs(3),stream.read_u8()).await;Ok(())
}
pub async fn run(dir:PathBuf,device_dir:PathBuf,port:u16)->R<()>{
 if port==0{return Err("Loopback port must be between 1 and 65535".into());}
 check_data_filesystem(&dir)?;let _=crate::diagnostics::init(&dir);
 let mut terminate=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|e|e.to_string())?;
 let mut interrupt=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).map_err(|e|e.to_string())?;
 let (host,listener)=CoreHost::initialize(dir,&device_dir,port).await?;
 let control=Control::bind(&host.app)?;let app=host.app.clone();let session=host.session.clone();let core=host.start(listener)?;
 let result=loop{tokio::select!{biased;
  _=terminate.recv()=>break Ok(()),_=interrupt.recv()=>break Ok(()),
  accepted=control.listener.accept()=>match accepted{
   Ok((stream,_))=>{let _=handle(stream,&app,&session).await;},
   Err(e)=>break Err(e.to_string())
  }
 }};
 drop(control);core.pause_for_stop()?;core.shutdown().await?;result
}
#[cfg(any(target_os="linux",target_os="android",test))]
fn rejected_magic(kind:u32)->bool{matches!(kind,0x6969|0xff534d42|0xfe534d42|0x517b|0x65735546)} // NFS, CIFS, SMB2, SMB, FUSE
#[cfg(any(target_os="macos",test))]
fn rejected_name(kind:&str)->bool{let kind=kind.to_ascii_lowercase();matches!(kind.as_str(),"nfs"|"smbfs"|"cifs"|"smb"|"smb2")||kind.contains("fuse")}
pub fn check_data_filesystem(dir:&Path)->R<()>{
 std::fs::create_dir_all(dir).map_err(|e|e.to_string())?;
 let path=std::ffi::CString::new(dir.as_os_str().as_bytes()).map_err(|_|"Invalid data directory")?;
 #[cfg(any(target_os="linux",target_os="android",target_os="macos"))]{
  let mut info=std::mem::MaybeUninit::<libc::statfs>::uninit();
  if unsafe{libc::statfs(path.as_ptr(),info.as_mut_ptr())}!=0{return Err(std::io::Error::last_os_error().to_string());}
  let info=unsafe{info.assume_init()};
  #[cfg(any(target_os="linux",target_os="android"))]let rejected=rejected_magic(info.f_type as u32);
  #[cfg(target_os="macos")]let rejected=rejected_name(&unsafe{std::ffi::CStr::from_ptr(info.f_fstypename.as_ptr())}.to_string_lossy());
  if rejected{return Err("Core data directory must be on a local filesystem; NFS, CIFS/SMB and FUSE cannot safely host the collection database and process lock".into());}Ok(())
 }
 #[cfg(not(any(target_os="linux",target_os="android",target_os="macos")))]{let _=path;Err("Data filesystem validation is not supported on this Unix platform".into())}
}

#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->(tempfile::TempDir,App,Session){
  let t=tempfile::tempdir_in("/tmp").unwrap();let app=crate::initialize(t.path().join("d")).unwrap();
  let session=Session{library_id:"library".into(),device_id:app.device.id.clone(),api_min:1,api_max:1,url:"http://127.0.0.1:14800".into(),token:app.token.clone(),instance_id:app.instance_id.clone(),pid:std::process::id()};(t,app,session)
 }
 #[tokio::test]async fn same_uid_session_and_private_permissions(){
  let (_t,app,session)=fixture();let control=Control::bind(&app).unwrap();
  assert_eq!(std::fs::metadata(control.path.parent().unwrap()).unwrap().mode()&0o777,0o700);assert_eq!(std::fs::metadata(&control.path).unwrap().mode()&0o777,0o600);
  let client=request(&app.state_dir,"session");let server=async{let (stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();};
  let (value,())=tokio::join!(client,server);assert_eq!(value.unwrap(),serde_json::to_value(&session).unwrap());
 }
 #[tokio::test]async fn client_rejects_wide_directory_or_socket_before_connect(){
  let (_t,app,_)=fixture();let control=Control::bind(&app).unwrap();
  for (path,mode) in [(control.path.parent().unwrap(),0o711),(control.path.as_path(),0o644)]{
   std::fs::set_permissions(path,std::fs::Permissions::from_mode(mode)).unwrap();assert!(request(&app.state_dir,"session").await.unwrap_err().contains("unsafe"));
   assert!(tokio::time::timeout(Duration::from_millis(30),control.listener.accept()).await.is_err());
   std::fs::set_permissions(path,std::fs::Permissions::from_mode(if mode==0o711{0o700}else{0o600})).unwrap();
  }
 }
 #[tokio::test]async fn stale_socket_rebuilt_only_after_lock_is_acquired(){
  let (_t,app,session)=fixture();let path=socket_path(&app.state_dir);std::fs::DirBuilder::new().mode(0o700).create(path.parent().unwrap()).unwrap();
  let stale=UnixListener::bind(&path).unwrap();drop(stale);let old=std::fs::symlink_metadata(&path).unwrap();
  // This is also the startup order in run: initialize must succeed before Control::bind.
  let blocked=crate::initialize(app.state_dir.clone()).and_then(|other|Control::bind(&other));assert!(blocked.is_err());
  assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(),old.ino());
  let dir=app.state_dir.clone();drop(app);let app=crate::initialize(dir).unwrap();let control=Control::bind(&app).unwrap();
  let (value,())=tokio::join!(request(&app.state_dir,"session"),async{let(stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();});assert!(value.is_ok());
  drop(control);assert!(!path.exists());
 }
 #[tokio::test]async fn refuses_symlinks_and_non_socket_stale_paths(){
  let (_t,app,_)=fixture();let path=socket_path(&app.state_dir);std::fs::DirBuilder::new().mode(0o700).create(path.parent().unwrap()).unwrap();std::fs::write(&path,b"keep").unwrap();
  assert!(Control::bind(&app).is_err());assert_eq!(std::fs::read(&path).unwrap(),b"keep");std::fs::remove_file(&path).unwrap();
  std::os::unix::fs::symlink(app.state_dir.join("core.lock"),&path).unwrap();assert!(Control::bind(&app).is_err());assert!(request(&app.state_dir,"session").await.is_err());
 }
 #[tokio::test]async fn bounded_protocol_rejects_oversized_and_malformed_requests(){
  let (_t,app,session)=fixture();let control=Control::bind(&app).unwrap();
  for action in ["x".repeat(REQUEST_LIMIT+1),"{\"password\":\"do-not-echo-this-secret\"}".into()]{
   let client=async{let mut stream=UnixStream::connect(&control.path).await.unwrap();stream.write_all(format!("{action}\n").as_bytes()).await.unwrap();let bytes=read_line(&mut stream,RESPONSE_LIMIT).await.unwrap();let _=stream.write_all(b"!").await;serde_json::from_slice::<Value>(&bytes).unwrap()};
   let (value,())=tokio::join!(client,async{let(stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();});assert!(value["error"].is_string());assert!(!value.to_string().contains("do-not-echo"));
  }
  assert!(request(&app.state_dir,"session\nignored").await.is_err());
 }
 #[test]fn injected_filesystem_types_reject_network_and_fuse(){
  for kind in [0x6969,0xff534d42,0xfe534d42,0x517b,0x65735546]{assert!(rejected_magic(kind));}for kind in [0xef53,0x9123683e,0x58465342,0x794c7630]{assert!(!rejected_magic(kind));}
  for name in ["nfs","smbfs","cifs","smb2","osxfuse","macfuse","fusefs"]{assert!(rejected_name(name));}for name in ["apfs","hfs","ext4"]{assert!(!rejected_name(name));}
 }
 // A different uid needs another OS user; both peers call verify_peer before all I/O.
}

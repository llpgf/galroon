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
fn socket_path(dir:&Path)->PathBuf{dir.join("core.sock")}
fn private_metadata(path:&Path,directory:bool)->R<std::fs::Metadata>{
 let m=std::fs::symlink_metadata(path).map_err(|e|format!("Control endpoint {}: {e}",path.display()))?;let allowed=if directory{0o700}else{0o600};
 if m.uid()!=euid()||m.mode()&0o7777&!allowed!=0||if directory{!m.is_dir()}else{!m.file_type().is_socket()}{return Err(format!("Control endpoint {} has an unsafe owner, type or permissions; on a Synology/ACL share, use --control-dir with a non-ACL location (for example a tmpfs)",path.display()));}Ok(m)
}
fn verify_peer(stream:&UnixStream)->R<()>{
 // Both client and server call this before reading or writing any application data.
 if stream.peer_cred().map_err(|e|e.to_string())?.uid()!=euid(){return Err("Control peer uid does not match effective uid".into());}Ok(())
}
struct Control{listener:UnixListener,path:PathBuf,identity:(u64,u64)}
impl Control{
 fn bind(_app:&App,dir:&Path)->R<Self>{
  // An initialized App holds core.lock. Never unlink a socket before initialize succeeds.
  let path=socket_path(dir);let parent=path.parent().unwrap();
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
pub async fn request(dir:&Path,action:&str)->R<Value>{request_with_timeout(dir,action,Duration::from_secs(5)).await}
pub async fn request_with_timeout(dir:&Path,action:&str,timeout:Duration)->R<Value>{
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
 tokio::time::timeout(timeout,exchange).await.map_err(|_|"Local Core did not respond".to_string())?
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
fn rollback_relaunch(data_root:&Path,device_dir:&Path,req:&crate::backup::RelaunchRequest,reason:&str){
 crate::diagnostics::record("error","core.relaunch.rollback",&format!("Relaunch into {} failed ({reason}); rolling back to {}",req.new_collection.display(),req.previous_collection.display()),None);
 if let Err(e)=crate::active_collection::rollback(data_root,&req.previous_collection){crate::diagnostics::record("error","core.relaunch.rollback",&format!("Failed to roll back active-library.json: {e}"),None);}
 if !req.installed_identity.is_empty()||req.replaced_identity.is_some(){
  if let Err(e)=crate::backup::rollback_identity(device_dir,&req.installed_identity,req.replaced_identity.as_deref()){crate::diagnostics::record("error","core.relaunch.rollback",&format!("Failed to roll back device identity: {e}"),None);}
 }
}
pub async fn run(dir:PathBuf,device_dir:PathBuf,control_dir:PathBuf,port:u16)->R<()>{
 if port==0{return Err("Loopback port must be between 1 and 65535".into());}
 let mut terminate=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|e|e.to_string())?;
 let mut interrupt=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).map_err(|e|e.to_string())?;
 run_with_shutdown(dir,device_dir,control_dir,port,async move{tokio::select!{_=terminate.recv()=>{},_=interrupt.recv()=>{}}}).await
}
pub(crate) async fn run_with_shutdown(dir:PathBuf,device_dir:PathBuf,control_dir:PathBuf,port:u16,shutdown:impl std::future::Future<Output=()>+Send)->R<()>{
 run_loop(dir,device_dir,control_dir,port,shutdown,None).await
}
async fn run_loop(dir:PathBuf,device_dir:PathBuf,control_dir:PathBuf,port:u16,shutdown:impl std::future::Future<Output=()>+Send,fail_relaunch_once:Option<std::sync::Arc<std::sync::atomic::AtomicBool>>)->R<()>{
 check_data_filesystem(&dir)?;let _=crate::diagnostics::init(&dir);
 let data_root=dir;let mut pending_rollback:Option<crate::backup::RelaunchRequest>=None;
 tokio::pin!(shutdown);
 loop{
  let active=match crate::active_collection::resolve(&data_root){
   Ok(path)=>path,
   Err(err)=>{if let Some(req)=pending_rollback.take(){rollback_relaunch(&data_root,&device_dir,&req,&err);continue;}return Err(err);}
  };
  let started=async{
   if pending_rollback.is_some()&&fail_relaunch_once.as_ref().is_some_and(|f|f.swap(false,std::sync::atomic::Ordering::SeqCst)){
    return Err("Injected relaunch startup failure".into());
   }
   let(mut host,listener)=CoreHost::initialize(active.clone(),&device_dir,port).await?;
   host.app.data_root=data_root.clone();
   let(relaunch_tx,relaunch_rx)=tokio::sync::oneshot::channel();
   *host.app.relaunch.lock().map_err(|e|e.to_string())?=Some(relaunch_tx);
   let control=Control::bind(&host.app,&control_dir)?;
   let app=host.app.clone();let session=host.session.clone();
   let core=host.start(listener)?;
   Ok::<_,String>((core,control,app,session,relaunch_rx))
  }.await;
  let(core,control,app,session,mut relaunch_rx)=match started{
   Ok(v)=>{let _=pending_rollback.take();v}
   Err(err)=>{if let Some(req)=pending_rollback.take(){rollback_relaunch(&data_root,&device_dir,&req,&err);continue;}return Err(err);}
  };
  enum Next{Stop(R<()>),Relaunch(crate::backup::RelaunchRequest)}
  let next=loop{tokio::select!{biased;
   _=&mut shutdown=>break Next::Stop(Ok(())),
   res=&mut relaunch_rx=>match res{Ok(req)=>break Next::Relaunch(req),Err(_)=>break Next::Stop(Ok(()))},
   accepted=control.listener.accept()=>match accepted{
    Ok((stream,_))=>{let _=handle(stream,&app,&session).await;},
    Err(e)=>break Next::Stop(Err(e.to_string()))
   }
  }};
  drop(control);drop(app);
  let stop_res=core.pause_for_stop();let shut_res=core.shutdown().await;
  match next{
   Next::Stop(res)=>{stop_res?;shut_res?;return res;}
   Next::Relaunch(req)=>{
    if let Err(err)=stop_res.and(shut_res){crate::diagnostics::record("error","core.relaunch.shutdown",&err,None);}
    pending_rollback=Some(req);
   }
  }
 }
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
  let (_t,app,session)=fixture();let control=Control::bind(&app,&app.state_dir.join("control")).unwrap();
  assert_eq!(std::fs::metadata(control.path.parent().unwrap()).unwrap().mode()&0o777,0o700);assert_eq!(std::fs::metadata(&control.path).unwrap().mode()&0o777,0o600);
  let dir=app.state_dir.join("control");let client=request(&dir,"session");let server=async{let (stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();};
  let (value,())=tokio::join!(client,server);assert_eq!(value.unwrap(),serde_json::to_value(&session).unwrap());
 }
 #[tokio::test]async fn client_rejects_wide_directory_or_socket_before_connect(){
  let (_t,app,_)=fixture();let control=Control::bind(&app,&app.state_dir.join("control")).unwrap();
  for (path,mode) in [(control.path.parent().unwrap(),0o711),(control.path.as_path(),0o644)]{
   std::fs::set_permissions(path,std::fs::Permissions::from_mode(mode)).unwrap();assert!(request(&app.state_dir.join("control"),"session").await.unwrap_err().contains("unsafe"));
   assert!(tokio::time::timeout(Duration::from_millis(30),control.listener.accept()).await.is_err());
   std::fs::set_permissions(path,std::fs::Permissions::from_mode(if mode==0o711{0o700}else{0o600})).unwrap();
  }
 }
 #[tokio::test]async fn custom_control_directory_is_used_by_serve_and_owner(){
  use crate::owner_cli::{parse,Command};use std::ffi::OsString;
  let t=tempfile::tempdir_in("/tmp").unwrap();let data=t.path().join("data");let runtime=t.path().join("runtime");
  let Command::Serve{dir,device_dir,control_dir,..}=parse([OsString::from("serve"),"--data-dir".into(),data.clone().into(),"--control-dir".into(),runtime.clone().into()]).unwrap()else{panic!()};
  let (host,_listener)=CoreHost::initialize(dir,&device_dir,0).await.unwrap();let control=Control::bind(&host.app,&control_dir).unwrap();
  assert_eq!(control.path,runtime.join("core.sock"));assert!(!data.join("control").exists());
  assert_eq!(std::fs::metadata(&runtime).unwrap().mode()&0o777,0o700);assert_eq!(std::fs::metadata(&control.path).unwrap().mode()&0o777,0o600);
  // An unrelated owner data directory must not affect the explicit socket location.
  let owner=parse([OsString::from("owner"),"session".into(),"--data-dir".into(),t.path().join("other").into(),"--control-dir".into(),runtime.into()]).unwrap();
  let exchange=async{tokio::join!(crate::owner_cli::run(owner),async{let(stream,_)=control.listener.accept().await.unwrap();handle(stream,&host.app,&host.session).await.unwrap();})};
  let (value,())=tokio::time::timeout(Duration::from_secs(5),exchange).await.unwrap();value.unwrap();
 }
 #[tokio::test]async fn rejects_wide_control_directory_with_path_and_acl_hint(){
  let t=tempfile::tempdir_in("/tmp").unwrap();let data=t.path().join("data");let runtime=t.path().join("runtime");std::fs::create_dir(&runtime).unwrap();
  std::fs::set_permissions(&runtime,std::fs::Permissions::from_mode(0o777)).unwrap();
  let app=crate::initialize(data).unwrap();let error=Control::bind(&app,&runtime).err().expect("Wide permissions must fail");
  for error in [error,request(&runtime,"session").await.unwrap_err()]{
   assert!(error.contains(&runtime.display().to_string()),"{error}");for expected in ["unsafe","Synology/ACL","--control-dir","non-ACL","tmpfs"]{assert!(error.contains(expected),"{error}");}
  }
  assert_eq!(std::fs::metadata(&runtime).unwrap().mode()&0o777,0o777);assert!(!runtime.join("core.sock").exists());
 }
 #[tokio::test]async fn stale_socket_rebuilt_only_after_lock_is_acquired(){
  for custom in [false,true]{
  let (t,app,session)=fixture();let control_dir=if custom{t.path().join("runtime")}else{app.state_dir.join("control")};let path=socket_path(&control_dir);std::fs::DirBuilder::new().mode(0o700).create(path.parent().unwrap()).unwrap();
  let stale=UnixListener::bind(&path).unwrap();drop(stale);let old=std::fs::symlink_metadata(&path).unwrap();
  // This is also the startup order in run: initialize must succeed before Control::bind.
  let blocked=crate::initialize(app.state_dir.clone()).and_then(|other|Control::bind(&other,&control_dir));assert!(blocked.is_err());
  assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(),old.ino());
  let dir=app.state_dir.clone();drop(app);let app=crate::initialize(dir).unwrap();let control=Control::bind(&app,&control_dir).unwrap();
  let (value,())=tokio::join!(request(&control_dir,"session"),async{let(stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();});assert!(value.is_ok());
  drop(control);assert!(!path.exists());
  }
 }
 #[tokio::test]async fn refuses_symlinks_and_non_socket_stale_paths(){
  let (_t,app,_)=fixture();let path=socket_path(&app.state_dir.join("control"));std::fs::DirBuilder::new().mode(0o700).create(path.parent().unwrap()).unwrap();std::fs::write(&path,b"keep").unwrap();
  assert!(Control::bind(&app,&app.state_dir.join("control")).is_err());assert_eq!(std::fs::read(&path).unwrap(),b"keep");std::fs::remove_file(&path).unwrap();
  std::os::unix::fs::symlink(app.state_dir.join("core.lock"),&path).unwrap();assert!(Control::bind(&app,&app.state_dir.join("control")).is_err());assert!(request(&app.state_dir.join("control"),"session").await.is_err());
 }
 #[tokio::test]async fn bounded_protocol_rejects_oversized_and_malformed_requests(){
  let (_t,app,session)=fixture();let control=Control::bind(&app,&app.state_dir.join("control")).unwrap();
  for action in ["x".repeat(REQUEST_LIMIT+1),"{\"password\":\"do-not-echo-this-secret\"}".into()]{
   let client=async{let mut stream=UnixStream::connect(&control.path).await.unwrap();stream.write_all(format!("{action}\n").as_bytes()).await.unwrap();let bytes=read_line(&mut stream,RESPONSE_LIMIT).await.unwrap();let _=stream.write_all(b"!").await;serde_json::from_slice::<Value>(&bytes).unwrap()};
   let (value,())=tokio::join!(client,async{let(stream,_)=control.listener.accept().await.unwrap();handle(stream,&app,&session).await.unwrap();});assert!(value["error"].is_string());assert!(!value.to_string().contains("do-not-echo"));
  }
  assert!(request(&app.state_dir.join("control"),"session\nignored").await.is_err());
 }
 #[test]fn injected_filesystem_types_reject_network_and_fuse(){
  for kind in [0x6969,0xff534d42,0xfe534d42,0x517b,0x65735546]{assert!(rejected_magic(kind));}for kind in [0xef53,0x9123683e,0x58465342,0x794c7630]{assert!(!rejected_magic(kind));}
  for name in ["nfs","smbfs","cifs","smb2","osxfuse","macfuse","fusefs"]{assert!(rejected_name(name));}for name in ["apfs","hfs","ext4"]{assert!(!rejected_name(name));}
 }
 async fn wait_session(control_dir:&Path,previous_instance:Option<&str>)->Session{
  let deadline=tokio::time::Instant::now()+Duration::from_secs(10);
  loop{
   if let Ok(v)=request(control_dir,"session").await{
    if let Ok(s)=serde_json::from_value::<Session>(v){if previous_instance.is_none_or(|old|s.instance_id!=old){return s;}}
   }
   assert!(tokio::time::Instant::now()<deadline,"Timed out waiting for Core session");
   tokio::time::sleep(Duration::from_millis(50)).await;
  }
 }
 #[tokio::test]async fn first_run_full_restore_relaunches_in_process_preserving_spki_and_sessions(){
  let t=tempfile::tempdir_in("/tmp").unwrap();
  let src=crate::initialize(t.path().join("src")).unwrap();
  let password="Generated full relaunch owner password";
  crate::access::setup(&src.db,password,false).unwrap();
  let pair=crate::owner_cli::execute(&src,crate::owner_cli::OwnerCommand::Pair{name:"Paired desktop".into(),role:"desktop".into()}).await.unwrap();
  let redeemed=crate::access::pair(axum::extract::State(src.clone()),axum::Json(json!({"code":pair["code"]}))).await.map_err(|e|e.0).unwrap();
  let redeemed:Value=serde_json::from_slice(&axum::body::to_bytes(redeemed.into_body(),8192).await.unwrap()).unwrap();
  let desktop_token=redeemed["token"].as_str().unwrap().to_owned();
  let net=crate::network::NetworkSettings::for_test("192.168.50.154".parse().unwrap(),14830);
  let tls=crate::lan_tls::LanTls::load_or_generate(&src.device_dir,&net).unwrap();
  let expected_spki=crate::lan_tls::format_spki_fingerprint(&crate::lan_tls::spki_fingerprint(&tls.cert_der));
  std::fs::write(src.device_dir.join("network.json"),serde_json::to_vec(&net).unwrap()).unwrap();
  {let c=src.db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w1','Restored Game','Restored Game'); INSERT INTO roots(id,path,label) VALUES('r1','/tmp/source','Source'); INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('j1','scan','running','{}',1,1);").unwrap();crate::watching::ensure(&c).unwrap();c.execute("UPDATE source_watch SET enabled=1 WHERE root_id='r1'",[]).unwrap();}
  let backups=t.path().join("backups");std::fs::create_dir(&backups).unwrap();
  let bk=crate::backup::export_with(&src.db,&backups,Some(&src.device_dir),crate::backup::Options{mode:crate::backup::Mode::Full,origin:crate::backup::Origin::Scheduled,artwork:true}).unwrap();
  let manifest=crate::backup::inspect(&bk).unwrap();let src_device_id=src.device.id.clone();drop(src);

  // Pick a free fixed loopback port to prove in-process relaunch rebinds the exact same port.
  let probe=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let port=probe.local_addr().unwrap().port();drop(probe);
  let data=t.path().join("data");let control=t.path().join("control");
  let(stop_tx,stop_rx)=tokio::sync::oneshot::channel::<()>();
  let server=tokio::spawn(run_with_shutdown(data.clone(),data.clone(),control.clone(),port,async{let _=stop_rx.await;}));
  let before=wait_session(&control,None).await;assert_ne!(before.device_id,src_device_id);
  let client=reqwest::Client::new();
  let resp=client.post(format!("{}/api/setup/restore",before.url)).bearer_auth(&before.token).json(&json!({"path":bk,"sha256":manifest.sha256,"original_offline":true})).send().await.unwrap();
  assert_eq!(resp.status(),202);let body:Value=resp.json().await.unwrap();assert_eq!(body["status"],"relaunching");assert_eq!(body["lan_disabled"],true);

  let after=wait_session(&control,Some(&before.instance_id)).await;
  assert_eq!(after.url,before.url);assert_ne!(after.token,before.token);assert_eq!(after.device_id,src_device_id);
  let me:Value=client.get(format!("{}/api/access/me",after.url)).bearer_auth(&desktop_token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(me["role"],"desktop");
  let login=client.post(format!("{}/api/access/login",after.url)).header("Origin",&after.url).json(&json!({"password":password})).send().await.unwrap();
  assert_eq!(login.status(),200);
  let works:Value=client.get(format!("{}/api/works",after.url)).bearer_auth(&after.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(works.as_array().unwrap().len(),1);assert_eq!(works[0]["title"],"Restored Game");
  let net_show:Value=client.get(format!("{}/api/access/network",after.url)).bearer_auth(&after.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(net_show["spki_fingerprint"],expected_spki);assert_eq!(net_show["settings"]["enabled"],false);
  let watch:Value=client.get(format!("{}/api/roots/r1/watch",after.url)).bearer_auth(&after.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(watch["enabled"],false);
  let job:Value=client.get(format!("{}/api/jobs/j1",after.url)).bearer_auth(&after.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(job["state"],"interrupted");

  let _=stop_tx.send(());tokio::time::timeout(Duration::from_secs(5),server).await.unwrap().unwrap().unwrap();
 }
 #[tokio::test]async fn owner_cli_restore_relaunches_and_failed_relaunch_rolls_back_to_previous(){
  let t=tempfile::tempdir_in("/tmp").unwrap();
  let src=crate::initialize(t.path().join("src")).unwrap();
  src.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('w1','CLI Game','CLI Game')",[]).unwrap();
  let backups=t.path().join("backups");std::fs::create_dir(&backups).unwrap();
  let bk=crate::backup::export(&src.db,&backups).unwrap();drop(src);

  let data=t.path().join("data");let control=t.path().join("control");
  let(stop_tx,stop_rx)=tokio::sync::oneshot::channel::<()>();
  let server=tokio::spawn(run_with_shutdown(data.clone(),data.clone(),control.clone(),0,async{let _=stop_rx.await;}));
  let before=wait_session(&control,None).await;
  let cmd=crate::owner_cli::parse([std::ffi::OsString::from("owner"),"restore".into(),"--from".into(),bk.into(),"--control-dir".into(),control.clone().into()]).unwrap();
  crate::owner_cli::run(cmd).await.unwrap();
  let after=wait_session(&control,Some(&before.instance_id)).await;
  let client=reqwest::Client::new();
  let works:Value=client.get(format!("{}/api/works",after.url)).bearer_auth(&after.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  assert_eq!(works[0]["title"],"CLI Game");
  let _=stop_tx.send(());tokio::time::timeout(Duration::from_secs(5),server).await.unwrap().unwrap().unwrap();

  // Live rollback test: inject a startup failure on relaunch and verify unix_core rolls back collection & identity.
  let src2=crate::initialize(t.path().join("src2")).unwrap();
  let bk2=crate::backup::export_with(&src2.db,&backups,Some(&src2.device_dir),crate::backup::Options{mode:crate::backup::Mode::Full,origin:crate::backup::Origin::Manual,artwork:false}).unwrap();
  drop(src2);
  let data2=t.path().join("rollback-data");let control2=t.path().join("rollback-control");
  let fail_once=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
  let(stop2_tx,stop2_rx)=tokio::sync::oneshot::channel::<()>();
  let server2=tokio::spawn(run_loop(data2.clone(),data2.clone(),control2.clone(),0,async{let _=stop2_rx.await;},Some(fail_once)));
  let initial=wait_session(&control2,None).await;
  let cmd2=crate::owner_cli::parse([std::ffi::OsString::from("owner"),"restore".into(),"--from".into(),bk2.into(),"--confirm-original-offline".into(),"--control-dir".into(),control2.clone().into()]).unwrap();
  crate::owner_cli::run(cmd2).await.unwrap();
  let recovered=wait_session(&control2,Some(&initial.instance_id)).await;
  assert_eq!(recovered.device_id,initial.device_id);
  assert_eq!(recovered.library_id,initial.library_id);
  assert_eq!(crate::active_collection::resolve(&data2).unwrap(),data2.canonicalize().unwrap());
  assert_eq!(crate::context::device(&data2).unwrap().id,initial.device_id);
  let _=stop2_tx.send(());tokio::time::timeout(Duration::from_secs(5),server2).await.unwrap().unwrap().unwrap();
 }
 // A different uid needs another OS user; both peers call verify_peer before all I/O.
}

//! Per-user Windows control channel. Bearer credentials never go into argv or disk.
use std::{path::{Path,PathBuf},os::windows::{io::AsRawHandle},time::Duration};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::windows::named_pipe::{ServerOptions,ClientOptions}};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use windows_sys::Win32::{Foundation::{CloseHandle,LocalFree},Security::{GetTokenInformation,TokenUser,TOKEN_QUERY,TOKEN_USER,SECURITY_ATTRIBUTES},Security::Authorization::{ConvertSidToStringSidW,ConvertStringSecurityDescriptorToSecurityDescriptorW},System::{Threading::{GetCurrentProcess,OpenProcessToken,OpenProcess,QueryFullProcessImageNameW,PROCESS_QUERY_LIMITED_INFORMATION},Pipes::GetNamedPipeServerProcessId}};
type R<T>=Result<T,String>;
#[derive(Clone,Serialize,Deserialize)]pub struct Session{pub library_id:String,pub device_id:String,pub api_min:u32,pub api_max:u32,pub url:String,pub token:String,pub instance_id:String,pub pid:u32}
fn last_error()->String{std::io::Error::last_os_error().to_string()}
fn owner_sid()->R<String>{unsafe{
 let mut token=std::ptr::null_mut();if OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut token)==0{return Err(last_error());}
 let mut len=0;GetTokenInformation(token,TokenUser,std::ptr::null_mut(),0,&mut len);
 let mut buffer=vec![0usize;(len as usize+std::mem::size_of::<usize>()-1)/std::mem::size_of::<usize>()];
 if GetTokenInformation(token,TokenUser,buffer.as_mut_ptr().cast(),len,&mut len)==0{let e=last_error();CloseHandle(token);return Err(e);}CloseHandle(token);
 let user=&*buffer.as_ptr().cast::<TOKEN_USER>();let mut text=std::ptr::null_mut();if ConvertSidToStringSidW(user.User.Sid,&mut text)==0{return Err(last_error());}
 let mut n=0;while *text.add(n)!=0{n+=1;}let sid=String::from_utf16_lossy(std::slice::from_raw_parts(text,n));LocalFree(text.cast());Ok(sid)
}}
pub fn pipe_name(dir:&Path)->R<String>{let canonical=dir.canonicalize().map_err(|e|e.to_string())?;let key=format!("{}|{}",owner_sid()?,canonical.to_string_lossy().to_lowercase());Ok(format!(r"\\.\pipe\galroon-{}",hex::encode(Sha256::digest(key.as_bytes()))))}
fn secure_pipe(name:&str,first:bool)->R<tokio::net::windows::named_pipe::NamedPipeServer>{
 let sddl=format!("D:P(A;;GA;;;{})",owner_sid()?);let wide:Vec<u16>=sddl.encode_utf16().chain(Some(0)).collect();
 unsafe{let mut descriptor=std::ptr::null_mut();if ConvertStringSecurityDescriptorToSecurityDescriptorW(wide.as_ptr(),1,&mut descriptor,std::ptr::null_mut())==0{return Err(last_error());}
 let mut attrs=SECURITY_ATTRIBUTES{nLength:std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,lpSecurityDescriptor:descriptor,bInheritHandle:0};
 let result=ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true).create_with_security_attributes_raw(name,(&mut attrs as *mut SECURITY_ATTRIBUTES).cast());LocalFree(descriptor);result.map_err(|e|e.to_string())}
}
fn verify_server(pipe:&tokio::net::windows::named_pipe::NamedPipeClient,expected:&Path)->R<u32>{unsafe{
 let mut pid=0;if GetNamedPipeServerProcessId(pipe.as_raw_handle(),&mut pid)==0{return Err(last_error());}
 let process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid);if process.is_null(){return Err(last_error());}
 let mut name=vec![0u16;32768];let mut len=name.len()as u32;let ok=QueryFullProcessImageNameW(process,0,name.as_mut_ptr(),&mut len);let error=last_error();CloseHandle(process);if ok==0{return Err(error);}
 let actual=PathBuf::from(String::from_utf16_lossy(&name[..len as usize])).canonicalize().map_err(|e|e.to_string())?;let expected=expected.canonicalize().map_err(|e|e.to_string())?;
 if actual.as_os_str().to_string_lossy().to_lowercase()!=expected.as_os_str().to_string_lossy().to_lowercase(){return Err("The local control endpoint belongs to a different executable".into());}Ok(pid)
}}
pub async fn request(dir:&Path,expected_exe:&Path,action:&str)->R<serde_json::Value>{
 let name=pipe_name(dir)?;let deadline=tokio::time::Instant::now()+Duration::from_secs(4);
 let mut pipe=loop{match ClientOptions::new().open(&name){Ok(pipe)=>break pipe,Err(e)if matches!(e.raw_os_error(),Some(2|231))&&tokio::time::Instant::now()<deadline=>tokio::time::sleep(Duration::from_millis(100)).await,Err(e)=>return Err(e.to_string())}};
 let pid=verify_server(&pipe,expected_exe)?;
 let exchange=async{pipe.write_all(format!("{action}\n").as_bytes()).await.map_err(|e|e.to_string())?;let mut bytes=Vec::new();loop{let b=pipe.read_u8().await.map_err(|e|e.to_string())?;if b==b'\n'{break;}bytes.push(b);if bytes.len()>4096{return Err("Control response exceeded limit".into());}}pipe.write_all(b"!").await.map_err(|e|e.to_string())?;let v:serde_json::Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;if let Some(error)=v["error"].as_str(){return Err(error.into());}if action=="session"&&v["pid"].as_u64()!=Some(pid as u64){return Err("Core identity mismatch".into());}Ok(v)};
 tokio::time::timeout(Duration::from_secs(5),exchange).await.map_err(|_|"Local Core did not respond".to_string())?
}
pub async fn connect_or_start(dir:PathBuf,exe:PathBuf)->R<Session>{let device_dir=dir.clone();connect_or_start_with_device(dir,exe,device_dir).await}
pub async fn connect_or_start_with_device(dir:PathBuf,exe:PathBuf,device_dir:PathBuf)->R<Session>{
 std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 if let Ok(v)=request(&dir,&exe,"session").await{return serde_json::from_value(v).map_err(|e|e.to_string());}
 // Core itself holds the data-directory lock; racing launchers cannot create two writers.
 use std::os::windows::process::CommandExt;
 let mut child=std::process::Command::new(&exe).arg("--data-dir").arg(&dir).arg("--device-dir").arg(&device_dir).creation_flags(0x08000000|0x00000200).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e|e.to_string())?;
 let mut failure=String::new();for _ in 0..3{match request(&dir,&exe,"session").await{Ok(v)=>return serde_json::from_value(v).map_err(|e|e.to_string()),Err(e)=>{eprintln!("Core connection attempt failed: {e}");failure=e}}if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{return Err(format!("Core startup exited ({status}): {failure}"));}}
 Err(format!("Could not connect to local Core: {failure}"))
}
pub async fn run(dir:PathBuf)->R<()>{let device_dir=dir.clone();run_with_device(dir,device_dir).await}
pub async fn run_with_device(dir:PathBuf,device_dir:PathBuf)->R<()>{
 let app=crate::initialize_with_device(dir,&device_dir)?;
 let library=crate::context::library(&*app.db.lock().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e|e.to_string())?;
 let session=Session{library_id:library.id,device_id:app.device.id.clone(),api_min:1,api_max:1,url:format!("http://127.0.0.1:{}",listener.local_addr().map_err(|e|e.to_string())?.port()),token:app.token.clone(),instance_id:app.instance_id.clone(),pid:std::process::id()};
 let name=pipe_name(&app.state_dir)?;let mut next_pipe=secure_pipe(&name,true)?;let db=app.db.clone();let _lifetime_lock=app.process_lock.clone();let (stop_tx,stop_rx)=tokio::sync::oneshot::channel::<()>();
 let watching=crate::watching::start(app.clone())?;let _matching=crate::automatch::start(app.db.clone());let mut _smart=crate::smart_updates::start(app.db.clone());let mutation_gate=app.mutation_lock.clone();
 let http=tokio::spawn(async move{axum::serve(listener,crate::router(app)).with_graceful_shutdown(async{let _=stop_rx.await;}).await.map_err(|e|e.to_string())});
 loop{next_pipe.connect().await.map_err(|e|e.to_string())?;let mut pipe=next_pipe;next_pipe=secure_pipe(&name,false)?;
  let read=async{let mut bytes=Vec::new();loop{let b=pipe.read_u8().await?;if b==b'\n'{break;}bytes.push(b);if bytes.len()>32{return Err(std::io::Error::other("Invalid request"));}}Ok(String::from_utf8_lossy(&bytes).to_string())};
  let action=tokio::time::timeout(Duration::from_secs(3),read).await;let mut stopping=false;
  let response=match action{Ok(Ok(a))if a=="session"=>serde_json::to_value(&session).unwrap(),Ok(Ok(a))if a=="stop"=>{
   _smart.request_stop();let _gate=mutation_gate.lock().map_err(|e|e.to_string())?;let c=db.lock().map_err(|e|e.to_string())?;let active:i64=c.query_row("SELECT (SELECT count(*) FROM jobs WHERE state IN ('queued','running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
   if active>0{_smart=crate::smart_updates::start(db.clone());serde_json::json!({"error":"Pause running tasks before stopping Core"})}else{watching.request_stop();_matching.request_stop();_smart.request_stop();stopping=true;serde_json::json!({"stopped":true})}
  },Ok(Ok(a))=>serde_json::json!({"error":format!("Unknown control command ({} bytes)",a.len())}),Ok(Err(e))=>serde_json::json!({"error":format!("Control read failed: {e}")}),Err(_)=>serde_json::json!({"error":"Control request timed out"})};
  let payload=format!("{response}\n");let _=tokio::time::timeout(Duration::from_secs(3),pipe.write_all(payload.as_bytes())).await;
  let _=tokio::time::timeout(Duration::from_secs(3),pipe.read_u8()).await;
  if stopping{let _=stop_tx.send(());http.await.map_err(|e|e.to_string())??;
   // Drain any request that passed authentication just before HTTP shutdown.
   loop{let active={let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET state='pausing' WHERE state='running'",[]).map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET state='interrupted' WHERE state='queued'",[]).map_err(|e|e.to_string())?;c.query_row("SELECT (SELECT count(*) FROM jobs WHERE state IN ('running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?};if active==0{break;}tokio::time::sleep(Duration::from_millis(100)).await;}
   let checkpoint_path={let c=db.lock().map_err(|e|e.to_string())?;c.path().map(PathBuf::from)};
   if let Some(path)=checkpoint_path {tokio::task::spawn_blocking(move||crate::checkpoint::drain(&path)).await.map_err(|e|e.to_string())?;}
   return Ok(());
  }
  let _=pipe.disconnect();
 }
}




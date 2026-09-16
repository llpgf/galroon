//! Bounded helper output and cooperative cancellation. Never logs passwords or helper output.
use std::{io::{Read,Write},process::{Command,Stdio,Output},sync::{Arc,atomic::{AtomicBool,Ordering}},thread,time::{Duration,Instant}};
type R<T>=Result<T,String>;
#[derive(Clone,Copy)]pub(crate) struct Limits{pub timeout:Duration,pub stdout:usize,pub stderr:usize}
pub(crate) const LIST_LIMITS:Limits=Limits{timeout:Duration::from_secs(120),stdout:32*1024*1024,stderr:256*1024};
fn reader(mut stream:impl Read+Send+'static,limit:usize,exceeded:Arc<AtomicBool>,failed:Arc<AtomicBool>)->thread::JoinHandle<R<Vec<u8>>>{thread::spawn(move||{
 let mut result=Vec::new();let mut buffer=[0u8;8192];loop{let count=match stream.read(&mut buffer){Ok(n)=>n,Err(_)=>{failed.store(true,Ordering::Release);return Err("Unable to read archive helper output".into());}};if count==0{return Ok(result);}if count>limit.saturating_sub(result.len()){exceeded.store(true,Ordering::Release);return Err("Archive helper output exceeds the supported limit".into());}result.extend_from_slice(&buffer[..count]);}
})}
pub(crate) fn run(command:&mut Command,password:Option<&str>,limits:Limits,mut checkpoint:impl FnMut()->R<()>)->R<Output>{
 checkpoint()?;if password.is_some_and(|p|p.len()>1024||p.contains(['\r','\n'])){return Err("Archive password must be one line of at most 1024 bytes".into());}
 command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());let mut child=command.spawn().map_err(|e|e.to_string())?;
 let exceeded=Arc::new(AtomicBool::new(false));let failed=Arc::new(AtomicBool::new(false));
 let stdout=reader(child.stdout.take().expect("Piped stdout"),limits.stdout,exceeded.clone(),failed.clone());let stderr=reader(child.stderr.take().expect("Piped stderr"),limits.stderr,exceeded.clone(),failed.clone());
 let result:R<std::process::ExitStatus>=(||{
  if let Some(mut input)=child.stdin.take(){if let Some(password)=password{input.write_all(password.as_bytes()).and_then(|_|input.write_all(b"\n")).map_err(|_|"Unable to pass archive password to helper")?;}}
  let started=Instant::now();loop{
   checkpoint()?;
   if exceeded.load(Ordering::Acquire){return Err("Archive helper output exceeds the supported limit".into());}
   if failed.load(Ordering::Acquire){return Err("Unable to read archive helper output".into());}
   if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{return Ok(status);}
   if started.elapsed()>=limits.timeout{return Err("Archive listing timed out. No extracted files were published.".into());}
   thread::sleep(Duration::from_millis(50));
  }
 })();
 if result.is_err(){let _=child.kill();}let _=child.wait();
 let out=stdout.join().map_err(|_|"Archive output reader failed")?;let err=stderr.join().map_err(|_|"Archive error reader failed")?;
 // The child may exit in the same poll in which a stream reaches its limit.
 let status=result?;let stdout=out?;let stderr=err?;checkpoint()?;Ok(Output{status,stdout,stderr})
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture(mode:&str)->Command{let mut c=Command::new(std::env::current_exe().unwrap());c.args(["--exact","archive_process::tests::child_fixture","--nocapture"]).env("GALROON_GENERATED_ARCHIVE_CHILD",mode);#[cfg(windows)]{use std::os::windows::process::CommandExt;c.creation_flags(0x08000000);}c}
 #[test]fn child_fixture(){let Ok(mode)=std::env::var("GALROON_GENERATED_ARCHIVE_CHILD")else{return};match mode.as_str(){"sleep"=>thread::sleep(Duration::from_secs(15)),"stdout"=>{let _=std::io::stdout().write_all(&vec![b'x';128*1024]);thread::sleep(Duration::from_secs(15));},"stderr"=>{let _=std::io::stderr().write_all(&vec![b'x';128*1024]);thread::sleep(Duration::from_secs(15));},"password"=>{let mut value=String::new();std::io::stdin().read_to_string(&mut value).unwrap();assert_eq!(value,"generated-secret\n");println!("password received without echo");},_=>println!("generated result")}}
 #[test]fn bounded_capture_handles_success_and_private_stdin(){let limits=Limits{timeout:Duration::from_secs(5),stdout:8192,stderr:8192};let result=run(&mut fixture("success"),None,limits,||Ok(())).unwrap();assert!(result.status.success());assert!(String::from_utf8_lossy(&result.stdout).contains("generated result"));let result=run(&mut fixture("password"),Some("generated-secret"),limits,||Ok(())).unwrap();assert!(result.status.success());assert!(!String::from_utf8_lossy(&result.stdout).contains("generated-secret"));}
 #[test]fn acquire_pause_and_cancel_during_listing_keep_the_durable_terminal_state(){
  for (requested,expected)in [("pausing","paused"),("cancelling","cancelled")]{
   let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('listing','acquire','running','{}',0,0)",[]).unwrap();
   let writer=db.clone();let change=thread::spawn(move||{thread::sleep(Duration::from_millis(150));writer.lock().unwrap().execute("UPDATE jobs SET state=?1 WHERE id='listing'",[requested]).unwrap();});
   let started=Instant::now();let result=run(&mut fixture("sleep"),None,Limits{timeout:Duration::from_secs(5),stdout:4096,stderr:4096},||crate::acquire::control(&db,"listing"));change.join().unwrap();assert!(result.is_err());assert!(started.elapsed()<Duration::from_secs(4));assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id='listing'",[],|r|r.get::<_,String>(0)).unwrap(),expected);
  }
 }
 #[test]fn cancellation_timeout_and_each_stream_limit_reap_the_child_promptly(){
  let limits=Limits{timeout:Duration::from_secs(5),stdout:4096,stderr:4096};
  for mode in ["stdout","stderr"]{let start=Instant::now();assert!(run(&mut fixture(mode),None,limits,||Ok(())).unwrap_err().contains("limit"));assert!(start.elapsed()<Duration::from_secs(4));}
  let start=Instant::now();assert!(run(&mut fixture("sleep"),None,Limits{timeout:Duration::from_millis(150),..limits},||Ok(())).unwrap_err().contains("timed out"));assert!(start.elapsed()<Duration::from_secs(4));
  let start=Instant::now();let mut polls=0;assert!(run(&mut fixture("sleep"),None,limits,||{polls+=1;if polls>=4{Err("Generated task paused".into())}else{Ok(())}}).unwrap_err().contains("paused"));assert!(start.elapsed()<Duration::from_secs(4));
 }
}

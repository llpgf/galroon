//! Shared process lifetime; platform modules only own their control transport.
use std::{path::{Path,PathBuf},time::Duration};
use serde::{Serialize,Deserialize};
use crate::{App,Services};
type R<T>=Result<T,String>;
#[derive(Clone,Serialize,Deserialize)]pub struct Session{pub library_id:String,pub device_id:String,pub api_min:u32,pub api_max:u32,pub url:String,pub token:String,pub instance_id:String,pub pid:u32}
pub struct CoreHost{pub app:App,pub session:Session,lan:Vec<crate::Listener>}
pub struct RunningCore{app:App,services:Services,http:tokio::task::JoinHandle<R<()>>,stop:tokio::sync::oneshot::Sender<()>}
impl CoreHost{
 pub async fn initialize(dir:PathBuf,device_dir:&Path,port:u16)->R<(Self,tokio::net::TcpListener)>{
  let app=crate::initialize_with_device(dir,device_dir)?;
  let library=crate::context::library(&*app.db.lock().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  let listener=tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,port)).await.map_err(|e|if port==0{e.to_string()}else{format!("Cannot bind loopback port {port} (127.0.0.1): {e}")})?;
  let session=Session{library_id:library.id,device_id:app.device.id.clone(),api_min:1,api_max:1,url:format!("http://127.0.0.1:{}",listener.local_addr().map_err(|e|e.to_string())?.port()),token:app.token.clone(),instance_id:app.instance_id.clone(),pid:std::process::id()};
  let mut listeners=crate::configured_listeners(&app,listener).await?;let crate::Listener::Loopback(listener)=listeners.remove(0)else{unreachable!()};
  Ok((Self{app,session,lan:listeners},listener))
 }
 pub fn start(self,listener:tokio::net::TcpListener)->R<RunningCore>{
  let services=Services::start(self.app.clone())?;let (stop,stopped)=tokio::sync::oneshot::channel();let app=self.app.clone();
  let mut listeners=self.lan;listeners.insert(0,crate::Listener::Loopback(listener));
  let http=tokio::spawn(async move{crate::serve_tls_listeners(app,listeners,async{let _=stopped.await;}).await});
  Ok(RunningCore{app:self.app,services,http,stop})
 }
}
impl RunningCore{
 /// Windows stop remains conditional: restart smart updates when work is active.
 pub fn request_stop_if_idle(&mut self)->R<bool>{
  self.services.stop_smart_updates();let _gate=self.app.mutation_lock.lock().map_err(|e|e.to_string())?;let c=self.app.db.lock().map_err(|e|e.to_string())?;
  let active:i64=c.query_row("SELECT (SELECT count(*) FROM jobs WHERE state IN ('queued','running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  if active>0{self.services.restart_smart_updates(self.app.db.clone());Ok(false)}else{self.services.request_stop();Ok(true)}
 }
 /// Signals must finish shutdown even when work is active.
 pub fn pause_for_stop(&self)->R<()>{
  self.services.request_stop();let _gate=self.app.mutation_lock.lock().map_err(|e|e.to_string())?;pause_jobs(&self.app.db)?;Ok(())
 }
 pub async fn shutdown(self)->R<()>{
  let Self{app,services,http,stop}=self;
  let _=stop.send(());http.await.map_err(|e|e.to_string())??;
  let result=drain(&app.db).await;drop(services);result
 }
}
fn pause_jobs(db:&crate::db::Db)->R<i64>{
 let c=db.lock().map_err(|e|e.to_string())?;
 c.execute("UPDATE jobs SET state='pausing' WHERE state='running'",[]).map_err(|e|e.to_string())?;
 c.execute("UPDATE jobs SET state='interrupted' WHERE state='queued'",[]).map_err(|e|e.to_string())?;
 c.query_row("SELECT (SELECT count(*) FROM jobs WHERE state IN ('running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())
}
async fn drain(db:&crate::db::Db)->R<()>{
 // Catch requests that passed authentication just before HTTP shutdown as well.
 while pause_jobs(db)?>0{tokio::time::sleep(Duration::from_millis(100)).await;}
 let checkpoint_path={let c=db.lock().map_err(|e|e.to_string())?;c.path().map(PathBuf::from)};
 if let Some(path)=checkpoint_path{tokio::task::spawn_blocking(move||crate::checkpoint::drain(&path)).await.map_err(|e|e.to_string())?;}
 Ok(())
}

#[cfg(test)]mod tests{
 use super::*;
 fn job(app:&App,id:&str,state:&str){app.db.lock().unwrap().execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES(?1,'scan',?2,'{}',0,0)",rusqlite::params![id,state]).unwrap();}
 fn running(app:App)->RunningCore{
  let services=Services::start(app.clone()).unwrap();let (stop,stopped)=tokio::sync::oneshot::channel();
  let http=tokio::spawn(async move{stopped.await.unwrap();Ok(())});RunningCore{app,services,http,stop}
 }
 #[tokio::test]async fn signal_stop_pauses_jobs_drains_and_keeps_lock_through_checkpoint(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("data")).unwrap();let core=running(app.clone());
  job(&app,"running","running");job(&app,"queued","queued");job(&app,"paused","paused");
  core.pause_for_stop().unwrap();
  let state=|id:&str|app.db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[id],|r|r.get::<_,String>(0)).unwrap();
  assert_eq!(state("running"),"pausing");assert_eq!(state("queued"),"interrupted");assert_eq!(state("paused"),"paused");
  let dir=app.state_dir.clone();let db=app.db.clone();drop(app);
  let shutdown=tokio::spawn(core.shutdown());tokio::time::sleep(Duration::from_millis(150)).await;assert!(!shutdown.is_finished());
  assert!(crate::initialize(dir.clone()).is_err());
  {let c=db.lock().unwrap();let mut checkpoint=crate::checkpoint::LargeEdition::new(&c,None,1000).unwrap();c.execute("UPDATE jobs SET state='paused' WHERE id='running'",[]).unwrap();checkpoint.committed();}
  tokio::time::timeout(Duration::from_secs(5),shutdown).await.unwrap().unwrap().unwrap();
  drop(db);let reopened=crate::initialize(dir).unwrap();let c=reopened.db.lock().unwrap();
  assert_eq!(c.query_row("SELECT state FROM jobs WHERE id='queued'",[],|r|r.get::<_,String>(0)).unwrap(),"interrupted");
  assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
 }
 #[tokio::test]async fn conditional_stop_refuses_active_work_then_allows_idle(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("data")).unwrap();let mut core=running(app.clone());job(&app,"queued","queued");
  {let c=app.db.lock().unwrap();let definition=serde_json::json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('stop-fixture','Stop fixture','',1,?1)",[definition.to_string()]).unwrap();}
  assert!(!core.request_stop_if_idle().unwrap());assert!(!core.http.is_finished());
  assert_eq!(app.db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id='queued'",[],|r|r.get::<_,String>(0)).unwrap(),"queued");
  app.db.lock().unwrap().execute("UPDATE jobs SET state='interrupted'",[]).unwrap();
  // The replacement smart worker must actually publish after stop was refused.
  // This is a ceiling for slow CI under contention, not an expected duration.
  tokio::time::timeout(Duration::from_secs(30),async{loop{let count:i64=app.db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshots WHERE list_id='stop-fixture'",[],|r|r.get(0)).unwrap();if count>0{break;}tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();
  assert!(core.request_stop_if_idle().unwrap());core.shutdown().await.unwrap();
 }
 #[tokio::test]async fn occupied_fixed_port_fails_without_fallback(){
  let occupied=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let port=occupied.local_addr().unwrap().port();let t=tempfile::tempdir().unwrap();
  let error=CoreHost::initialize(t.path().join("data"),t.path(),port).await.err().expect("Occupied port must fail");assert!(error.contains(&port.to_string()),"{error}");assert!(error.contains("127.0.0.1"));
 }
}

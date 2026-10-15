//! Durable Core-owned downloads, enqueued in the same transaction as catalog changes.
use crate::{App,db::now};
use rusqlite::{Connection,params};
use serde_json::{Value,json};

pub fn migrate(c:&Connection,backfill:bool)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS collection_downloads(work_id TEXT PRIMARY KEY REFERENCES works(id) ON DELETE CASCADE,generation INTEGER NOT NULL DEFAULT 1,retry_at INTEGER NOT NULL DEFAULT 0,error TEXT NOT NULL DEFAULT '');
 CREATE TRIGGER IF NOT EXISTS collection_download_insert AFTER INSERT ON works WHEN NEW.merged_into IS NULL BEGIN INSERT INTO collection_downloads(work_id) VALUES(NEW.id); END;
 CREATE TRIGGER IF NOT EXISTS collection_download_update AFTER UPDATE OF vndb_id,cover,source_json,merged_into ON works WHEN NEW.merged_into IS NULL AND (NEW.vndb_id IS NOT OLD.vndb_id OR NEW.cover IS NOT OLD.cover OR NEW.source_json IS NOT OLD.source_json OR OLD.merged_into IS NOT NULL) BEGIN INSERT INTO collection_downloads(work_id) VALUES(NEW.id) ON CONFLICT(work_id) DO UPDATE SET generation=generation+1,retry_at=0,error=''; END;
 CREATE TRIGGER IF NOT EXISTS collection_download_merge AFTER UPDATE OF merged_into ON works WHEN NEW.merged_into IS NOT NULL BEGIN DELETE FROM collection_downloads WHERE work_id=NEW.id; END;
 CREATE INDEX IF NOT EXISTS collection_download_retry ON collection_downloads(retry_at);")?;
 if backfill{c.execute("INSERT OR IGNORE INTO collection_downloads(work_id) SELECT id FROM works WHERE merged_into IS NULL",[])?;}
 for action in ["INSERT","UPDATE","DELETE"]{let row=if action=="DELETE"{"OLD"}else{"NEW"};c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS collection_download_relation_{action} AFTER {action} ON relation_corrections BEGIN INSERT INTO collection_downloads(work_id) SELECT id FROM works WHERE merged_into IS NULL AND vndb_id={row}.vndb_id ON CONFLICT(work_id) DO UPDATE SET generation=generation+1,retry_at=0,error=''; END;"))?;}
 Ok(())
}
pub fn status(c:&Connection)->rusqlite::Result<Value>{c.query_row("SELECT count(*),coalesce(sum(error!=''),0),min(CASE WHEN retry_at>0 THEN retry_at END) FROM collection_downloads",[],|r|Ok(json!({"pending":r.get::<_,i64>(0)?,"errors":r.get::<_,i64>(1)?,"retry_at":r.get::<_,Option<i64>>(2)?})))}
pub(crate) fn refresh(c:&Connection,key:&str)->rusqlite::Result<()>{
 let parts:Vec<_>=key.split('.').collect();
 let condition=match parts.as_slice(){
  ["exploration","v4","work",_,"1"]=>"w.vndb_id=?1",
  ["exploration","v3",_,_,_]=>"EXISTS(SELECT 1 FROM settings s,json_tree(CASE WHEN json_valid(s.value) THEN s.value ELSE '{}' END) j WHERE s.key='exploration.v4.work.'||w.vndb_id||'.1' AND j.key='id' AND j.value=?1) OR EXISTS(SELECT 1 FROM relation_corrections r,json_tree(r.corrections_json) j WHERE r.vndb_id=w.vndb_id AND j.value=?1)",
  _=>return Ok(())
 };
 c.execute(&format!("INSERT INTO collection_downloads(work_id) SELECT w.id FROM works w WHERE w.merged_into IS NULL AND ({condition}) ON CONFLICT(work_id) DO UPDATE SET generation=generation+1,retry_at=0,error=''"),[parts[3]])?;Ok(())
}
pub(crate) fn finish(c:&Connection,id:&str,generation:i64,error:Option<&str>)->rusqlite::Result<()>{
 if let Some(error)=error{c.execute("UPDATE collection_downloads SET retry_at=?3,error=?4 WHERE work_id=?1 AND generation=?2",params![id,generation,now()+300,error])?;}else{c.execute("DELETE FROM collection_downloads WHERE work_id=?1 AND generation=?2",params![id,generation])?;}Ok(())
}

pub struct Worker{stop:tokio::sync::watch::Sender<bool>,task:tokio::task::JoinHandle<()>}
impl Worker{pub fn request_stop(&self){let _=self.stop.send(true);}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();self.task.abort();}}
pub fn start(a:App)->Worker{
 let(stop,mut stopped)=tokio::sync::watch::channel(false);
 let task=tokio::spawn(async move{loop{
  if *stopped.borrow(){break;}
  match crate::artwork_offline::enqueue_pending(&a){
   Ok(Some(job))=>{tokio::select!{biased;
    _=stopped.changed()=>{if let Ok(c)=a.db.lock(){let _=c.execute("UPDATE jobs SET state='interrupted',message='Core stopped; queued downloads will continue on restart.' WHERE id=?1 AND state IN ('queued','running','pausing')",[&job]);}break;}
    _=crate::artwork_offline::run_job(a.clone(),job.clone())=>{}
   }},
   Ok(None)=>{},Err(error)=>crate::diagnostics::record("error","collection.download.queue",&error,None),
  }
  tokio::select!{_=stopped.changed()=>break,_=tokio::time::sleep(std::time::Duration::from_secs(2))=>{}}
 }});Worker{stop,task}
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn schema47_backfill_is_transactional_and_stale_completion_cannot_drop_new_work(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");
  {let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Existing','Existing');DROP TRIGGER collection_download_insert;DROP TRIGGER collection_download_update;DROP TRIGGER collection_download_merge;DROP TABLE collection_downloads;PRAGMA user_version=47;").unwrap();}
  let db=crate::db::open(&path).unwrap();{
   let mut c=db.lock().unwrap();assert_eq!(status(&c).unwrap()["pending"],1);
   c.execute("UPDATE works SET notes='Keep',favorite=1 WHERE id='w'",[]).unwrap();
   assert_eq!(c.query_row("SELECT generation FROM collection_downloads",[],|r|r.get::<_,i64>(0)).unwrap(),1);
   {let tx=c.transaction().unwrap();tx.execute("UPDATE works SET cover='changed' WHERE id='w'",[]).unwrap();assert_eq!(tx.query_row("SELECT generation FROM collection_downloads",[],|r|r.get::<_,i64>(0)).unwrap(),2);}
   c.execute("UPDATE works SET vndb_id='v2' WHERE id='w'",[]).unwrap();finish(&c,"w",1,None).unwrap();assert_eq!(status(&c).unwrap()["pending"],1);
   finish(&c,"w",2,Some("Offline")).unwrap();assert_eq!(status(&c).unwrap()["errors"],1);
  }drop(db);
  let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(status(&c).unwrap()["errors"],1);assert!(status(&c).unwrap()["retry_at"].as_i64().unwrap()>now());
  c.execute("UPDATE works SET cover='new' WHERE id='w'",[]).unwrap();assert_eq!(status(&c).unwrap()["errors"],0);
  c.execute("INSERT INTO works(id,title,original_title) VALUES('other','Other','Other')",[]).unwrap();c.execute("UPDATE works SET merged_into='other' WHERE id='w'",[]).unwrap();assert_eq!(status(&c).unwrap()["pending"],1);
  c.execute("DELETE FROM works WHERE id='other'",[]).unwrap_err();
 }
 #[tokio::test]async fn adding_without_opening_a_page_downloads_full_profiles_and_restores_offline(){
  use crate::{artwork_library::Library,artwork_cache::Cache};
  use axum::{body::Body,http::Request};use tower::ServiceExt;
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();
  let cover="https://t.vndb.org/cv/01/1.jpg";let portrait="https://t.vndb.org/ch/02/2.png";let related="https://t.vndb.org/cv/03/3.jpg";
  let profile=json!({"vn":{"id":"v1","title":"Provider title","alttitle":"Original","description":"Full introduction","image":{"url":cover},"developers":[{"id":"p1","name":"Studio"}],"staff":[{"id":"s1"}],"va":[{"staff":{"id":"s1"}}]},"characters":[{"id":"c1","image":{"url":portrait}}],"more":false,"fetched_at":now()});
  crate::exploration::save(&a,"exploration.v4.work.v1.1",&profile).unwrap();
  for(kind,id,pages)in [("character","c1",1),("person","s1",2),("person","s2",1),("company","p1",1)]{for page in 1..=pages{
   crate::exploration::save(&a,&format!("exploration.v3.{kind}.{id}.{page}"),&json!({(kind):{"id":id,"description":"Detailed biography","traits":[{"name":"Trait"}],"image":{"url":portrait}},"works":[{"id":"v2","title":"Related work","image":{"url":related}}],"page":page,"more":page<pages,"fetched_at":now()})).unwrap();
  }}
  let mut cache=Cache::open(&a.state_dir).unwrap();for url in [cover,portrait,related]{cache.put(url,b"\x89PNG\r\n\x1a\nfixture").unwrap();}
  let request=Request::builder().method("POST").uri("/api/works").header("Host","127.0.0.1").header("Authorization",format!("Bearer {}",a.token)).header("content-type","application/json").body(Body::from(json!({"title":"Initial","vndb_id":"v1"}).to_string())).unwrap();
  let response=crate::router(a.clone()).oneshot(request).await.unwrap();assert_eq!(response.status(),200);
  let value:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),4096).await.unwrap()).unwrap();let id=value["id"].as_str().unwrap();
  crate::metadata::apply(&a.db,id,1,&json!({"title":"My title"}),&[],None).unwrap();a.db.lock().unwrap().execute("UPDATE works SET notes='My note',favorite=1 WHERE id=?1",[id]).unwrap();
  {use crate::relation_corrections::{Correction,Key,Link};let mut c=a.db.lock().unwrap();crate::relation_correction_store::change(&mut c,"v1",&crate::relation_correction_store::Edit{revision:0,request_id:"manual-person".into(),corrections:vec![Correction{id:"extra".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s2".into(),aid:None,role:"music".into(),note:"".into()},name:"Manual person".into(),character_name:None,spoiler:0})}]}).ok().unwrap();}
  let worker=start(a.clone());tokio::time::timeout(std::time::Duration::from_secs(10),async{loop{if status(&a.db.lock().unwrap()).unwrap()["pending"]==0{break;}tokio::time::sleep(std::time::Duration::from_millis(20)).await;}}).await.unwrap();drop(worker);
  let library=Library::open(&a.state_dir).unwrap();assert_eq!(library.stats().unwrap().images,2);assert_eq!(library.stats().unwrap().profiles,6);assert!(!crate::artwork_library::contains(&a.state_dir,related).unwrap());
  let personal:(String,String,String,bool)=a.db.lock().unwrap().query_row("SELECT title,description,notes,favorite FROM works WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();assert_eq!(personal,("My title".into(),"Full introduction".into(),"My note".into(),true));
  let snapshot=crate::metadata::snapshot(&a.db,id).unwrap();crate::metadata::apply(&a.db,id,snapshot["revision"].as_i64().unwrap(),&json!({}),&[],Some(json!({"description":"Newer catalog description"}))).unwrap();
  let job=crate::artwork_offline::enqueue_pending(&a).unwrap().unwrap();crate::artwork_offline::run_job(a.clone(),job).await;
  assert_eq!(a.db.lock().unwrap().query_row("SELECT description FROM works WHERE id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),"Newer catalog description");
  a.db.lock().unwrap().execute("INSERT INTO settings(key,value) VALUES('unrelated','plain text')",[]).unwrap();
  let key="exploration.v3.person.s1.1";let mut refreshed=library.profile(key).unwrap().unwrap();refreshed["person"]["name"]=json!("Updated name");crate::exploration::save(&a,key,&refreshed).unwrap();assert_eq!(status(&a.db.lock().unwrap()).unwrap()["pending"],1);
  for(key,raw)in library.profiles().unwrap(){let mut value:Value=serde_json::from_str(&raw).unwrap();value["fetched_at"]=json!(0);library.save_profile(&key,&value).unwrap();}cache.clear().unwrap();
  let backup=crate::backup::export(&a.db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::initialize(restored).unwrap();
  for(kind,id,page)in [("work","v1",1),("character","c1",1),("person","s1",2),("person","s2",1),("company","p1",1)]{let value=crate::exploration::load(&restored,kind,id,page,false).await.unwrap();assert_eq!(value["fetched_at"],0);assert_eq!(value["partial"],Value::Null);}
  assert_eq!(Library::open(&restored.state_dir).unwrap().stats().unwrap().images,2);assert!(!crate::artwork_library::contains(&restored.state_dir,related).unwrap());assert!(!restored.state_dir.join("artwork-cache.sqlite").exists());
 }
 #[tokio::test]async fn partial_download_keeps_good_images_and_retries_after_restart(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("data");let a=crate::initialize(path.clone()).unwrap();let cover="https://t.vndb.org/cv/01/1.jpg";
  crate::artwork_cache::Cache::open(&path).unwrap().put(cover,b"\x89PNG\r\n\x1a\nfixture").unwrap();
  let mut profile=json!({"vn":{"id":"v1","image":{"url":cover}},"characters":[],"partial":true,"fetched_at":now()});crate::exploration::save(&a,"exploration.v4.work.v1.1",&profile).unwrap();
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Work','Work','v1')",[]).unwrap();
  let job=crate::artwork_offline::enqueue_pending(&a).unwrap().unwrap();crate::artwork_offline::run_job(a.clone(),job).await;
  assert_eq!(status(&a.db.lock().unwrap()).unwrap()["errors"],1);assert!(crate::artwork_library::contains(&path,cover).unwrap());assert!(crate::artwork_offline::enqueue_pending(&a).unwrap().is_none());drop(a);
  let a=crate::initialize(path).unwrap();assert_eq!(status(&a.db.lock().unwrap()).unwrap()["errors"],1);profile["partial"]=json!(false);crate::exploration::save(&a,"exploration.v4.work.v1.1",&profile).unwrap();
  a.db.lock().unwrap().execute("UPDATE collection_downloads SET retry_at=0",[]).unwrap();let job=crate::artwork_offline::enqueue_pending(&a).unwrap().unwrap();crate::artwork_offline::run_job(a.clone(),job).await;
  assert_eq!(status(&a.db.lock().unwrap()).unwrap()["pending"],0);assert_eq!(crate::artwork_library::Library::open(&a.state_dir).unwrap().stats().unwrap().images,1);
 }
 #[tokio::test]async fn stopping_during_provider_wait_releases_job_and_preserves_pending_work(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();
  let provider=crate::exploration::PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Work','Work','v1')",[]).unwrap();
  let worker=start(a.clone());tokio::time::timeout(std::time::Duration::from_secs(5),async{loop{let active:i64=a.db.lock().unwrap().query_row("SELECT count(*) FROM jobs WHERE kind='artwork' AND state='running'",[],|r|r.get(0)).unwrap();if active==1{break;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;}}).await.unwrap();
  worker.request_stop();tokio::time::timeout(std::time::Duration::from_secs(5),async{while !worker.task.is_finished(){tokio::time::sleep(std::time::Duration::from_millis(10)).await;}}).await.unwrap();drop(worker);drop(provider);
  let c=a.db.lock().unwrap();assert_eq!(status(&c).unwrap()["pending"],1);assert_eq!(c.query_row("SELECT state FROM jobs",[],|r|r.get::<_,String>(0)).unwrap(),"interrupted");crate::roots::require_idle(&c).unwrap();
 }
}

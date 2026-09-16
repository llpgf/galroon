//! Durable catalog invalidation and one-at-a-time automatic smart-list computation.
use rusqlite::{Connection,OptionalExtension,params};
use std::{collections::BTreeMap,sync::{Arc,atomic::{AtomicBool,Ordering}}};
type R<T>=Result<T,String>;
static ACTIVE:std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<(String,String)>>>=std::sync::OnceLock::new();
pub(crate) struct Computation((String,String));
impl Drop for Computation{fn drop(&mut self){if let Ok(mut active)=ACTIVE.get().unwrap().lock(){active.remove(&self.0);}}}
pub(crate) fn claim(path:&str,id:&str)->Option<Computation>{
 let mut active=ACTIVE.get_or_init(Default::default).lock().ok()?;
 let key=(path.to_owned(),id.to_owned());if active.insert(key.clone()){Some(Computation(key))}else{None}
}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS smart_catalog_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),revision INTEGER NOT NULL,root_digest TEXT NOT NULL DEFAULT ''); INSERT OR IGNORE INTO smart_catalog_state(singleton,revision) VALUES(1,1); CREATE TABLE IF NOT EXISTS smart_compute_state(list_id TEXT PRIMARY KEY REFERENCES smart_lists(id),catalog_revision INTEGER NOT NULL,last_attempt INTEGER NOT NULL,error TEXT NOT NULL DEFAULT '');")?;
 for table in ["works","custom_tags","custom_tag_entities","relation_overrides","relation_corrections","entity_overrides","custom_tag_works","releases","work_releases","resource_bindings","resources","resource_files","files","roots","plans","plan_locations","curated_lists","list_entries","smart_lists"]{for action in ["INSERT","UPDATE","DELETE"]{c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS smart_dirty_{table}_{action} AFTER {action} ON {table} BEGIN UPDATE smart_catalog_state SET revision=revision+1 WHERE singleton=1; END;"))?;}}
 for action in ["INSERT","UPDATE"]{c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS smart_dirty_cache_{action} AFTER {action} ON settings WHEN NEW.key LIKE 'exploration.%' BEGIN UPDATE smart_catalog_state SET revision=revision+1 WHERE singleton=1; END;"))?;}
 for action in ["INSERT","UPDATE","DELETE"] {let row=if action=="DELETE"{"OLD"}else{"NEW"};c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS smart_dirty_managed_{action} AFTER {action} ON settings WHEN {row}.key='organization.managed_root' BEGIN UPDATE smart_catalog_state SET revision=revision+1 WHERE singleton=1; END;"))?;}
 c.execute_batch("CREATE INDEX IF NOT EXISTS smart_plan_resource ON plan_locations(resource_id,plan_id);")?;
 c.execute_batch("CREATE TRIGGER IF NOT EXISTS smart_dirty_cache_DELETE AFTER DELETE ON settings WHEN OLD.key LIKE 'exploration.%' BEGIN UPDATE smart_catalog_state SET revision=revision+1 WHERE singleton=1; END;")?;Ok(())
}
pub fn tick(c:&mut Connection,online:&BTreeMap<String,bool>,stopped:&AtomicBool)->R<bool>{tick_at(c,online,stopped,crate::db::now())}
fn tick_at(c:&mut Connection,online:&BTreeMap<String,bool>,stopped:&AtomicBool,now:i64)->R<bool>{
 let Some((id,list_revision,revision))=plan(c,online,stopped,now)? else{return Ok(false);};
 let started=std::time::Instant::now();crate::diagnostics::record("info","smart.compute.started",&format!("List {id}; revision {list_revision}"),None);
 let outcome={let tx=c.transaction().map_err(|e|e.to_string())?;match crate::smart_snapshots::build_at(&tx,&id,&crate::smart_snapshots::Build{revision:list_revision,request_id:format!("auto:{}",crate::db::id())},online,&||stopped.load(Ordering::SeqCst),now){Ok(_)=>{if stopped.load(Ordering::SeqCst){return Ok(false);}tx.execute("INSERT INTO smart_compute_state(list_id,catalog_revision,last_attempt,error) VALUES(?1,?2,?3,'') ON CONFLICT(list_id) DO UPDATE SET catalog_revision=excluded.catalog_revision,last_attempt=excluded.last_attempt,error=''",params![id,revision,now]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(())},Err(e)=>Err(e.0)}};
 if stopped.load(Ordering::SeqCst){crate::diagnostics::record("info","smart.compute.cancelled",&format!("List {id}; rolled back after {} ms",started.elapsed().as_millis()),None);return Ok(false);}
 if outcome.is_ok(){crate::diagnostics::record("info","smart.compute.completed",&format!("List {id}; committed in {} ms",started.elapsed().as_millis()),None);}
 if let Err(error)=outcome{crate::diagnostics::record("error","smart.compute",&error,None);c.execute("INSERT INTO smart_compute_state(list_id,catalog_revision,last_attempt,error) VALUES(?1,?2,?3,?4) ON CONFLICT(list_id) DO UPDATE SET catalog_revision=excluded.catalog_revision,last_attempt=excluded.last_attempt,error=excluded.error",params![id,revision,now,error]).map_err(|e|e.to_string())?;}Ok(true)
}
pub(crate) fn observe_roots(c:&Connection,online:&BTreeMap<String,bool>)->R<()> {
 let digest=serde_json::to_string(online).map_err(|e|e.to_string())?;
 c.execute("UPDATE smart_catalog_state SET revision=revision+1,root_digest=?1 WHERE singleton=1 AND root_digest<>?1",[digest]).map_err(|e|e.to_string())?;Ok(())
}
fn plan(c:&mut Connection,online:&BTreeMap<String,bool>,stopped:&AtomicBool,now:i64)->R<Option<(String,i64,i64)>>{
 if stopped.load(Ordering::SeqCst){return Ok(None);}
 let busy:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE state IN ('queued','running','pausing','cancelling')) OR EXISTS(SELECT 1 FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())?;if busy{return Ok(None);}
 {let tx=c.transaction().map_err(|e|e.to_string())?;crate::smart_snapshots::prune(&tx,now).map_err(|e|e.0)?;if stopped.load(Ordering::SeqCst){return Ok(None);}tx.commit().map_err(|e|e.to_string())?;}
 observe_roots(c,online)?;
 let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let candidate:Option<(String,i64)>=c.query_row("SELECT l.id,l.revision FROM smart_lists l LEFT JOIN smart_compute_state s ON s.list_id=l.id WHERE l.deleted=0 AND (s.catalog_revision IS NULL OR s.catalog_revision<>?1 OR (s.error<>'' AND s.last_attempt<?2) OR ((s.error='' OR s.last_attempt<?2) AND EXISTS(SELECT 1 FROM smart_snapshot_freshness f WHERE f.snapshot_id=(SELECT id FROM smart_snapshots WHERE list_id=l.id ORDER BY rowid DESC LIMIT 1) AND f.expires<=?3))) ORDER BY coalesce(s.last_attempt,0),l.id LIMIT 1",params![revision,now-60,now],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
 let Some((id,list_revision))=candidate else{return Ok(None);};if stopped.load(Ordering::SeqCst){return Ok(None);}
 Ok(Some((id,list_revision,revision)))
}
fn offlock(db:&crate::db::Db,stopped:&AtomicBool,before_publish:&dyn Fn())->R<bool>{
 if stopped.load(Ordering::SeqCst){return Ok(false);}
 let (path,roots)={let Ok(c)=db.try_lock()else{return Ok(false);};let path=c.path().ok_or("Catalog path unavailable")?.to_string();
 let mut q=c.prepare("SELECT id,path FROM roots").map_err(|e|e.to_string())?;let roots=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;(path,roots)};
 let online=roots.into_iter().map(|(id,path)|(id,std::fs::metadata(path).is_ok_and(|m|m.is_dir()))).collect();
 let now=crate::db::now();let Some((id,list_revision,revision))=({let Ok(mut c)=db.try_lock()else{return Ok(false);};plan(&mut c,&online,stopped,now)?})else{return Ok(false);};
 let Some(_claim)=claim(&path,&id)else{return Ok(false);};
 let started=std::time::Instant::now();crate::diagnostics::record("info","smart.compute.started",&format!("List {id}; revision {list_revision}"),None);
 let evaluated=(||{
  let mut read=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
  let tx=read.transaction().map_err(|e|e.to_string())?;
  let observed:i64=tx.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  if observed!=revision{return Ok(None);}
  let row=crate::smart_lists::read_one(&tx,&id).map_err(|e|e.0)?;
  let definition=serde_json::from_value(row["definition"].clone()).map_err(|e|e.to_string())?;
  crate::smart_snapshots::evaluate_scope(&tx,&definition,&online,&||stopped.load(Ordering::SeqCst),now).map(Some).map_err(|e|e.0)
 })();
 if stopped.load(Ordering::SeqCst){crate::diagnostics::record("info","smart.compute.cancelled",&format!("List {id}; discarded after {} ms",started.elapsed().as_millis()),None);return Ok(false);}
 before_publish();
 let mut c=db.lock().map_err(|e|e.to_string())?;
 if stopped.load(Ordering::SeqCst){return Ok(false);}
 let tx=c.transaction().map_err(|e|e.to_string())?;
 let current:i64=tx.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let busy:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE state IN ('queued','running','pausing','cancelling')) OR EXISTS(SELECT 1 FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if current!=revision||busy{return Ok(false);}
 let error=match evaluated{
  Ok(Some(prepared))=>{crate::smart_snapshots::build_using(&tx,&id,&crate::smart_snapshots::Build{revision:list_revision,request_id:format!("auto:{}",crate::db::id())},&online,&||stopped.load(Ordering::SeqCst),now,Some(prepared)).map_err(|e|e.0)?;String::new()},
  Ok(None)=>return Ok(false),
  Err(error)=>{crate::diagnostics::record("error","smart.compute",&error,None);error}
 };
 if stopped.load(Ordering::SeqCst){return Ok(false);}
 tx.execute("INSERT INTO smart_compute_state(list_id,catalog_revision,last_attempt,error) VALUES(?1,?2,?3,?4) ON CONFLICT(list_id) DO UPDATE SET catalog_revision=excluded.catalog_revision,last_attempt=excluded.last_attempt,error=excluded.error",params![id,revision,now,error]).map_err(|e|e.to_string())?;
 tx.commit().map_err(|e|e.to_string())?;
 if error.is_empty(){crate::diagnostics::record("info","smart.compute.completed",&format!("List {id}; committed in {} ms",started.elapsed().as_millis()),None);}
 Ok(true)
}
pub struct Worker{task:tokio::task::JoinHandle<()>,stop:Arc<AtomicBool>}
impl Worker{pub fn request_stop(&self){self.stop.store(true,Ordering::SeqCst);}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();self.task.abort();}}
pub fn start(db:crate::db::Db)->Worker{let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();let task=tokio::spawn(async move{loop{if stopped.load(Ordering::SeqCst){break;}let database=db.clone();let stop=stopped.clone();let _=tokio::task::spawn_blocking(move||{if let Err(error)=offlock(&database,&stop,&||{}){crate::diagnostics::record("error","smart.worker",&error,None);}}).await;tokio::time::sleep(std::time::Duration::from_secs(3)).await;}});Worker{task,stop}}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn computation_claim_is_per_catalog_and_list_and_released_on_drop(){
 let first=claim("claim-fixture-a","s").unwrap();assert!(claim("claim-fixture-a","s").is_none());
 let _other_list=claim("claim-fixture-a","other").unwrap();let _other_catalog=claim("claim-fixture-b","s").unwrap();
 drop(first);assert!(claim("claim-fixture-a","s").is_some());
 }
 #[test]fn offlock_discards_changed_catalog_and_cancelled_publication(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let stop=AtomicBool::new(false);
 {let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','W','W')",[]).unwrap();let definition=serde_json::json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','S','',1,?1)",[definition.to_string()]).unwrap();}
 assert!(offlock(&db,&stop,&||{}).unwrap());
 db.lock().unwrap().execute("UPDATE works SET status='playing'",[]).unwrap();
 assert!(!offlock(&db,&stop,&||{db.try_lock().expect("Main catalog lock must be released").execute("UPDATE works SET title='Changed during evaluation'",[]).unwrap();}).unwrap());
 assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 assert!(!offlock(&db,&stop,&||stop.store(true,Ordering::SeqCst)).unwrap());
 assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 stop.store(false,Ordering::SeqCst);assert!(offlock(&db,&stop,&||{}).unwrap());
 let c=db.lock().unwrap();assert_eq!(crate::smart_snapshots::page(&c,"s",&Default::default()).ok().unwrap()["total"],0);
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn upgrade_invalidates_old_snapshots_once(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
 let revision={let c=db.lock().unwrap();c.pragma_update(None,"user_version",26).unwrap();c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap()};drop(db);
 let db=crate::db::open(&path).unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),revision+1);drop(db);
 let db=crate::db::open(&path).unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),revision+1);
 }
 #[test]fn expiry_alone_recomputes_negative_rules_and_preserves_pinned_pages(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
 let now=crate::db::now();let definition=serde_json::json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"brands","mode":"none","values":["p2"]}}});
 c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','Brands','',1,?1)",[definition.to_string()]).unwrap();
 c.execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','W','W','v1')",[]).unwrap();
 let cache=serde_json::json!({"vn":{"developers":[{"id":"p1"}]},"fetched_at":now-86399});
 c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v1.1',?1)",[cache.to_string()]).unwrap();
 let stop=AtomicBool::new(false);assert!(tick_at(&mut c,&BTreeMap::new(),&stop,now).unwrap());
 let old=crate::smart_snapshots::page(&c,"s",&Default::default()).ok().unwrap();assert_eq!(old["total"],1);
 assert!(!tick_at(&mut c,&BTreeMap::new(),&stop,now).unwrap());
 assert!(crate::smart_snapshots::latest_expired(&c,"s",now+1).ok().unwrap());
 assert!(tick_at(&mut c,&BTreeMap::new(),&stop,now+1).unwrap());
 let fresh=crate::smart_snapshots::page(&c,"s",&Default::default()).ok().unwrap();assert_eq!(fresh["total"],0);assert_eq!(fresh["unknown_count"],1);
 assert!(!tick_at(&mut c,&BTreeMap::new(),&stop,now+2).unwrap());
 let pinned=crate::smart_snapshots::page(&c,"s",&crate::smart_snapshots::Page{snapshot:Some(old["snapshot"].as_str().unwrap().into()),after:None}).ok().unwrap();assert_eq!(pinned["total"],1);assert_eq!(pinned["newer_available"],true);
 let future=crate::smart_facts::load_at(&c,"w",&BTreeMap::new(),now-86400).unwrap();assert!(!future.fields[&crate::smart_rules::Field::Brands].complete);
 }
 #[test]fn committed_changes_recompute_while_old_results_stay_and_stop_prevents_writes(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let definition=serde_json::json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','Auto','',1,?1)",[definition.to_string()]).unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','W','W')",[]).unwrap();let stop=AtomicBool::new(false);assert!(tick(&mut c,&BTreeMap::new(),&stop).unwrap());let old=crate::smart_snapshots::page(&c,"s",&Default::default()).ok().unwrap();assert_eq!(old["total"],1);assert!(!tick(&mut c,&BTreeMap::new(),&stop).unwrap());
 {let tx=c.transaction().unwrap();tx.execute("UPDATE works SET status='playing' WHERE id='w'",[]).unwrap();}assert!(!tick(&mut c,&BTreeMap::new(),&stop).unwrap());c.execute("UPDATE works SET status='playing' WHERE id='w'",[]).unwrap();assert!(tick(&mut c,&BTreeMap::new(),&stop).unwrap());assert_eq!(crate::smart_snapshots::page(&c,"s",&Default::default()).ok().unwrap()["total"],0);assert_eq!(crate::smart_snapshots::page(&c,"s",&crate::smart_snapshots::Page{snapshot:Some(old["snapshot"].as_str().unwrap().into()),after:None}).ok().unwrap()["total"],1);
 c.execute("UPDATE works SET status='backlog' WHERE id='w'",[]).unwrap();stop.store(true,Ordering::SeqCst);let changes=c.total_changes();assert!(!tick(&mut c,&BTreeMap::new(),&stop).unwrap());assert_eq!(changes,c.total_changes());
 }
}

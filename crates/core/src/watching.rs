//! Native notifications are hints. Only successful directory enumeration proves absence.
use crate::{App,db::{Db,now},scan,plans,roots};
use notify::{Watcher,RecursiveMode,EventKind};
use rusqlite::{Connection,params,OptionalExtension};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::{collections::BTreeMap,path::{Path,PathBuf,Component},sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},thread,time::Duration};
type R<T>=Result<T,String>;
fn sql<T>(r:rusqlite::Result<T>)->R<T>{r.map_err(|e|e.to_string())}
const QUIET_SECONDS:i64=3;
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]struct Change{scope:String,recursive:bool}
#[derive(Clone)]struct Config{id:String,path:String,enabled:bool,exclude:Vec<String>,revision:i64}
pub fn migrate(c:&Connection)->R<()>{sql(c.execute_batch("CREATE TABLE IF NOT EXISTS source_watch(root_id TEXT PRIMARY KEY REFERENCES roots(id),path TEXT NOT NULL,enabled INTEGER NOT NULL DEFAULT 1,exclude TEXT NOT NULL DEFAULT '[]',revision INTEGER NOT NULL DEFAULT 1,status TEXT NOT NULL DEFAULT 'starting',message TEXT NOT NULL DEFAULT '',needs_scan INTEGER NOT NULL DEFAULT 1,gap_after INTEGER NOT NULL DEFAULT 0,pending TEXT NOT NULL DEFAULT '[]',last_event INTEGER NOT NULL DEFAULT 0,job_id TEXT);"))}
pub(crate) fn ensure(c:&Connection)->R<()>{sql(c.execute("INSERT OR IGNORE INTO source_watch(root_id,path) SELECT id,path FROM roots",[]))?;Ok(())}
pub fn status(c:&Connection,rid:&str)->R<Value>{sql(c.query_row("SELECT enabled,exclude,revision,status,message,needs_scan,pending,job_id,(SELECT state FROM jobs WHERE id=job_id) FROM source_watch WHERE root_id=?1",[rid],|r|Ok(json!({"enabled":r.get::<_,bool>(0)?,"exclude":serde_json::from_str::<Value>(&r.get::<_,String>(1)?).unwrap_or(json!([])),"revision":r.get::<_,i64>(2)?,"status":r.get::<_,String>(3)?,"message":r.get::<_,String>(4)?,"needs_scan":r.get::<_,bool>(5)?,"pending":serde_json::from_str::<Vec<Value>>(&r.get::<_,String>(6)?).unwrap_or_default().len(),"job_id":r.get::<_,Option<String>>(7)?,"job_state":r.get::<_,Option<String>>(8)?}))))}
#[derive(Deserialize)]pub struct Edit{pub revision:i64,pub enabled:bool,pub exclude:Vec<String>}
fn valid_scope(s:&str)->bool{!Path::new(s).is_absolute()&&!s.contains(':')&&!s.chars().any(|c|c.is_control())&&!Path::new(s).components().any(|c|matches!(c,Component::ParentDir|Component::Prefix(_)|Component::RootDir))}
pub fn configure(db:&Db,rid:&str,input:Edit)->R<Value>{
 if input.exclude.len()>100||input.exclude.iter().any(|s|s.is_empty()||s.len()>1024||!valid_scope(s)){return Err("Watch exclusions must be relative folders inside the source".into());}
 let c=db.lock().map_err(|e|e.to_string())?;ensure(&c)?;let n=sql(c.execute("UPDATE source_watch SET enabled=?2,exclude=?3,revision=revision+1,needs_scan=1,pending='[]',job_id=CASE WHEN (SELECT state FROM jobs WHERE id=job_id) IN ('cancelled','failed','completed','superseded') THEN NULL ELSE job_id END,status='starting',message='' WHERE root_id=?1 AND revision=?4",params![rid,input.enabled,json!(input.exclude).to_string(),input.revision]))?;if n!=1{return Err("Watch settings changed. Reload before saving.".into());}status(&c,rid)
}
fn gap(c:&Connection,rid:&str,message:&str)->R<()>{sql(c.execute("UPDATE source_watch SET needs_scan=1,gap_after=(SELECT coalesce(max(rowid),0) FROM jobs),message=?2 WHERE root_id=?1",params![rid,message]))?;Ok(())}
fn exclusions(app:&App,cfg:&Config)->Vec<String>{let mut excluded=cfg.exclude.clone();if let Ok(rel)=app.state_dir.strip_prefix(&cfg.path){if !rel.as_os_str().is_empty(){excluded.push(rel.to_string_lossy().into());}}excluded}
fn inside(p:&Path,base:&Path)->bool{
 #[cfg(windows)]{Path::new(&p.to_string_lossy().to_lowercase()).starts_with(base.to_string_lossy().to_lowercase())}
 #[cfg(not(windows))]{p.starts_with(base)}
}
fn merge(changes:&mut Vec<Change>,new:Change){
 if changes.iter().any(|c|c.scope==new.scope&&(c.recursive||!new.recursive)||c.recursive&&inside(Path::new(&new.scope),Path::new(&c.scope))){return;}
 changes.retain(|c|!(c.scope==new.scope||new.recursive&&inside(Path::new(&c.scope),Path::new(&new.scope))));changes.push(new);changes.sort_by(|a,b|a.scope.cmp(&b.scope));
}
fn ignored(rel:&Path,exclude:&[String])->bool{crate::backup::protected_path(rel)||rel.components().any(|c|{let name=c.as_os_str().to_string_lossy();["@eaDir","#recycle",".snapshot",".galroon"].contains(&name.as_ref())||name.starts_with(".galroon-")})||exclude.iter().any(|ex|inside(rel,Path::new(ex)))}
fn hints(root:&Path,paths:&[PathBuf],exclude:&[String],state_dir:&Path)->Vec<Change>{
 let mut changes=Vec::new();for path in paths{
  if inside(path,state_dir){continue;}let Ok(rel)=path.strip_prefix(root)else{continue;};if !valid_scope(&rel.to_string_lossy())||ignored(rel,exclude){continue;}
  // Root events never recursively walk the entire source. A new/moved-in directory
  // gets its own recursive scan; its parent is enumerated without visiting siblings.
  if !rel.as_os_str().is_empty()&&path.is_dir(){merge(&mut changes,Change{scope:rel.to_string_lossy().into(),recursive:true});}
  let mut parent=path.parent().unwrap_or(root);while inside(parent,root)&&!parent.is_dir(){parent=parent.parent().unwrap_or(root);}
  if let Ok(scope)=parent.strip_prefix(root){merge(&mut changes,Change{scope:scope.to_string_lossy().into(),recursive:false});}
  if rel.as_os_str().is_empty(){merge(&mut changes,Change{scope:String::new(),recursive:false});}
 }changes
}
fn save_hints(c:&Connection,cfg:&Config,changes:Vec<Change>)->R<()>{
 if changes.is_empty(){return Ok(());}let current=sql(c.query_row("SELECT pending FROM source_watch WHERE root_id=?1 AND revision=?2 AND enabled=1",params![cfg.id,cfg.revision],|r|r.get::<_,String>(0)).optional())?;let Some(current)=current else{return Ok(());};let mut pending:Vec<Change>=serde_json::from_str(&current).map_err(|e|e.to_string())?;for change in changes{merge(&mut pending,change);}
 if pending.len()>128{gap(c,&cfg.id,"Too many changed folders. Run a manual scan to reconcile this source.")?;pending.clear();}
 sql(c.execute("UPDATE source_watch SET pending=?2,last_event=?3 WHERE root_id=?1",params![cfg.id,json!(pending).to_string(),now()]))?;Ok(())
}
fn enqueue(app:&App,cfg:&Config,stop:&AtomicBool)->R<Option<String>>{
 let _gate=app.mutation_lock.lock().map_err(|e|e.to_string())?;if stop.load(Ordering::SeqCst){return Ok(None);}let mut c=app.db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;
 if roots::require_idle(&tx).is_err(){return Ok(None);}
 let(saved,event,job):(String,i64,Option<String>)=sql(tx.query_row("SELECT pending,last_event,job_id FROM source_watch WHERE root_id=?1 AND revision=?2 AND enabled=1 AND status='watching'",params![cfg.id,cfg.revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))))?;
 if let Some(j)=job{let state=sql(tx.query_row("SELECT state FROM jobs WHERE id=?1",[&j],|r|r.get::<_,String>(0)).optional())?;match state.as_deref(){Some("completed")|Some("superseded")|None=>{sql(tx.execute("UPDATE source_watch SET job_id=NULL WHERE root_id=?1",[&cfg.id]))?;},Some("cancelled")=>{sql(tx.execute("UPDATE source_watch SET enabled=0,revision=revision+1,status='disabled',needs_scan=1,message='Automatic updates stopped after cancellation. Enable watching to continue.' WHERE root_id=?1",[&cfg.id]))?;sql(tx.commit())?;return Ok(None);},_=>return Ok(None)}}
 if now()-event<QUIET_SECONDS{sql(tx.commit())?;return Ok(None);}
 let mut pending:Vec<Change>=serde_json::from_str(&saved).map_err(|e|e.to_string())?;
 // First attachment, reconnects and notification gaps reconcile automatically.
 // A failed/partial baseline remains visible for review instead of looping forever.
 let reconcile:bool=sql(tx.query_row("SELECT needs_scan=1 AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.rowid>source_watch.gap_after AND j.kind='scan' AND json_extract(j.spec,'$.root_id')=source_watch.root_id AND json_extract(j.spec,'$.scope')='' AND (j.state IN ('failed','paused','interrupted') OR (j.state='completed' AND j.errors>0))) FROM source_watch WHERE root_id=?1",[&cfg.id],|r|r.get(0)))? && crate::automatch::enabled(&tx)?;
 if reconcile {pending.clear();pending.push(Change{scope:String::new(),recursive:true});}
 if pending.is_empty(){sql(tx.commit())?;return Ok(None);}let mut change=pending.remove(0);
 // If a queued directory disappeared, enumerate its nearest surviving parent.
 while !Path::new(&cfg.path).join(&change.scope).is_dir(){if change.scope.is_empty(){return Ok(None);}change.scope=Path::new(&change.scope).parent().unwrap_or(Path::new("")).to_string_lossy().into();change.recursive=false;}
 let j=scan::enqueue(&tx,scan::ScanSpec{root_id:cfg.id.clone(),scope:change.scope,exclude:exclusions(app,cfg)},change.recursive,true)?;
 sql(tx.execute("UPDATE source_watch SET pending=?2,job_id=?3 WHERE root_id=?1",params![cfg.id,json!(pending).to_string(),j]))?;sql(tx.commit())?;Ok(Some(j))
}
struct Attached{_watcher:notify::RecommendedWatcher,rx:mpsc::Receiver<notify::Result<notify::Event>>,overflow:Arc<AtomicBool>,revision:i64,path:String}
pub struct Service{stop:Arc<AtomicBool>,join:Option<thread::JoinHandle<()>>}
impl Service{pub fn request_stop(&self){self.stop.store(true,Ordering::SeqCst);}}
impl Drop for Service{fn drop(&mut self){self.request_stop();if let Some(j)=self.join.take(){let _=j.join();}}}
pub fn start(app:App)->R<Service>{
 {let c=app.db.lock().map_err(|e|e.to_string())?;ensure(&c)?;sql(c.execute("UPDATE source_watch SET status='starting',needs_scan=1,gap_after=(SELECT coalesce(max(rowid),0) FROM jobs),message='Core restarted. Scan to cover changes while it was offline.' WHERE enabled=1",[]))?;}
 let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();let join=thread::Builder::new().name("galroon-source-watch".into()).spawn(move||{
  let mut attached=BTreeMap::new();let mut retries=BTreeMap::new();while !worker_stop.load(Ordering::SeqCst){if let Err(error)=tick(&app,&worker_stop,&mut attached,&mut retries){eprintln!("Source watcher: {error}");}thread::sleep(Duration::from_millis(500));}
 }).map_err(|e|e.to_string())?;Ok(Service{stop,join:Some(join)})
}
fn tick(app:&App,stop:&AtomicBool,attached:&mut BTreeMap<String,Attached>,retries:&mut BTreeMap<String,(i64,std::time::Instant)>)->R<()>{
 let configs={let c=app.db.lock().map_err(|e|e.to_string())?;ensure(&c)?;
  sql(c.execute("UPDATE source_watch SET path=(SELECT path FROM roots WHERE id=root_id),pending='[]',job_id=NULL,revision=revision+1,needs_scan=1,status='starting' WHERE path!=(SELECT path FROM roots WHERE id=root_id)",[]))?;
  let mut s=sql(c.prepare("SELECT root_id,path,enabled,exclude,revision FROM source_watch ORDER BY last_event,root_id"))?;let rows=sql(sql(s.query_map([],|r|Ok(Config{id:r.get(0)?,path:r.get(1)?,enabled:r.get(2)?,exclude:serde_json::from_str(&r.get::<_,String>(3)?).unwrap_or_default(),revision:r.get(4)?})))?.collect::<rusqlite::Result<Vec<_>>>())?;rows};
 for cfg in configs{
  if stop.load(Ordering::SeqCst){break;}
  if !cfg.enabled{attached.remove(&cfg.id);retries.remove(&cfg.id);let c=app.db.lock().map_err(|e|e.to_string())?;sql(c.execute("UPDATE source_watch SET status='disabled' WHERE root_id=?1",[&cfg.id]))?;continue;}
  let root=Path::new(&cfg.path);if !root.is_dir()||plans::validate_chain(root).is_err(){let was=attached.remove(&cfg.id).is_some();let c=app.db.lock().map_err(|e|e.to_string())?;if was{gap(&c,&cfg.id,"Source went offline. Reconnect it and scan to reconcile missed changes.")?;}sql(c.execute("UPDATE source_watch SET status='offline',needs_scan=1 WHERE root_id=?1",[&cfg.id]))?;continue;}
  if attached.get(&cfg.id).is_some_and(|a|a.path!=cfg.path||a.revision!=cfg.revision){attached.remove(&cfg.id);}
  if !attached.contains_key(&cfg.id){
   if retries.get(&cfg.id).is_some_and(|(rev,until)|*rev==cfg.revision&&std::time::Instant::now()<*until){continue;}
   let (tx,rx)=mpsc::sync_channel(256);let overflow=Arc::new(AtomicBool::new(false));let full=overflow.clone();let watcher=notify::recommended_watcher(move|event:notify::Result<notify::Event>|{if matches!(&event,Ok(e)if matches!(e.kind,EventKind::Access(_))){return;}if tx.try_send(event).is_err(){full.store(true,Ordering::SeqCst);}});
   let result=watcher.and_then(|mut watcher|{watcher.watch(root,RecursiveMode::Recursive)?;Ok(watcher)});
   let c=app.db.lock().map_err(|e|e.to_string())?;gap(&c,&cfg.id,"Scan to cover changes before watching began.")?;
   match result{Ok(watcher)=>{retries.remove(&cfg.id);attached.insert(cfg.id.clone(),Attached{_watcher:watcher,rx,overflow,revision:cfg.revision,path:cfg.path.clone()});sql(c.execute("UPDATE source_watch SET status='watching' WHERE root_id=?1",[&cfg.id]))?;},Err(e)=>{retries.insert(cfg.id.clone(),(cfg.revision,std::time::Instant::now()+Duration::from_secs(30)));sql(c.execute("UPDATE source_watch SET status='error',message=?2 WHERE root_id=?1",params![cfg.id,format!("Watching unavailable: {e}. Use manual scanning.")]))?;continue;}}
  }
  let a=&attached[&cfg.id];let mut changed=Vec::new();let mut missed=a.overflow.swap(false,Ordering::SeqCst);let mut backend_failed=false;
  for result in a.rx.try_iter().take(256){match result{Ok(e)=>{if e.need_rescan()||e.paths.is_empty(){missed=true;}for hint in hints(root,&e.paths,&cfg.exclude,&app.state_dir){merge(&mut changed,hint);}},Err(_)=>{missed=true;backend_failed=true;}}}
  {let c=app.db.lock().map_err(|e|e.to_string())?;if missed{gap(&c,&cfg.id,"Some file notifications were missed. Run a manual scan.")?;}save_hints(&c,&cfg,changed)?;
   // Only a later successful full scan with the same exclusions closes a notification gap.
   let baseline:Option<String>=sql(c.query_row("SELECT j.spec FROM jobs j JOIN source_watch w ON w.root_id=?1 WHERE j.rowid>w.gap_after AND j.kind='scan' AND j.state='completed' AND j.errors=0 AND json_extract(j.spec,'$.root_id')=?1 AND json_extract(j.spec,'$.scope')='' AND coalesce(json_extract(j.spec,'$.recursive'),1)=1 ORDER BY j.rowid DESC LIMIT 1",[&cfg.id],|r|r.get(0)).optional())?;
   if let Some(s)=baseline{let spec:scan::ScanSpec=serde_json::from_str(&s).map_err(|e|e.to_string())?;if spec.exclude==cfg.exclude||spec.exclude==exclusions(app,&cfg){sql(c.execute("UPDATE source_watch SET needs_scan=0,message='' WHERE root_id=?1",[&cfg.id]))?;}}
  }
  if backend_failed{attached.remove(&cfg.id);retries.insert(cfg.id.clone(),(cfg.revision,std::time::Instant::now()+Duration::from_secs(30)));let c=app.db.lock().map_err(|e|e.to_string())?;sql(c.execute("UPDATE source_watch SET status='error',message='File watcher disconnected. Retrying; manual scanning remains available.' WHERE root_id=?1",[&cfg.id]))?;continue;}
  // Share admission with manual scans, transfer/file plans and Core shutdown.
  if !stop.load(Ordering::SeqCst){if let Some(job)=enqueue(app,&cfg,stop)?{scan::run(app.db.clone(),job);}}
 }Ok(())
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn enabled_intake_reconciles_initial_and_missed_changes_without_manual_scan(){
  let(_t,app,cfg)=fixture();{let c=app.db.lock().unwrap();c.execute("INSERT INTO settings(key,value) VALUES('auto_match_enabled','1')",[]).unwrap();gap(&c,&cfg.id,"Test gap").unwrap();c.execute("UPDATE source_watch SET status='watching'",[]).unwrap();}
  let job=enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().unwrap();scan::run(app.db.clone(),job.clone());
  let c=app.db.lock().unwrap();let spec:String=c.query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();let value:Value=serde_json::from_str(&spec).unwrap();assert_eq!(value["scope"],"");assert_eq!(value["automatic"],true);assert_eq!(value["recursive"],true);
  // The same resources are not announced again by reconciliation scans.
  let notices:i64=c.query_row("SELECT count(*) FROM events WHERE code='resource.discovered'",[],|r|r.get(0)).unwrap();assert_eq!(notices,2);
  c.execute("UPDATE jobs SET errors=1 WHERE id=?1",[&job]).unwrap();drop(c);assert!(enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().is_none());
 }
 fn fixture()->(tempfile::TempDir,App,Config){let t=tempfile::tempdir().unwrap();let source=t.path().join("source");std::fs::create_dir_all(source.join("A/nested")).unwrap();std::fs::create_dir_all(source.join("B")).unwrap();for p in ["A/a.bin","A/nested/b.bin","B/outside.bin"]{std::fs::write(source.join(p),p.as_bytes()).unwrap();}let app=crate::initialize(t.path().join("state")).unwrap();let source=source.canonicalize().unwrap();{let c=app.db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Watch fixture')",[source.to_str().unwrap()]).unwrap();ensure(&c).unwrap();}let cfg=Config{id:"r".into(),path:source.to_string_lossy().into(),enabled:true,exclude:vec![],revision:1};let j=scan::create(&app.db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(app.db.clone(),j);(t,app,cfg)}
 fn pending(app:&App,cfg:&Config,changes:Vec<Change>){let c=app.db.lock().unwrap();save_hints(&c,cfg,changes).unwrap();c.execute("UPDATE source_watch SET last_event=0,status='watching'",[]).unwrap();}
 #[test]fn notifications_are_scoped_coalesced_and_exclude_internal_paths(){let(_t,app,mut cfg)=fixture();let root=Path::new(&cfg.path);cfg.exclude=vec!["B".into()];let paths=vec![root.join("A/nested/new.bin"),root.join("A/nested"),root.join("B/outside.bin"),root.join(".galroon-copy/files/a.bin")];let changes=hints(root,&paths,&cfg.exclude,&app.state_dir);assert!(changes.contains(&Change{scope:"A".into(),recursive:false}));assert!(changes.iter().any(|c|Path::new(&c.scope)==Path::new("A/nested")&&c.recursive));assert_eq!(changes.len(),2);let mut combined=changes;merge(&mut combined,Change{scope:"A".into(),recursive:true});assert_eq!(combined,vec![Change{scope:"A".into(),recursive:true}]);assert!(!valid_scope("../escape"));assert!(!valid_scope("C:\\escape"));}
 #[test]fn incremental_scan_does_not_visit_siblings_and_deletion_is_proven_by_parent(){let(_t,app,cfg)=fixture();let root=Path::new(&cfg.path);std::fs::remove_file(root.join("A/nested/b.bin")).unwrap();std::fs::remove_dir(root.join("A/nested")).unwrap();std::fs::write(root.join("A/new.bin"),b"new").unwrap();pending(&app,&cfg,vec![Change{scope:"A".into(),recursive:false}]);let j=enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().unwrap();scan::run(app.db.clone(),j.clone());let c=app.db.lock().unwrap();let n:i64=c.query_row("SELECT count(*) FROM observations WHERE job_id=?1",[&j],|r|r.get(0)).unwrap();assert_eq!(n,2);let missing:String=c.query_row("SELECT availability FROM files WHERE relative_path LIKE '%b.bin'",[],|r|r.get(0)).unwrap();assert_eq!(missing,"missing");let outside:String=c.query_row("SELECT availability FROM files WHERE relative_path LIKE '%outside.bin'",[],|r|r.get(0)).unwrap();assert_eq!(outside,"present");assert_eq!(std::fs::read(root.join("B/outside.bin")).unwrap(),b"B/outside.bin");}
 #[test]fn queue_survives_reopen_and_respects_pause_cancel_and_shutdown(){let(t,app,cfg)=fixture();pending(&app,&cfg,vec![Change{scope:"A".into(),recursive:true}]);assert!(enqueue(&app,&cfg,&AtomicBool::new(true)).unwrap().is_none());drop(app);let app=crate::initialize(t.path().join("state")).unwrap();let j=enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().unwrap();app.db.lock().unwrap().execute("UPDATE jobs SET state='paused' WHERE id=?1",[&j]).unwrap();pending(&app,&cfg,vec![Change{scope:"B".into(),recursive:false}]);assert!(enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().is_none());app.db.lock().unwrap().execute("UPDATE jobs SET state='cancelled' WHERE id=?1",[&j]).unwrap();assert!(enqueue(&app,&cfg,&AtomicBool::new(false)).unwrap().is_none());let s=status(&app.db.lock().unwrap(),"r").unwrap();assert_eq!(s["enabled"],false);let enabled=configure(&app.db,"r",Edit{revision:s["revision"].as_i64().unwrap(),enabled:true,exclude:vec![]}).unwrap();assert_eq!(enabled["job_id"],Value::Null);}
 #[test]fn overflow_requires_manual_scan_and_stale_settings_are_rejected(){let(_t,app,cfg)=fixture();let c=app.db.lock().unwrap();save_hints(&c,&cfg,(0..129).map(|i|Change{scope:format!("folder-{i}"),recursive:false}).collect()).unwrap();let s=status(&c,"r").unwrap();assert_eq!(s["pending"],0);assert_eq!(s["needs_scan"],true);drop(c);configure(&app.db,"r",Edit{revision:1,enabled:false,exclude:vec![]}).unwrap();assert!(configure(&app.db,"r",Edit{revision:1,enabled:true,exclude:vec![]}).is_err());assert!(configure(&app.db,"r",Edit{revision:2,enabled:true,exclude:vec!["../escape".into()]}).is_err());}
}

//! Reviewed reversal of completed organization plans. File bytes and manual metadata remain intact.
use crate::{db::{Db,id},plans::{self,Item},scan};
use rusqlite::{Connection,params,OptionalExtension};
use serde::{Serialize,Deserialize};
use serde_json::Value;
use std::path::Path;
type R<T>=Result<T,String>;
fn sql<T>(result:rusqlite::Result<T>)->R<T>{result.map_err(|e|e.to_string())}
#[derive(Clone,Serialize,Deserialize,Debug,PartialEq)]
pub struct Origin{pub resource_id:String,pub root_id:String,pub root_path:String,pub relative_path:String}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS plan_origins(plan_id TEXT PRIMARY KEY REFERENCES plans(id),snapshot TEXT NOT NULL); CREATE TABLE IF NOT EXISTS plan_reversals(plan_id TEXT PRIMARY KEY REFERENCES plans(id),original_plan_id TEXT NOT NULL REFERENCES plans(id)); CREATE INDEX IF NOT EXISTS reversals_original ON plan_reversals(original_plan_id);")}
pub fn capture(c:&Connection,rid:&str)->R<Origin>{sql(c.query_row("SELECT r.root_id,rt.path,r.relative_path FROM resources r JOIN roots rt ON rt.id=r.root_id WHERE r.id=?1",[rid],|r|Ok(Origin{resource_id:rid.into(),root_id:r.get(0)?,root_path:r.get(1)?,relative_path:r.get(2)?})))}
pub fn store(c:&Connection,pid:&str,origin:&Origin)->R<()>{sql(c.execute("INSERT INTO plan_origins(plan_id,snapshot) VALUES(?1,?2)",params![pid,serde_json::to_string(origin).map_err(|e|e.to_string())?]))?;Ok(())}
fn origin(c:&Connection,pid:&str)->R<Origin>{let raw:Option<String>=sql(c.query_row("SELECT snapshot FROM plan_origins WHERE plan_id=?1",[pid],|r|r.get(0)).optional())?;serde_json::from_str(&raw.ok_or("This older plan has no original-location snapshot and cannot be undone automatically.")?).map_err(|e|e.to_string())}
fn current_resource(c:&Connection,parent:&str,o:&Origin)->R<()>{
 let(root,relative):(String,String)=sql(c.query_row("SELECT root_path,relative_path FROM plan_locations WHERE plan_id=?1 AND resource_id=?2",params![parent,o.resource_id],|r|Ok((r.get(0)?,r.get(1)?))))?;
 let current=capture(c,&o.resource_id)?;if current.root_path!=root||current.relative_path!=relative{return Err("The resource moved again. Undo its latest move first.".into());}
 let saved_root:Option<String>=sql(c.query_row("SELECT path FROM roots WHERE id=?1",[&o.root_id],|r|r.get(0)).optional())?;
 if saved_root.as_deref()!=Some(&o.root_path){return Err("The original source mapping changed. Automatic undo cannot use the old paths.".into());}
 plans::validate_chain(Path::new(&o.root_path))?;if !Path::new(&o.root_path).is_dir(){return Err("Reconnect the original source folder before undoing this move.".into());}
 let conflict:i64=sql(c.query_row("SELECT count(*) FROM resources WHERE root_id=?1 AND relative_path=?2 AND id!=?3 AND manual_group=0 AND (SELECT manual_group FROM resources WHERE id=?3)=0",params![o.root_id,o.relative_path,o.resource_id],|r|r.get(0)))?;
 if conflict>0{return Err("Another catalog resource occupies the original location. Resolve it before undoing this move.".into());}Ok(())
}
fn membership(c:&Connection,o:&Origin,items:&[Item])->R<()>{
 let mut s=sql(c.prepare("SELECT file_id FROM resource_files WHERE resource_id=?1 ORDER BY file_id"))?;
 let actual=sql(sql(s.query_map([&o.resource_id],|r|r.get::<_,String>(0)))?.collect::<rusqlite::Result<Vec<_>>>())?;
 let mut expected:Vec<_>=items.iter().map(|i|i.file_id.clone()).collect();expected.sort();
 if actual!=expected{return Err("Resource membership changed. Review its files before undoing this move.".into());}Ok(())
}
fn not_reversed(c:&Connection,parent:&str,pid:&str)->R<()>{
 let state:String=sql(c.query_row("SELECT state FROM plans WHERE id=?1 AND kind='organize'",[parent],|r|r.get(0)))?;
 if state!="completed"{return Err("Only a completed organization plan can be undone. Finish its pending operations first.".into());}
 let n:i64=sql(c.query_row("SELECT count(*) FROM plan_reversals r JOIN plans p ON p.id=r.plan_id WHERE r.original_plan_id=?1 AND r.plan_id!=?2 AND p.state IN ('executing','partial','completed')",params![parent,pid],|r|r.get(0)))?;
 if n>0{return Err("This move already has an active or completed undo plan. Open that plan instead.".into());}Ok(())
}
pub fn prepare(db:&Db,parent:&str)->R<Value>{
 let(o,old)={let c=db.lock().map_err(|e|e.to_string())?;crate::roots::require_idle(&c)?;not_reversed(&c,parent,"")?;let o=origin(&c,parent)?;current_resource(&c,parent,&o)?;
  let pending:i64=sql(c.query_row("SELECT count(*) FROM plan_locations l JOIN plans p ON p.id=l.plan_id WHERE l.resource_id=?1 AND p.state IN ('approved','executing','partial')",[&o.resource_id],|r|r.get(0)))?;if pending>0{return Err("Finish the existing approved file plan for this resource first.".into());}
  let(payload,digest):(String,String)=sql(c.query_row("SELECT items,digest FROM plans WHERE id=?1",[parent],|r|Ok((r.get(0)?,r.get(1)?))))?;
  use sha2::{Digest,Sha256};if hex::encode(Sha256::digest(payload.as_bytes()))!=digest{return Err("Original plan integrity check failed".into());}
  let old:Vec<Item>=serde_json::from_str(&payload).map_err(|e|e.to_string())?;membership(&c,&o,&old)?;(o,old)};
 let mut items=Vec::new();for item in old{
  let source=Path::new(&item.target);let target=Path::new(&item.source);plans::validate_chain(source)?;plans::validate_chain(target)?;
  if target.exists(){return Err(format!("Original location is occupied; undo cannot overwrite it: {}",target.display()));}
  let(size,hash)=plans::hash(source)?;if size!=item.size||hash!=item.sha256{return Err("A moved file changed after the original preview. Automatic undo cannot accept changed content.".into());}
  if !target.starts_with(scan::canonical(Path::new(&o.root_path))?){return Err("Original target is outside its recorded source root".into());}
  items.push(Item{id:id(),file_id:item.file_id,source:item.target,target:item.source,sha256:item.sha256,size:item.size,retained:None});
 }
 let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;crate::roots::require_idle(&tx)?;not_reversed(&tx,parent,"")?;current_resource(&tx,parent,&o)?;membership(&tx,&o,&items)?;
 let plan=plans::store_in(&tx,"undo_organize",items)?;let pid=plan["id"].as_str().unwrap();
 sql(tx.execute("INSERT INTO plan_locations(plan_id,resource_id,root_path,relative_path) VALUES(?1,?2,?3,?4)",params![pid,o.resource_id,o.root_path,o.relative_path]))?;
 sql(tx.execute("INSERT INTO plan_reversals(plan_id,original_plan_id) VALUES(?1,?2)",params![pid,parent]))?;
 validate(&tx,pid,&serde_json::from_value::<Vec<Item>>(plan["items"].clone()).map_err(|e|e.to_string())?)?;sql(tx.commit())?;Ok(plan)
}
/// Called before accepting execution, including retries after an interrupted move.
pub fn validate(c:&Connection,pid:&str,items:&[Item])->R<()>{
 let parent:String=sql(c.query_row("SELECT original_plan_id FROM plan_reversals WHERE plan_id=?1",[pid],|r|r.get(0)))?;not_reversed(c,&parent,pid)?;let o=origin(c,&parent)?;current_resource(c,&parent,&o)?;membership(c,&o,items)?;
 for item in items{
  let(path,availability):(String,String)=sql(c.query_row("SELECT path,availability FROM files WHERE id=?1",[&item.file_id],|r|Ok((r.get(0)?,r.get(1)?))))?;
  let operation:Option<String>=sql(c.query_row("SELECT state FROM operations WHERE id=?1 AND plan_id=?2",params![item.id,pid],|r|r.get(0)).optional())?;
  if availability!="present"||!(path==item.source||operation.is_some()&&path==item.target){return Err("A selected file moved or became unavailable. Review the current source before undoing.".into());}
  let collision:i64=sql(c.query_row("SELECT count(*) FROM files WHERE root_id=?1 AND path=?2 AND id!=?3",params![o.root_id,item.target,item.file_id],|r|r.get(0)))?;
  if collision>0{return Err("Another catalog file occupies an original target path.".into());}
 }Ok(())
}

#[cfg(test)]mod tests{
 use super::*;use std::{fs,path::PathBuf};
 fn fixture()->(tempfile::TempDir,Db,String,PathBuf,PathBuf,Value){
  let t=tempfile::tempdir().unwrap();let source=t.path().join("source");let managed=t.path().join("managed");fs::create_dir_all(source.join("Game/extras")).unwrap();fs::create_dir(&managed).unwrap();
  fs::write(source.join("Game/main.bin"),b"Main fixture bytes").unwrap();fs::write(source.join("Game/extras/readme.txt"),b"Readme fixture bytes").unwrap();
  let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Original source')",[source.to_str().unwrap()]).unwrap();
  let scan=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),scan);
  let rid:String={let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,notes) VALUES('w','Display title','Original title','Keep my notes'); UPDATE resources SET work_id='w';").unwrap();c.query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap()};
  let p=plans::organize(&db,&rid,managed.to_str().unwrap()).unwrap();plans::approve(&db,p["id"].as_str().unwrap(),p["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&db,p["id"].as_str().unwrap()).unwrap()["failed"],0);(t,db,rid,source,managed,p)
 }
 fn approve(db:&Db,p:&Value){plans::approve(db,p["id"].as_str().unwrap(),p["digest"].as_str().unwrap()).unwrap();}
 fn assert_restored(db:&Db,rid:&str,source:&Path,original:&Value){
  let location=capture(&db.lock().unwrap(),rid).unwrap();assert_eq!(location.root_id,"r");assert_eq!(location.relative_path,"Game");
  for i in original["items"].as_array().unwrap(){assert!(Path::new(i["source"].as_str().unwrap()).is_file());assert!(!Path::new(i["target"].as_str().unwrap()).exists());let row:(String,String)=db.lock().unwrap().query_row("SELECT path,root_id FROM files WHERE id=?1",[i["file_id"].as_str().unwrap()],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(row.0,i["source"].as_str().unwrap());assert_eq!(row.1,"r");}
  assert_eq!(fs::read(source.join("Game/main.bin")).unwrap(),b"Main fixture bytes");assert_eq!(fs::read(source.join("Game/extras/readme.txt")).unwrap(),b"Readme fixture bytes");assert_eq!(db.lock().unwrap().query_row("SELECT notes FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Keep my notes");
 }
 #[test]fn undo_requires_approval_and_restores_paths_ids_and_resource_index(){
  let(_t,db,rid,source,_managed,p)=fixture();let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();let id=reverse["id"].as_str().unwrap();assert!(plans::execute(&db,id).is_err());assert!(!source.join("Game/main.bin").exists());approve(&db,&reverse);assert_eq!(plans::execute(&db,id).unwrap()["failed"],0);assert_restored(&db,&rid,&source,&p);assert!(prepare(&db,p["id"].as_str().unwrap()).is_err());
  let scan=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),scan);assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM files",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM resources",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn undo_preview_rejects_changed_bytes_occupied_targets_and_legacy_history(){
  for reason in ["changed","occupied","legacy"]{let(_t,db,_rid,_source,_managed,p)=fixture();let item=&p["items"][0];match reason{"changed"=>fs::write(item["target"].as_str().unwrap(),b"New external bytes").unwrap(),"occupied"=>fs::write(item["source"].as_str().unwrap(),b"Other file").unwrap(),_=>{db.lock().unwrap().execute("DELETE FROM plan_origins WHERE plan_id=?1",[p["id"].as_str().unwrap()]).unwrap();}}
   assert!(prepare(&db,p["id"].as_str().unwrap()).is_err(),"{reason}");assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM plans",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert!(Path::new(item["target"].as_str().unwrap()).exists());
  }
 }
 #[test]fn undo_conflict_after_approval_is_recoverable_without_overwrite(){
  let(_t,db,rid,source,_managed,p)=fixture();let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);let target=PathBuf::from(reverse["items"][0]["target"].as_str().unwrap());assert!(target.starts_with(source.canonicalize().unwrap()));fs::write(&target,b"Unrelated new file").unwrap();let id=reverse["id"].as_str().unwrap();assert_eq!(plans::execute(&db,id).unwrap()["failed"],1);assert_eq!(fs::read(&target).unwrap(),b"Unrelated new file");
  fs::remove_file(&target).unwrap();assert_eq!(plans::execute(&db,id).unwrap()["failed"],0);assert_restored(&db,&rid,&source,&p);
 }
 #[test]fn undo_recovers_filesystem_commit_before_catalog_commit(){
  let(t,db,rid,source,_managed,p)=fixture();let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);let item:Item=serde_json::from_value(reverse["items"][0].clone()).unwrap();let pid=reverse["id"].as_str().unwrap();db.lock().unwrap().execute("INSERT INTO operations(id,plan_id,source,target,state,fingerprint) VALUES(?1,?2,?3,?4,'prepared',?5)",params![item.id,pid,item.source,item.target,item.sha256]).unwrap();plans::no_replace_move(Path::new(&item.source),Path::new(&item.target)).unwrap();drop(db);
  let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();assert_eq!(plans::execute(&db,pid).unwrap()["failed"],0);assert_restored(&db,&rid,&source,&p);
 }
 #[test]fn sequential_moves_must_be_undone_from_the_latest_location(){
  let(_t,db,rid,source,next,p)=fixture();db.lock().unwrap().execute("UPDATE works SET original_title='Second title'",[]).unwrap();let second=plans::organize(&db,&rid,next.to_str().unwrap()).unwrap();approve(&db,&second);assert_eq!(plans::execute(&db,second["id"].as_str().unwrap()).unwrap()["failed"],0);assert!(prepare(&db,p["id"].as_str().unwrap()).unwrap_err().contains("latest move"));
  let reverse=prepare(&db,second["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);assert_eq!(plans::execute(&db,reverse["id"].as_str().unwrap()).unwrap()["failed"],0);let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);assert_eq!(plans::execute(&db,reverse["id"].as_str().unwrap()).unwrap()["failed"],0);assert_restored(&db,&rid,&source,&p);
 }
 #[test]fn membership_changes_after_preview_block_execution_before_moves(){
  let(_t,db,rid,_source,_managed,p)=fixture();let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);db.lock().unwrap().execute("DELETE FROM resource_files WHERE resource_id=?1 AND file_id=?2",params![rid,p["items"][0]["file_id"].as_str().unwrap()]).unwrap();assert!(plans::execute(&db,reverse["id"].as_str().unwrap()).unwrap_err().contains("membership"));for item in p["items"].as_array().unwrap(){assert!(Path::new(item["target"].as_str().unwrap()).exists());}
 }
 #[test]fn competing_approved_previews_cannot_undo_twice(){
  let(_t,db,rid,source,_managed,p)=fixture();let a=prepare(&db,p["id"].as_str().unwrap()).unwrap();let b=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&a);approve(&db,&b);assert_eq!(plans::execute(&db,a["id"].as_str().unwrap()).unwrap()["failed"],0);assert!(plans::execute(&db,b["id"].as_str().unwrap()).unwrap_err().contains("completed undo"));assert_restored(&db,&rid,&source,&p);
 }
 #[test]fn v6_migration_preserves_history_without_inventing_origins(){
  let(t,db,_rid,_source,_managed,p)=fixture();db.lock().unwrap().execute_batch("DROP TABLE plan_reversals; DROP TABLE plan_origins; PRAGMA user_version=6;").unwrap();drop(db);let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();assert_eq!(db.lock().unwrap().query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);assert_eq!(db.lock().unwrap().query_row("SELECT state FROM plans WHERE id=?1",[p["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).unwrap(),"completed");assert!(prepare(&db,p["id"].as_str().unwrap()).unwrap_err().contains("older plan"));assert_eq!(fs::read_dir(t.path().join("state/migration-backups")).unwrap().count(),1);
 }
 #[test]fn single_archive_restores_its_original_filename_resource(){
  let t=tempfile::tempdir().unwrap();let source=t.path().join("source");let managed=t.path().join("managed");fs::create_dir(&source).unwrap();fs::create_dir(&managed).unwrap();fs::write(source.join("story.zip"),b"Archive fixture").unwrap();let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Archive source')",[source.to_str().unwrap()]).unwrap();let j=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),j);
  let rid:String={let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Story','Story'); UPDATE resources SET work_id='w';").unwrap();c.query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap()};let p=plans::organize(&db,&rid,managed.to_str().unwrap()).unwrap();approve(&db,&p);plans::execute(&db,p["id"].as_str().unwrap()).unwrap();let reverse=prepare(&db,p["id"].as_str().unwrap()).unwrap();approve(&db,&reverse);assert_eq!(plans::execute(&db,reverse["id"].as_str().unwrap()).unwrap()["failed"],0);assert_eq!(capture(&db.lock().unwrap(),&rid).unwrap().relative_path,"story.zip");assert_eq!(fs::read(source.join("story.zip")).unwrap(),b"Archive fixture");
 }
 #[test]fn changed_original_root_mapping_blocks_undo(){
  let(_t,db,_rid,_source,managed,p)=fixture();db.lock().unwrap().execute("UPDATE roots SET path=?1 WHERE id='r'",[managed.join("different-source").to_str().unwrap()]).unwrap();assert!(prepare(&db,p["id"].as_str().unwrap()).unwrap_err().contains("mapping changed"));for item in p["items"].as_array().unwrap(){assert!(Path::new(item["target"].as_str().unwrap()).exists());}
 }
}

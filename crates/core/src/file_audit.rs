//! State-transition audit. Triggers share the mutation's transaction, including
//! recovery paths, and deliberately exclude paths, raw errors and transfer specs.
use rusqlite::Connection;

pub fn migrate(c:&Connection, previous:i64)->rusqlite::Result<()> {
 c.execute_batch(r#"
 CREATE TRIGGER IF NOT EXISTS audit_file_plan_insert AFTER INSERT ON plans BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES('','file.plan.created',json_object('plan_id',NEW.id,'kind',NEW.kind,'state',NEW.state,'files',CASE WHEN json_valid(NEW.items) THEN json_array_length(NEW.items) ELSE NULL END),unixepoch());
 END;
 CREATE TRIGGER IF NOT EXISTS audit_file_plan_state AFTER UPDATE OF state ON plans WHEN OLD.state IS NOT NEW.state BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES('','file.plan.state',json_object('plan_id',NEW.id,'kind',NEW.kind,'from',OLD.state,'to',NEW.state,'undo_of',(SELECT original_plan_id FROM plan_reversals WHERE plan_id=NEW.id),'failed',(SELECT count(*) FROM operations WHERE plan_id=NEW.id AND state='conflict')),unixepoch());
 END;
 CREATE TRIGGER IF NOT EXISTS audit_file_operation_insert AFTER INSERT ON operations BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES('','file.operation.recorded',json_object('plan_id',NEW.plan_id,'operation_id',NEW.id,'kind',(SELECT kind FROM plans WHERE id=NEW.plan_id),'state',NEW.state),unixepoch());
 END;
 CREATE TRIGGER IF NOT EXISTS audit_file_operation_state AFTER UPDATE OF state,error ON operations WHEN OLD.state IS NOT NEW.state OR OLD.error IS NOT NEW.error BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES('','file.operation.state',json_object('plan_id',NEW.plan_id,'operation_id',NEW.id,'kind',(SELECT kind FROM plans WHERE id=NEW.plan_id),'from',OLD.state,'to',NEW.state),unixepoch());
 END;
 CREATE TRIGGER IF NOT EXISTS audit_file_transfer_insert AFTER INSERT ON jobs WHEN NEW.kind='acquire' BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES(NEW.id,'file.transfer.created',json_object('state',NEW.state),unixepoch());
 END;
 CREATE TRIGGER IF NOT EXISTS audit_file_transfer_state AFTER UPDATE OF state ON jobs WHEN NEW.kind='acquire' AND OLD.state IS NOT NEW.state BEGIN
  INSERT INTO events(job_id,code,payload,created) VALUES(NEW.id,'file.transfer.state',json_object('from',OLD.state,'to',NEW.state,'processed',NEW.processed,'bytes',NEW.bytes,'failed',NEW.errors),unixepoch());
 END;
 "#)?;
 if previous>0 && previous<41 {
  let plans:i64=c.query_row("SELECT count(*) FROM plans",[],|r|r.get(0))?;
  let transfers:i64=c.query_row("SELECT count(*) FROM jobs WHERE kind='acquire'",[],|r|r.get(0))?;
  if plans+transfers>0 {crate::db::event(c,"","file.audit.enabled",&serde_json::json!({"existing_plans":plans,"existing_transfers":transfers}))?;}
 }
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::{db,plans,scan,timeline};
 use serde_json::{json,Value};
 fn events(c:&Connection)->Vec<Value>{timeline::page(c,None).unwrap()["items"].as_array().unwrap().iter().filter(|e|e["code"].as_str().unwrap().starts_with("file.")).cloned().collect()}

 #[test]
 fn migration_marks_legacy_boundary_and_records_transitions_atomically(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");
  let database=db::open(&path).unwrap();{
   let c=database.lock().unwrap();
   for name in ["audit_file_plan_insert","audit_file_plan_state","audit_file_operation_insert","audit_file_operation_state","audit_file_transfer_insert","audit_file_transfer_state"]{c.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();}
   c.execute_batch("INSERT INTO plans(id,kind,state,items,digest,created) VALUES('old','organize','ready','[]','old-digest',1); PRAGMA user_version=40;").unwrap();
  }drop(database);
  let database=db::open(&path).unwrap();{
   let mut c=database.lock().unwrap();let before=events(&c);assert_eq!(before.len(),1);assert_eq!(before[0]["code"],"file.audit.enabled");assert_eq!(before[0]["details"]["existing_plans"],1);
   c.execute_batch("CREATE TRIGGER reject_audit BEFORE INSERT ON events WHEN NEW.code='file.plan.state' BEGIN SELECT RAISE(ABORT,'generated audit rejection'); END;").unwrap();
   assert!(c.execute("UPDATE plans SET state='approved' WHERE id='old'",[]).is_err());
   assert_eq!(c.query_row("SELECT state FROM plans WHERE id='old'",[],|r|r.get::<_,String>(0)).unwrap(),"ready");
   c.execute_batch("DROP TRIGGER reject_audit").unwrap();
   {let tx=c.transaction().unwrap();tx.execute("UPDATE plans SET state='approved' WHERE id='old'",[]).unwrap();}
   assert_eq!(events(&c),before);
   c.execute("UPDATE plans SET state='approved' WHERE id='old'",[]).unwrap();
   assert_eq!(events(&c)[0]["details"]["to"],"approved");
  }
  assert_eq!(std::fs::read_dir(path.parent().unwrap().join("migration-backups")).unwrap().count(),1);
 }

 #[test]
 fn real_quarantine_audit_failure_after_rename_is_recoverable_and_restorable(){
  let t=tempfile::tempdir().unwrap();let source=t.path().join("source");let quarantine=t.path().join("quarantine");std::fs::create_dir(&source).unwrap();std::fs::create_dir(&quarantine).unwrap();
  for name in ["a.zip","b.zip"]{std::fs::write(source.join(name),b"generated equal content").unwrap();}
  let database=db::open(&t.path().join("state/library.sqlite")).unwrap();database.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated source')",[source.to_str().unwrap()]).unwrap();
  let job=scan::create(&database,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(database.clone(),job);
  let files:Vec<String>={let c=database.lock().unwrap();let mut s=c.prepare("SELECT id FROM files ORDER BY path").unwrap();let ids=s.query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();ids};
  let plan=plans::isolate(&database,&files[0],&files[1],quarantine.to_str().unwrap()).unwrap();let pid=plan["id"].as_str().unwrap();let target=std::path::Path::new(plan["items"][0]["target"].as_str().unwrap());
  plans::approve(&database,pid,plan["digest"].as_str().unwrap()).unwrap();
  database.lock().unwrap().execute_batch("CREATE TRIGGER reject_audit BEFORE INSERT ON events WHEN NEW.code='file.operation.state' AND json_extract(NEW.payload,'$.to')='completed' BEGIN SELECT RAISE(ABORT,'generated audit rejection'); END;").unwrap();
  assert_eq!(plans::execute(&database,pid).unwrap()["state"],"partial");
  assert!(!source.join("a.zip").exists());assert_eq!(std::fs::read(target).unwrap(),b"generated equal content");assert_eq!(std::fs::read(source.join("b.zip")).unwrap(),b"generated equal content");
  {let c=database.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM operations WHERE plan_id=?1",[pid],|r|r.get::<_,String>(0)).unwrap(),"conflict");c.execute_batch("DROP TRIGGER reject_audit").unwrap();}
  assert_eq!(plans::execute(&database,pid).unwrap()["state"],"completed");
  let restore=plans::restore(&database,plan["items"][0]["id"].as_str().unwrap()).unwrap();let rid=restore["id"].as_str().unwrap();plans::approve(&database,rid,restore["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&database,rid).unwrap()["state"],"completed");
  assert_eq!(std::fs::read(source.join("a.zip")).unwrap(),b"generated equal content");
  let resource:String=database.lock().unwrap().query_row("SELECT resource_id FROM resource_files WHERE file_id=?1",[&files[0]],|r|r.get(0)).unwrap();
  database.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('work','Generated work','Generated work')",[]).unwrap();
  let revision=database.lock().unwrap().query_row("SELECT revision FROM resources WHERE id=?1",[&resource],|r|r.get(0)).unwrap();
  crate::editions::bind_primary(&database,&resource,"work","Generated edition",revision).unwrap();
  let managed=t.path().join("managed");std::fs::create_dir(&managed).unwrap();
  let organize=plans::organize(&database,&resource,managed.to_str().unwrap()).unwrap();let organize_id=organize["id"].as_str().unwrap();plans::approve(&database,organize_id,organize["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&database,organize_id).unwrap()["state"],"completed");
  let undo=crate::undo::prepare(&database,organize_id).unwrap();let undo_id=undo["id"].as_str().unwrap();plans::approve(&database,undo_id,undo["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&database,undo_id).unwrap()["state"],"completed");
  assert_eq!(std::fs::read(source.join("a.zip")).unwrap(),b"generated equal content");
  let before=events(&database.lock().unwrap());assert!(before.iter().any(|e|e["details"]["to"]=="conflict"));assert!(before.iter().any(|e|e["details"]["kind"]=="restore"));assert!(before.iter().any(|e|e["details"]["kind"]=="undo_organize"&&e["details"]["undo_of"]==organize_id));assert!(!json!(before).to_string().contains("generated audit rejection"));
  let backup=crate::backup::export(&database,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let copy=db::open(&restored.join("library.sqlite")).unwrap();assert_eq!(events(&copy.lock().unwrap()),before);
 }

 #[test]
 fn publication_audit_failure_retries_from_receipt_without_copying_or_exposing_secrets(){
  let t=tempfile::tempdir().unwrap();let source=t.path().join("source");let dest=t.path().join("destination");std::fs::create_dir(&source).unwrap();std::fs::create_dir(&dest).unwrap();std::fs::write(source.join("sample.zip"),b"generated copy-only bytes").unwrap();
  let database=db::open(&t.path().join("state/library.sqlite")).unwrap();database.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated')",[source.to_str().unwrap()]).unwrap();
  let scan_job=scan::create(&database,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(database.clone(),scan_job);
  let resource:String=database.lock().unwrap().query_row("SELECT id FROM resources LIMIT 1",[],|r|r.get(0)).unwrap();let job=crate::acquire::create(&database,&resource,dest.to_str().unwrap(),false).unwrap();
  let target:String=database.lock().unwrap().query_row("SELECT json_extract(spec,'$.final_folder') FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();
  database.lock().unwrap().execute_batch("CREATE TRIGGER reject_completion BEFORE INSERT ON events WHEN NEW.code='file.transfer.state' AND json_extract(NEW.payload,'$.to')='completed' BEGIN SELECT RAISE(ABORT,'generated audit rejection'); END;").unwrap();
  crate::acquire::run(database.clone(),job.clone(),Some("generated-test-password".into()));
  let published=std::path::Path::new(&target).join("sample.zip");assert_eq!(std::fs::read(&published).unwrap(),b"generated copy-only bytes");
  {let c=database.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"failed");c.execute_batch("DROP TRIGGER reject_completion").unwrap();timeline::control(&c,&job,"resume").unwrap();}
  crate::acquire::run(database.clone(),job.clone(),None);
  let log=events(&database.lock().unwrap());let transitions:Vec<String>=log.iter().rev().filter(|e|e["job_id"]==job&&e["code"]=="file.transfer.state").map(|e|e["details"]["to"].as_str().unwrap().to_owned()).collect();assert_eq!(transitions,["running","failed","queued","running","completed"]);
  let serialized=json!(log).to_string();assert!(!serialized.contains("generated-test-password"));assert!(!serialized.contains("generated audit rejection"));assert!(!serialized.contains(&target));
  assert_eq!(std::fs::read(&published).unwrap(),std::fs::read(source.join("sample.zip")).unwrap());
  let backup=crate::backup::export(&database,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let copy=db::open(&restored.join("library.sqlite")).unwrap();assert_eq!(events(&copy.lock().unwrap()),log);
 }
}

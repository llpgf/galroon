//! Durable entity-level accounting; legacy jobs deliberately have no summary.
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value,json};

pub fn migrate(c:&Connection)->rusqlite::Result<()> {
 c.execute_batch("CREATE TABLE IF NOT EXISTS job_summary_jobs(job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE);
 CREATE TABLE IF NOT EXISTS job_summary_entries(job_id TEXT NOT NULL REFERENCES job_summary_jobs(job_id) ON DELETE CASCADE,dimension TEXT NOT NULL,subject TEXT NOT NULL,value TEXT NOT NULL,PRIMARY KEY(job_id,dimension,subject));
 CREATE TRIGGER IF NOT EXISTS job_summary_begin AFTER INSERT ON jobs WHEN NEW.kind IN ('scan','match') BEGIN INSERT INTO job_summary_jobs(job_id) VALUES(NEW.id); END;")
}
pub fn record(c:&Connection,job:&str,dimension:&str,subject:&str,value:&str)->rusqlite::Result<()> {
 c.execute("INSERT INTO job_summary_entries(job_id,dimension,subject,value) SELECT ?1,?2,?3,?4 WHERE EXISTS(SELECT 1 FROM job_summary_jobs WHERE job_id=?1) ON CONFLICT(job_id,dimension,subject) DO UPDATE SET value=excluded.value WHERE value<>excluded.value",params![job,dimension,subject,value])?;Ok(())
}
pub fn resource(c:&Connection,job:&str,id:&str)->rusqlite::Result<()> {
 let unbound:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1 AND work_id IS NULL)",[id],|r|r.get(0))?;
 record(c,job,"unmatched_resources",id,if unbound{"yes"}else{"no"})
}
pub fn read(c:&Connection,job:&str)->rusqlite::Result<Value> {
 if c.query_row("SELECT job_id FROM job_summary_jobs WHERE job_id=?1",[job],|r|r.get::<_,String>(0)).optional()?.is_none(){return Ok(Value::Null);}
 let mut result=json!({"version":1,"new_works":0,"new_editions":0,"new_resources":0,"new_files":0,"updated_files":0,"unmatched_resources":0,"needs_review":0,"skipped":0,"failed":0});
 let mut q=c.prepare("SELECT dimension,count(*) FROM job_summary_entries WHERE job_id=?1 AND value='yes' GROUP BY dimension")?;
 for row in q.query_map([job],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))? {let(k,n)=row?;if result.get(&k).is_some(){result[k]=json!(n);}}
 Ok(result)
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn legacy_migration_deduplication_and_backup_restore(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("state/library.sqlite");let db=crate::db::open(&path).unwrap();
  db.lock().unwrap().execute_batch("DROP TRIGGER job_summary_begin; DROP TABLE job_summary_entries; DROP TABLE job_summary_jobs; INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('old','scan','completed','{}',1,1); PRAGMA user_version=42;").unwrap();drop(db);
  let db=crate::db::open(&path).unwrap();{
   let mut c=db.lock().unwrap();assert!(read(&c,"old").unwrap().is_null());record(&c,"old","new_files","private path","yes").unwrap();assert!(read(&c,"old").unwrap().is_null());
   c.execute_batch("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('new','scan','paused','{}',2,2);").unwrap();
   for _ in 0..3 {record(&c,"new","new_files","private path","yes").unwrap();}
   {let tx=c.transaction().unwrap();record(&tx,"new","new_files","rolled back","yes").unwrap();}
   assert_eq!(read(&c,"new").unwrap()["new_files"],1);assert!(!crate::job_history::page(&c,None).unwrap().to_string().contains("private path"));
  }
  let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let c=Connection::open(restored.join("library.sqlite")).unwrap();assert_eq!(read(&c,"new").unwrap()["new_files"],1);assert!(read(&c,"old").unwrap().is_null());
  assert_eq!(std::fs::read_dir(path.parent().unwrap().join("migration-backups")).unwrap().count(),1);
 }
 #[test]fn scan_counts_entities_changes_and_resumed_entries(){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("source");std::fs::create_dir_all(root.join("Game")).unwrap();for name in ["a.bin","b.bin"]{std::fs::write(root.join("Game").join(name),b"fixture").unwrap();}std::fs::write(root.join("excluded.zip"),b"skip").unwrap();
  let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Fixture')",[root.to_str().unwrap()]).unwrap();
  let scan=||{let j=crate::scan::create(&db,crate::scan::ScanSpec{root_id:"r".into(),scope:String::new(),exclude:vec!["excluded.zip".into()]}).unwrap();crate::scan::run(db.clone(),j.clone());j};
  let first=scan();let summary=read(&db.lock().unwrap(),&first).unwrap();assert_eq!(summary["new_files"],2);assert_eq!(summary["new_resources"],1);assert_eq!(summary["unmatched_resources"],1);assert_eq!(summary["new_works"],0);assert_eq!(summary["skipped"],1);
  {let c=db.lock().unwrap();c.execute("UPDATE jobs SET state='queued' WHERE id=?1",[&first]).unwrap();c.execute("UPDATE frontier SET state='pending' WHERE job_id=?1",[&first]).unwrap();}crate::scan::run(db.clone(),first.clone());assert_eq!(read(&db.lock().unwrap(),&first).unwrap(),summary);
  std::fs::write(root.join("Game/a.bin"),b"changed fixture size").unwrap();std::fs::write(root.join("Game/c.bin"),b"new").unwrap();let second=scan();let summary=read(&db.lock().unwrap(),&second).unwrap();assert_eq!(summary["new_files"],1);assert_eq!(summary["updated_files"],1);assert_eq!(summary["new_resources"],0);assert_eq!(summary["unmatched_resources"],1);
 }
}

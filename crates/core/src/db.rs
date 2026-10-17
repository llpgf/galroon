use rusqlite::{Connection, params};
use std::{path::Path, sync::{Arc, Mutex}};
pub type Db = Arc<Mutex<Connection>>;
pub const SCHEMA_VERSION:i64=54;
pub fn open(path: &Path) -> anyhow_result::Result<Db> {
    if path.parent().is_some_and(|p|p.join("restore-incomplete").exists()) {return Err("This restore is incomplete. Restore the backup again to a new collection.".into());}
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let mut c = Connection::open(path)?;
    let version:i64=c.query_row("PRAGMA user_version",[],|r|r.get(0))?;
    if version>SCHEMA_VERSION {return Err("This collection was created by a newer Galroon. Update the app before opening it.".into());}
    let tables:i64=c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
    if version==0 && tables>0 {return Err("Unversioned database cannot be upgraded automatically".into());}
    if version>0 && version<SCHEMA_VERSION {
        // Online backup includes committed WAL pages; it must succeed before any schema writes.
        let folder=path.parent().ok_or("Database has no parent")?.join("migration-backups");
        crate::plans::validate_chain(&folder)?;std::fs::create_dir_all(&folder)?;
        let snapshot=folder.join(format!("schema-{version}-{}-{}.sqlite",now(),id()));
        c.backup(rusqlite::MAIN_DB,&snapshot,None)?;
        let saved=Connection::open(&snapshot)?;crate::backup::clean_credentials(&saved)?;
    }
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=OFF; PRAGMA busy_timeout=5000;")?;
    let tx=c.transaction()?;
    tx.execute_batch("
    CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS roots(id TEXT PRIMARY KEY,path TEXT NOT NULL UNIQUE,label TEXT NOT NULL,role TEXT NOT NULL DEFAULT 'source');
    CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,kind TEXT NOT NULL,state TEXT NOT NULL,spec TEXT NOT NULL,processed INTEGER NOT NULL DEFAULT 0,discovered INTEGER NOT NULL DEFAULT 0,bytes INTEGER NOT NULL DEFAULT 0,errors INTEGER NOT NULL DEFAULT 0,current_path TEXT NOT NULL DEFAULT '',message TEXT NOT NULL DEFAULT '',created INTEGER NOT NULL,updated INTEGER NOT NULL);
    CREATE INDEX IF NOT EXISTS jobs_created ON jobs(created);
    CREATE TABLE IF NOT EXISTS frontier(job_id TEXT NOT NULL REFERENCES jobs(id),path TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'pending',PRIMARY KEY(job_id,path));
    CREATE TABLE IF NOT EXISTS files(id TEXT PRIMARY KEY,root_id TEXT NOT NULL REFERENCES roots(id),path TEXT NOT NULL,relative_path TEXT NOT NULL,size INTEGER NOT NULL,mtime TEXT NOT NULL,ext TEXT NOT NULL,seen_job TEXT NOT NULL,availability TEXT NOT NULL DEFAULT 'present',hash TEXT,UNIQUE(root_id,path));
    CREATE TABLE IF NOT EXISTS observations(job_id TEXT NOT NULL,file_id TEXT NOT NULL,PRIMARY KEY(job_id,file_id));
    CREATE TABLE IF NOT EXISTS resources(id TEXT PRIMARY KEY,root_id TEXT NOT NULL REFERENCES roots(id),relative_path TEXT NOT NULL,title TEXT NOT NULL,kind TEXT NOT NULL,work_id TEXT,release_label TEXT NOT NULL DEFAULT 'Unclassified edition',UNIQUE(root_id,relative_path));
    CREATE TABLE IF NOT EXISTS resource_files(resource_id TEXT NOT NULL REFERENCES resources(id),file_id TEXT NOT NULL REFERENCES files(id),PRIMARY KEY(resource_id,file_id));
    CREATE TABLE IF NOT EXISTS works(id TEXT PRIMARY KEY,title TEXT NOT NULL,original_title TEXT NOT NULL,vndb_id TEXT,description TEXT NOT NULL DEFAULT '',cover TEXT NOT NULL DEFAULT '',developer TEXT NOT NULL DEFAULT '',released TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '[]',status TEXT NOT NULL DEFAULT 'none',favorite INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',source_json TEXT NOT NULL DEFAULT '{}',overrides TEXT NOT NULL DEFAULT '{}',revision INTEGER NOT NULL DEFAULT 1);
    CREATE INDEX IF NOT EXISTS files_root ON files(root_id);
    CREATE INDEX IF NOT EXISTS files_size ON files(size);
    CREATE TABLE IF NOT EXISTS plans(id TEXT PRIMARY KEY,kind TEXT NOT NULL,state TEXT NOT NULL,items TEXT NOT NULL,digest TEXT NOT NULL,created INTEGER NOT NULL,approved INTEGER);
    CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES plans(id),source TEXT NOT NULL,target TEXT NOT NULL,state TEXT NOT NULL,fingerprint TEXT NOT NULL,error TEXT NOT NULL DEFAULT '');
    CREATE TABLE IF NOT EXISTS plan_locations(plan_id TEXT PRIMARY KEY REFERENCES plans(id),resource_id TEXT NOT NULL REFERENCES resources(id),root_path TEXT NOT NULL,relative_path TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS quarantine(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL,original TEXT NOT NULL,current TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'isolated');
    CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT,job_id TEXT NOT NULL,code TEXT NOT NULL,payload TEXT NOT NULL,created INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY,role TEXT NOT NULL,created INTEGER NOT NULL,revoked INTEGER NOT NULL DEFAULT 0);
    ")?;
    let has_aliases:bool=tx.prepare("PRAGMA table_info(works)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?.iter().any(|n|n=="aliases");
    if !has_aliases {tx.execute("ALTER TABLE works ADD COLUMN aliases TEXT NOT NULL DEFAULT '[]'",[])?;}
    let has_merge:bool=tx.prepare("PRAGMA table_info(works)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?.iter().any(|n|n=="merged_into");
    if !has_merge {tx.execute("ALTER TABLE works ADD COLUMN merged_into TEXT REFERENCES works(id)",[])?;}
    // Schema 44: a remake that VNDB files as a release of the original work gets its own card,
    // identified by the VNDB work plus that release. One card per (vndb_id, vndb_release).
    let has_release:bool=tx.prepare("PRAGMA table_info(works)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?.iter().any(|n|n=="vndb_release");
    if !has_release {tx.execute("ALTER TABLE works ADD COLUMN vndb_release TEXT",[])?;}
    // Schema 54: when a work first got a play status (unix seconds). Works that already had one keep NULL.
    let has_status_since:bool=tx.prepare("PRAGMA table_info(works)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?.iter().any(|n|n=="status_since");
    if !has_status_since {tx.execute("ALTER TABLE works ADD COLUMN status_since INTEGER",[])?;}
    // Every writer (edits, Bangumi/VNDB pulls, imports) goes through these, so none can forget the date.
    // Only the step from no status to a status counts: later changes, or clearing and re-setting, keep the first date.
    tx.execute_batch("CREATE TRIGGER IF NOT EXISTS work_status_since_update AFTER UPDATE OF status ON works WHEN OLD.status='none' AND NEW.status<>'none' AND NEW.status_since IS NULL BEGIN UPDATE works SET status_since=CAST(strftime('%s','now') AS INTEGER) WHERE id=NEW.id; END;
    CREATE TRIGGER IF NOT EXISTS work_status_since_insert AFTER INSERT ON works WHEN NEW.status<>'none' AND NEW.status_since IS NULL BEGIN UPDATE works SET status_since=CAST(strftime('%s','now') AS INTEGER) WHERE id=NEW.id; END;")?;
    tx.execute_batch("DROP INDEX IF EXISTS work_vndb; CREATE UNIQUE INDEX IF NOT EXISTS work_vndb_release ON works(vndb_id,coalesce(vndb_release,'')) WHERE vndb_id IS NOT NULL AND vndb_id != '';")?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS grouping_history(id TEXT PRIMARY KEY,kind TEXT NOT NULL,source TEXT NOT NULL,target TEXT NOT NULL,resources TEXT NOT NULL,created INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS root_history(id TEXT PRIMARY KEY,root_id TEXT NOT NULL,old_path TEXT NOT NULL,new_path TEXT NOT NULL,created INTEGER NOT NULL);")?;
    crate::editions::migrate(&tx,version)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS hash_entries(job_id TEXT NOT NULL REFERENCES jobs(id),file_id TEXT NOT NULL REFERENCES files(id),path TEXT NOT NULL,size INTEGER NOT NULL,mtime TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'pending',sha256 TEXT,error TEXT NOT NULL DEFAULT '',attempts INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(job_id,file_id)); CREATE INDEX IF NOT EXISTS hash_job_state ON hash_entries(job_id,state);")?;
    crate::access::migrate(&tx,version)?;
    crate::watching::migrate(&tx)?;
    crate::undo::migrate(&tx)?;
    crate::membership::migrate(&tx)?;
    crate::context::migrate(&tx)?;
    crate::automatch::migrate(&tx)?;
    crate::custom_tags::migrate(&tx)?;
    crate::lists::migrate(&tx)?;
    crate::resource_page::migrate(&tx)?;
    crate::smart_lists::migrate(&tx)?;
    crate::smart_snapshots::migrate(&tx)?;
    crate::entity_tags::migrate(&tx)?;
    crate::relation_overrides::migrate(&tx)?;
    crate::relation_correction_store::migrate(&tx)?;
    crate::relation_correction_index::migrate(&tx,version<37)?;
    crate::entity_overrides::migrate(&tx)?;
    crate::artwork_references::migrate(&tx,version<35)?;
    crate::smart_updates::migrate(&tx)?;
    crate::resource_members_page::migrate(&tx)?;
    if version<40 {tx.execute("UPDATE smart_catalog_state SET revision=revision+1 WHERE singleton=1",[])?;}
    if version<19 {crate::custom_tags::migrate_names(&tx)?;}
    let violations:i64=tx.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get(0))?;
    if violations!=0{return Err("Catalog foreign-key validation failed during migration".into());}
    crate::job_summary::migrate(&tx)?;
    crate::backup::migrate(&tx)?;
    crate::issues::migrate(&tx)?;
    crate::file_audit::migrate(&tx,version)?;
    crate::suggestions::migrate(&tx)?;
    crate::bangumi::migrate(&tx)?;
    crate::vndb_sync::migrate(&tx)?;
    crate::catalog_changes::migrate(&tx)?;
    crate::collection_download::migrate(&tx,version<48)?;
    if version>0 && version<53 {
        tx.execute("UPDATE works SET status='none',revision=revision+1,overrides=json_remove(overrides,'$.status') WHERE status='backlog' AND NOT EXISTS(SELECT 1 FROM bangumi_links b WHERE b.work_id=works.id AND b.base_status=1) AND NOT EXISTS(SELECT 1 FROM vndb_sync_links v WHERE v.work_id=works.id AND v.base_status=5)",[])?;
        // SQLite's DEFAULT-only schema edit: https://www.sqlite.org/lang_altertable.html
        let schema:i64=tx.query_row("PRAGMA schema_version",[],|r|r.get(0))?;
        tx.pragma_update(None,"writable_schema",true)?;
        tx.execute("UPDATE sqlite_schema SET sql=replace(sql,?1,?2) WHERE type='table' AND name='works'",params!["status TEXT NOT NULL DEFAULT 'backlog'","status TEXT NOT NULL DEFAULT 'none'"])?;
        tx.pragma_update(None,"schema_version",schema+1)?;
        tx.pragma_update(None,"writable_schema",false)?;
        let integrity:String=tx.query_row("PRAGMA integrity_check",[],|r|r.get(0))?;
        if integrity!="ok" {return Err(format!("Catalog integrity validation failed during migration: {integrity}").into());}
    }
    tx.pragma_update(None,"user_version",SCHEMA_VERSION)?;
    tx.execute("INSERT INTO settings(key,value) VALUES('schema_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[SCHEMA_VERSION.to_string()])?;
    tx.commit()?;
    c.pragma_update(None,"foreign_keys",true)?;
    c.execute("UPDATE jobs SET state='interrupted',message='Core restarted. Resume when ready.' WHERE state IN ('queued','running','pausing','cancelling')", [])?;
    c.execute("UPDATE frontier SET state='pending' WHERE state='running'", [])?;
    Ok(Arc::new(Mutex::new(c)))
}
pub fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64 }
pub fn id() -> String { uuid::Uuid::new_v4().to_string() }
pub fn event(c:&Connection, job:&str, code:&str, payload:&serde_json::Value) -> rusqlite::Result<()> {
    c.execute("INSERT INTO events(job_id,code,payload,created) VALUES(?1,?2,?3,?4)",params![job,code,payload.to_string(),now()])?; Ok(())
}
pub mod anyhow_result { pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>; }
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn migrations_snapshot_before_changes_and_reject_newer_databases(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("state/library.sqlite");let db=open(&path).unwrap();{
 let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Saved work','Saved work'); INSERT INTO sessions(token_hash,role,created) VALUES('never-back-up-this-token','owner',1); DROP TABLE root_history; PRAGMA user_version=1;").unwrap();}drop(db);
 let db=open(&path).unwrap();let snapshots=std::fs::read_dir(path.parent().unwrap().join("migration-backups")).unwrap().collect::<std::io::Result<Vec<_>>>().unwrap();assert_eq!(snapshots.len(),1);let saved=Connection::open(snapshots[0].path()).unwrap();assert_eq!(saved.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(saved.query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Saved work");assert_eq!(saved.query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 db.lock().unwrap().pragma_update(None,"user_version",SCHEMA_VERSION+1).unwrap();drop(db);assert!(open(&path).is_err());let future=Connection::open(&path).unwrap();assert_eq!(future.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),SCHEMA_VERSION+1);
 }
 #[test]fn incomplete_restore_is_never_opened(){let t=tempfile::tempdir().unwrap();std::fs::write(t.path().join("restore-incomplete"),b"").unwrap();assert!(open(&t.path().join("library.sqlite")).is_err());assert!(!t.path().join("library.sqlite").exists());}
}






#[cfg(test)]mod pairing_migration_tests{
 use super::*;
 #[test]fn schema_44_pairing_codes_become_desktop_with_public_ids(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("collection.sqlite");let db=open(&path).unwrap();
  {let c=db.lock().unwrap();c.execute_batch("DROP TABLE pairing_codes; CREATE TABLE pairing_codes(hash TEXT PRIMARY KEY,name TEXT NOT NULL,expires INTEGER NOT NULL); INSERT INTO pairing_codes VALUES('first-secret-hash','Old desktop',9999999999),('second-secret-hash','Other desktop',9999999999); PRAGMA user_version=44; UPDATE settings SET value='44' WHERE key='schema_version';").unwrap();}drop(db);
  let db=open(&path).unwrap();let ids={let c=db.lock().unwrap();assert_eq!(SCHEMA_VERSION,54);assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),SCHEMA_VERSION);assert_eq!(c.query_row("SELECT value FROM settings WHERE key='schema_version'",[],|r|r.get::<_,String>(0)).unwrap(),SCHEMA_VERSION.to_string());
   let mut s=c.prepare("SELECT id,role,hash FROM pairing_codes ORDER BY name").unwrap();let rows=s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();assert_eq!(rows.len(),2);for(id,role,hash)in &rows{assert!(!id.is_empty());assert_ne!(id,hash);assert_eq!(role,"desktop");}assert_ne!(rows[0].0,rows[1].0);rows};drop(db);
  let db=open(&path).unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT id FROM pairing_codes WHERE name='Old desktop'",[],|r|r.get::<_,String>(0)).unwrap(),ids[0].0);
  let snapshots=std::fs::read_dir(t.path().join("migration-backups")).unwrap().collect::<std::io::Result<Vec<_>>>().unwrap();assert_eq!(snapshots.len(),1);let saved=Connection::open(snapshots[0].path()).unwrap();assert_eq!(saved.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),44);assert_eq!(saved.query_row("SELECT count(*) FROM pairing_codes",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
}

#[cfg(test)]mod play_status_migration_tests{
 use super::*;
 fn revisions(c:&Connection)->(i64,i64,i64){(crate::catalog_changes::seq(c).unwrap(),c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap(),c.query_row("SELECT collection_revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap())}
 #[test]fn schema_53_works_keep_no_first_status_date_until_they_get_one(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=open(&path).unwrap();
  db.lock().unwrap().execute_batch("DROP TRIGGER work_status_since_update; DROP TRIGGER work_status_since_insert; DROP TRIGGER catalog_changed_works_UPDATE; INSERT INTO works(id,title,original_title,status) VALUES('old','Old','Old','playing'),('fresh','Fresh','Fresh','none'); ALTER TABLE works DROP COLUMN status_since; PRAGMA user_version=53;").unwrap();drop(db);
  let db=open(&path).unwrap();let c=db.lock().unwrap();let since=|id:&str|c.query_row("SELECT status_since FROM works WHERE id=?1",[id],|r|r.get::<_,Option<i64>>(0)).unwrap();
  assert_eq!((since("old"),since("fresh")),(None,None));
  c.execute_batch("UPDATE works SET status='completed' WHERE id='old'; UPDATE works SET status='backlog' WHERE id='fresh';").unwrap();assert_eq!(since("old"),None);assert!(since("fresh").is_some());
 }
 #[test]fn schema_52_migrates_only_unsynced_backlog_and_changes_the_default(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=open(&path).unwrap();let before={let c=db.lock().unwrap();
   c.execute_batch("INSERT INTO works(id,title,original_title,status,overrides) VALUES('plain','Plain','Plain','backlog','{\"status\":\"backlog\",\"notes\":\"keep\"}'),('bgm','Bangumi','Bangumi','backlog','{\"status\":\"backlog\"}'),('vn','VNDB','VNDB','backlog','{}'),('unbased','Unbased','Unbased','backlog','{}'),('playing','Playing','Playing','playing','{}'),('merged','Merged','Merged','backlog','{}'); UPDATE works SET merged_into='playing' WHERE id='merged'; INSERT INTO bangumi_links(work_id,subject_id,base_status) VALUES('bgm',1,1),('unbased',2,NULL); INSERT INTO vndb_sync_links(work_id,subject_id,base_status) VALUES('vn',1,5); INSERT INTO bangumi_remote VALUES(2,1,'{}');").unwrap();
   let schema:i64=c.query_row("PRAGMA schema_version",[],|r|r.get(0)).unwrap();c.pragma_update(None,"writable_schema",true).unwrap();c.execute("UPDATE sqlite_schema SET sql=replace(sql,?1,?2) WHERE type='table' AND name='works'",params!["status TEXT NOT NULL DEFAULT 'none'","status TEXT NOT NULL DEFAULT 'backlog'"]).unwrap();c.pragma_update(None,"schema_version",schema+1).unwrap();c.pragma_update(None,"writable_schema",false).unwrap();c.pragma_update(None,"user_version",52).unwrap();c.execute("UPDATE settings SET value='52' WHERE key='schema_version'",[]).unwrap();revisions(&c)
  };drop(db);
  let db=open(&path).unwrap();let after={let c=db.lock().unwrap();
   for(id,status,revision)in[("plain","none",2),("bgm","backlog",1),("vn","backlog",1),("unbased","none",2),("playing","playing",1),("merged","none",2)]{assert_eq!(c.query_row("SELECT status,revision FROM works WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),(status.into(),revision));}
   assert_eq!(c.query_row("SELECT overrides FROM works WHERE id='plain'",[],|r|r.get::<_,String>(0)).unwrap(),"{\"notes\":\"keep\"}");assert_eq!(c.query_row("SELECT json_extract(overrides,'$.status') FROM works WHERE id='bgm'",[],|r|r.get::<_,String>(0)).unwrap(),"backlog");
   let after=revisions(&c);assert!(after.0>before.0&&after.1>before.1&&after.2>before.2);
   c.execute("INSERT INTO works(id,title,original_title) VALUES('new','New','New')",[]).unwrap();assert_eq!(c.query_row("SELECT status FROM works WHERE id='new'",[],|r|r.get::<_,String>(0)).unwrap(),"none");assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");assert_eq!(c.query_row("PRAGMA writable_schema",[],|r|r.get::<_,i64>(0)).unwrap(),0);
   c.execute("INSERT INTO works(id,title,original_title,status) VALUES('wanted','Wanted','Wanted','backlog')",[]).unwrap();revisions(&c)
  };drop(db);
  let db=open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(revisions(&c),after);assert_eq!(c.query_row("SELECT status FROM works WHERE id='wanted'",[],|r|r.get::<_,String>(0)).unwrap(),"backlog");assert_eq!(std::fs::read_dir(t.path().join("migration-backups")).unwrap().count(),1);
 }
}

use rusqlite::{Connection, params};
use std::{path::Path, sync::{Arc, Mutex}};
pub type Db = Arc<Mutex<Connection>>;
pub const SCHEMA_VERSION:i64=43;
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
    CREATE TABLE IF NOT EXISTS works(id TEXT PRIMARY KEY,title TEXT NOT NULL,original_title TEXT NOT NULL,vndb_id TEXT,description TEXT NOT NULL DEFAULT '',cover TEXT NOT NULL DEFAULT '',developer TEXT NOT NULL DEFAULT '',released TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '[]',status TEXT NOT NULL DEFAULT 'backlog',favorite INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',source_json TEXT NOT NULL DEFAULT '{}',overrides TEXT NOT NULL DEFAULT '{}',revision INTEGER NOT NULL DEFAULT 1);
    CREATE UNIQUE INDEX IF NOT EXISTS work_vndb ON works(vndb_id) WHERE vndb_id IS NOT NULL AND vndb_id != '';
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






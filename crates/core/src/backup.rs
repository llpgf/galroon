//! Consistent catalog-only snapshots. Original game files are never copied.
use crate::{db::{Db,id,now},plans};
use rusqlite::{Connection,MAIN_DB,OpenFlags};
use serde::{Serialize,Deserialize};
use std::{path::{Path,PathBuf},fs,io::Write};
type R<T>=Result<T,String>;
/// Reserved export directory names are excluded even while a backup is being written.
pub fn protected_path(path:&Path)->bool{path.components().any(|part|{let name=part.as_os_str().to_string_lossy().to_ascii_lowercase();name.starts_with("galroon-backup-")||name.starts_with(".galroon-backup-")})}
#[derive(Serialize,Deserialize,Debug)]pub struct Manifest{pub format:String,pub version:u32,pub created:i64,pub database:String,pub bytes:u64,pub sha256:String,pub includes_game_files:bool,pub includes_credentials:bool,
 #[serde(default)]pub library:Option<crate::context::Library>,#[serde(default)]pub schema:Option<i64>,#[serde(default)]pub tables:Vec<String>,#[serde(default)]pub verification:String}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS backup_history(id TEXT PRIMARY KEY,path TEXT NOT NULL,created INTEGER NOT NULL,manifest TEXT NOT NULL);")}
fn tables(c:&Connection)->R<Vec<String>>{let mut q=c.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<String>>>().map_err(|e|e.to_string())?;Ok(rows)}
pub fn history(db:&Db)->R<serde_json::Value>{let c=db.lock().map_err(|e|e.to_string())?;let mut q=c.prepare("SELECT id,path,created,manifest FROM backup_history ORDER BY created DESC,rowid DESC LIMIT 50").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;Ok(serde_json::json!(rows.into_iter().map(|(id,path,created,manifest)|serde_json::json!({"id":id,"path":path,"created":created,"manifest":serde_json::from_str::<serde_json::Value>(&manifest).unwrap_or_default()})).collect::<Vec<_>>()))}
pub(crate) fn clean_credentials(c:&Connection)->R<()>{
 c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA secure_delete=ON; DELETE FROM sessions; DELETE FROM settings WHERE key NOT IN ('schema_version','organization.managed_root');").map_err(|e|e.to_string())?;
 for table in ["pairing_codes","access_attempts"]{let exists:i64=c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?1",[table],|r|r.get(0)).map_err(|e|e.to_string())?;if exists==1{c.execute(&format!("DELETE FROM {table}"),[]).map_err(|e|e.to_string())?;}}
 c.execute_batch("VACUUM;").map_err(|e|e.to_string())
}
fn check(c:&Connection)->R<()>{
 c.execute_batch("PRAGMA trusted_schema=OFF;").map_err(|e|e.to_string())?;
 let integrity:String=c.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;if integrity!="ok"{return Err("Backup database failed its integrity check".into());}
 let foreign:i64=c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;if foreign!=0{return Err("Backup database contains invalid references".into());}
 let schema:i64=c.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?;if !(1..=crate::db::SCHEMA_VERSION).contains(&schema){return Err("Unsupported backup database version".into());}
 for table in ["works","resources","files","roots","jobs","plans","operations","quarantine","sessions","settings"]{let n:i64=c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?1",[table],|r|r.get(0)).map_err(|e|e.to_string())?;if n!=1{return Err(format!("Backup is missing the {table} table"));}}
 Ok(())
}
pub fn export(db:&Db,destination:&Path)->R<PathBuf>{
 let parent=destination.canonicalize().map_err(|e|match e.kind(){std::io::ErrorKind::NotFound=>"Choose an existing backup destination folder".to_owned(),std::io::ErrorKind::PermissionDenied=>"Cannot access the backup destination folder. Check its permissions or choose another folder.".to_owned(),_=>format!("Could not access the backup destination folder: {e}")})?;plans::validate_chain(&parent)?;if !parent.is_dir(){return Err("Choose an existing backup destination folder".into());}
 let folder=parent.join(format!("Galroon-backup-{}-{}",now(),id()));fs::create_dir(&folder).map_err(|e|e.to_string())?;let file=folder.join("collection.sqlite");
 {let c=db.lock().map_err(|e|e.to_string())?;c.backup(MAIN_DB,&file,None).map_err(|e|e.to_string())?;}
 {let c=Connection::open(&file).map_err(|e|e.to_string())?;clean_credentials(&c)?;check(&c)?;}
 let(bytes,sha256)=plans::hash(&file)?;let c=Connection::open_with_flags(&file,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
 let manifest=Manifest{format:"galroon-catalog-backup".into(),version:2,created:now(),database:"collection.sqlite".into(),bytes,sha256,includes_game_files:false,includes_credentials:false,library:Some(crate::context::library(&c).map_err(|e|e.to_string())?),schema:Some(c.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?),tables:tables(&c)?,verification:"integrity-and-hash".into()};drop(c);
 let mut f=fs::OpenOptions::new().write(true).create_new(true).open(folder.join("manifest.json")).map_err(|e|e.to_string())?;f.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;f.sync_all().map_err(|e|e.to_string())?;drop(f);
 inspect(&folder)?;
 db.lock().map_err(|e|e.to_string())?.execute("INSERT INTO backup_history(id,path,created,manifest) VALUES(?1,?2,?3,?4)",rusqlite::params![id(),folder.to_string_lossy(),manifest.created,serde_json::to_string(&manifest).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(folder)
}
pub fn inspect(folder:&Path)->R<Manifest>{
 let folder=folder.canonicalize().map_err(|e|e.to_string())?;plans::validate_chain(&folder)?;let path=folder.join("manifest.json");plans::validate_chain(&path)?;if fs::metadata(&path).map_err(|e|e.to_string())?.len()>16384{return Err("Invalid backup manifest size".into());}
 let manifest:Manifest=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if manifest.format!="galroon-catalog-backup"||!(1..=2).contains(&manifest.version)||manifest.database!="collection.sqlite"||manifest.includes_game_files||manifest.includes_credentials{return Err("Unsupported backup format".into());}
 let file=folder.join(&manifest.database);plans::validate_chain(&file)?;let(size,hash)=plans::hash(&file)?;if size!=manifest.bytes||hash!=manifest.sha256{return Err("Backup content changed or is incomplete".into());}
 let c=Connection::open_with_flags(&file,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;check(&c)?;
 if manifest.version==2 {
  let schema:i64=c.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  if manifest.schema!=Some(schema)||manifest.library.as_ref()!=Some(&crate::context::library(&c).map_err(|e|e.to_string())?)||manifest.tables!=tables(&c)?||manifest.verification!="integrity-and-hash"{return Err("Backup manifest does not match its collection".into());}
 }
 Ok(manifest)
}
/// Restore to a new data directory; switching the running Core is a separate,
/// reviewed lifecycle operation. Never overwrite an existing collection here.
pub fn restore_new(folder:&Path,destination:&Path,expected_hash:Option<&str>)->R<PathBuf>{
 let manifest=inspect(folder)?;if expected_hash.is_some_and(|h|h!=manifest.sha256){return Err("Backup changed since preview. Inspect it again.".into());}plans::validate_chain(destination)?;if destination.exists(){return Err("Restore requires a new, unused data directory".into());}
 fs::create_dir(destination).map_err(|e|e.to_string())?;let marker=destination.join("restore-incomplete");fs::File::create(&marker).map_err(|e|e.to_string())?.sync_all().map_err(|e|e.to_string())?;let file=destination.join("library.sqlite");let mut source=fs::File::open(folder.join("collection.sqlite")).map_err(|e|e.to_string())?;let mut dest=fs::OpenOptions::new().create_new(true).write(true).open(&file).map_err(|e|e.to_string())?;std::io::copy(&mut source,&mut dest).map_err(|e|e.to_string())?;dest.sync_all().map_err(|e|e.to_string())?;drop(dest);
 let(size,hash)=plans::hash(&file)?;if size!=manifest.bytes||hash!=manifest.sha256{return Err("Backup changed during restore".into());}
 let c=Connection::open(&file).map_err(|e|e.to_string())?;check(&c)?;clean_credentials(&c)?;
 c.execute_batch("UPDATE jobs SET state='interrupted',message='Restored from backup; verify source paths before resuming.' WHERE state IN ('queued','running','pausing','cancelling'); UPDATE frontier SET state='pending' WHERE state='running'; UPDATE plans SET state='ready',approved=NULL WHERE state IN ('approved','executing','partial');").map_err(|e|e.to_string())?;
 // Materialize disabled entries even for legacy backups without watcher rows;
 // startup's ensure() must not recreate enabled watchers for restored roots.
 crate::watching::migrate(&c)?;crate::watching::ensure(&c)?;
 c.execute_batch("UPDATE source_watch SET enabled=0,pending='[]',job_id=NULL,needs_scan=1,status='disabled',revision=revision+1,message='Restored collection. Check source paths before enabling monitoring.';").map_err(|e|e.to_string())?;
 drop(c);fs::remove_file(marker).map_err(|e|e.to_string())?;destination.canonicalize().map_err(|e|e.to_string())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn invalid_destination_explains_recovery_without_creating_backup_history(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();
  let missing=t.path().join("missing");assert_eq!(export(&db,&missing).unwrap_err(),"Choose an existing backup destination folder");assert!(!missing.exists());
  let file=t.path().join("file");fs::write(&file,b"unchanged").unwrap();assert_eq!(export(&db,&file).unwrap_err(),"Choose an existing backup destination folder");assert_eq!(fs::read(file).unwrap(),b"unchanged");assert_eq!(history(&db).unwrap(),serde_json::json!([]));
  fs::create_dir(&missing).unwrap();let saved=export(&db,&missing).unwrap();assert_eq!(inspect(&saved).unwrap().verification,"integrity-and-hash");assert_eq!(history(&db).unwrap().as_array().unwrap().len(),1);
 }
 #[tokio::test]async fn restore_disables_watchers_and_preserves_history_without_replaying_jobs(){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("source");fs::create_dir(&root).unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();
  {let c=db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Fixture')",[root.to_string_lossy().to_string()]).unwrap();crate::watching::ensure(&c).unwrap();
   c.execute_batch("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('j','scan','queued','{}',1,1); UPDATE source_watch SET enabled=1,job_id='j',pending='[{\"scope\":\"\",\"recursive\":true}]'; INSERT INTO plans(id,kind,state,items,digest,created,approved) VALUES('p','move','approved','[]','fixture',1,1);").unwrap();}
  let folder=export(&db,t.path()).unwrap();let restored=restore_new(&folder,&t.path().join("restored"),None).unwrap();let app=crate::initialize(restored).unwrap();
  {let c=app.db.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM jobs WHERE id='j'",[],|r|r.get::<_,String>(0)).unwrap(),"interrupted");assert_eq!(c.query_row("SELECT state FROM plans WHERE id='p'",[],|r|r.get::<_,String>(0)).unwrap(),"ready");let watch=crate::watching::status(&c,"r").unwrap();assert_eq!(watch["enabled"],false);assert_eq!(watch["pending"],0);assert_eq!(watch["job_id"],serde_json::Value::Null);}
  let service=crate::watching::start(app.clone()).unwrap();fs::write(root.join("new.zip"),b"generated after restore").unwrap();tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
  assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),1);drop(service);
  // A legacy snapshot with no watcher table must also stay disabled on upgrade.
  db.lock().unwrap().execute_batch("DROP TABLE source_watch;").unwrap();let old=export(&db,t.path()).unwrap();let old_restored=restore_new(&old,&t.path().join("legacy"),None).unwrap();let old_app=crate::initialize(old_restored).unwrap();assert_eq!(crate::watching::status(&old_app.db.lock().unwrap(),"r").unwrap()["enabled"],false);
 }
 #[test]fn schema_twelve_migration_preserves_tags_and_creates_empty_history(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();let identity=crate::context::library(&db.lock().unwrap()).unwrap();
  db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Fixture','Fixture'); INSERT INTO custom_tags(id,name,name_key) VALUES('t','Private','private'); INSERT INTO custom_tag_works(tag_id,work_id) VALUES('t','w'); DROP TABLE backup_history; PRAGMA user_version=12;").unwrap();drop(db);
  let db=crate::db::open(&path).unwrap();assert_eq!(history(&db).unwrap(),serde_json::json!([]));assert_eq!(crate::context::library(&db.lock().unwrap()).unwrap(),identity);
  assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn exported_catalog_is_not_scanned_even_with_explicit_scope(){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("games");fs::create_dir(&root).unwrap();fs::write(root.join("sample.zip"),b"generated fixture").unwrap();let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();
  db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Fixture')",[root.to_string_lossy().to_string()]).unwrap();let folder=export(&db,&root).unwrap();
  let jid=crate::scan::create(&db,crate::scan::ScanSpec{root_id:"r".into(),scope:String::new(),exclude:vec![]}).unwrap();crate::scan::run(db.clone(),jid);
  assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM files",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  assert!(crate::scan::create(&db,crate::scan::ScanSpec{root_id:"r".into(),scope:folder.file_name().unwrap().to_string_lossy().into(),exclude:vec![]}).is_err());
  assert!(inspect(&folder).is_ok());
 }
 #[test]fn manifest_identity_history_and_legacy_compatibility(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();
  let folder=export(&db,t.path()).unwrap();let manifest=inspect(&folder).unwrap();assert_eq!(manifest.version,2);assert_eq!(manifest.schema,Some(crate::db::SCHEMA_VERSION));assert!(manifest.tables.contains(&"custom_tags".into()));assert_eq!(manifest.library.as_ref().unwrap(),&crate::context::library(&db.lock().unwrap()).unwrap());
  let h=history(&db).unwrap();assert_eq!(h.as_array().unwrap().len(),1);assert_eq!(h[0]["manifest"]["verification"],"integrity-and-hash");
  let path=folder.join("manifest.json");let original=fs::read(&path).unwrap();let mut forged:serde_json::Value=serde_json::from_slice(&original).unwrap();forged["library"]["id"]=serde_json::json!("different-library");fs::write(&path,serde_json::to_vec(&forged).unwrap()).unwrap();assert!(inspect(&folder).is_err());
  let mut legacy:serde_json::Value=serde_json::from_slice(&original).unwrap();legacy["version"]=serde_json::json!(1);for key in ["library","schema","tables","verification"]{legacy.as_object_mut().unwrap().remove(key);}fs::write(&path,serde_json::to_vec(&legacy).unwrap()).unwrap();assert_eq!(inspect(&folder).unwrap().version,1);
  assert!(export(&db,&t.path().join("unavailable")).is_err());assert_eq!(history(&db).unwrap().as_array().unwrap().len(),1);
 }
 #[test]fn backup_is_consistent_private_and_restore_never_overwrites(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();{let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,overrides) VALUES('w','Manual title','Original','{\"title\":\"Manual title\"}'); INSERT INTO sessions(token_hash,role,created) VALUES('test-session-secret','owner',1); INSERT INTO settings(key,value) VALUES('owner_password','test-password-secret');").unwrap();}
 let folder=export(&db,t.path()).unwrap();let m=inspect(&folder).unwrap();assert!(!m.includes_game_files);assert!(!m.includes_credentials);let raw=fs::read(folder.join("collection.sqlite")).unwrap();for secret in [b"test-session-secret".as_slice(),b"test-password-secret".as_slice()]{assert!(!raw.windows(secret.len()).any(|w|w==secret));}
 let restored=restore_new(&folder,&t.path().join("restored"),None).unwrap();let c=Connection::open(restored.join("library.sqlite")).unwrap();assert_eq!(c.query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Manual title");assert!(restore_new(&folder,&restored,None).is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 fs::OpenOptions::new().append(true).open(folder.join("collection.sqlite")).unwrap().write_all(b"corruption").unwrap();assert!(inspect(&folder).is_err());
 }
}



//! Catalog identity survives backup; machine identity belongs to the local installation.
use crate::{db, App, ApiError};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{fs, io::{Read, Write}, path::Path};
use axum::{extract::State, http::HeaderMap, Json};
use serde_json::{Value,json};
type R<T> = Result<T,String>;

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct Device {pub id:String,pub name:String,pub platform:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct Library {pub id:String,pub name:String,pub revision:i64}
pub fn migrate(c:&Connection)->rusqlite::Result<()> {
 c.execute_batch("CREATE TABLE IF NOT EXISTS library_identity(singleton INTEGER PRIMARY KEY CHECK(singleton=1),id TEXT NOT NULL UNIQUE,name TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1);")?;
 c.execute("INSERT OR IGNORE INTO library_identity(singleton,id,name) VALUES(1,?1,'My collection')",[db::id()])?;
 Ok(())
}
pub fn library(c:&Connection)->rusqlite::Result<Library>{c.query_row("SELECT id,name,revision FROM library_identity WHERE singleton=1",[],|r|Ok(Library{id:r.get(0)?,name:r.get(1)?,revision:r.get(2)?}))}
fn valid_name(value:&str)->R<String>{let s=value.trim();if s.is_empty()||s.chars().count()>80||s.chars().any(char::is_control){return Err("Choose a name between 1 and 80 characters".into());}Ok(s.into())}
pub fn rename(db:&db::Db,revision:i64,name:&str)->R<Library>{
 let name=valid_name(name)?;let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 let old=library(&tx).map_err(|e|e.to_string())?;if revision!=old.revision{return Err("Collection name changed. Refresh before saving.".into());}
 if name!=old.name{tx.execute("UPDATE library_identity SET name=?1,revision=revision+1 WHERE singleton=1",[name]).map_err(|e|e.to_string())?;}
 let result=library(&tx).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(result)
}
pub fn device(dir:&Path)->R<Device>{
 crate::plans::validate_chain(dir)?;fs::create_dir_all(dir).map_err(|e|e.to_string())?;
 let lock_path=dir.join("device.lock");crate::plans::validate_chain(&lock_path)?;
 let lock=fs::OpenOptions::new().create(true).read(true).write(true).open(&lock_path).map_err(|e|e.to_string())?;
 fs2::FileExt::lock_exclusive(&lock).map_err(|e|e.to_string())?;
 let path=dir.join("device.json");crate::plans::validate_chain(&path)?;
 if !path.exists(){
  let name=std::env::var("COMPUTERNAME").ok().and_then(|n|valid_name(&n).ok()).unwrap_or_else(||"This computer".into());
  let value=Device{id:db::id(),name,platform:std::env::consts::OS.into()};
  let pending=dir.join(format!("device-{}.tmp",db::id()));let mut f=fs::OpenOptions::new().create_new(true).write(true).open(&pending).map_err(|e|e.to_string())?;
  f.write_all(&serde_json::to_vec(&value).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;f.sync_all().map_err(|e|e.to_string())?;drop(f);fs::rename(&pending,&path).map_err(|e|e.to_string())?;
 }
 let mut bytes=Vec::new();fs::File::open(&path).map_err(|e|e.to_string())?.take(4097).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>4096{return Err("Local device identity is too large".into());}
 let value:Device=serde_json::from_slice(&bytes).map_err(|_|"Local device identity is damaged; restore device.json before reconnecting".to_string())?;
 if uuid::Uuid::parse_str(&value.id).is_err()||valid_name(&value.name).is_err()||value.platform!=std::env::consts::OS{return Err("Local device identity is invalid".into());}Ok(value)
}
pub fn snapshot(a:&App,role:&str)->R<Value>{
 let library=library(&*a.db.lock().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 Ok(json!({"library":library,"core":{"device":a.device,"instance_id":a.instance_id,"version":env!("CARGO_PKG_VERSION"),"location":"local","transport":"loopback"},"api":{"min":1,"max":1},"capabilities":{"collection_paging":true,"work_scoped_reads":true,"edition_membership_edit":true,"resource_grouping_scoped":true,"resource_members_paging":true,"resource_preview_paging":true,"read_catalog":true,"edit_catalog":role!="web","manage_access":role=="owner","file_operations":role!="web","acquire_on_core":role!="web","remote_acquisition":false,"job_lifetime":"core_process","source_availability":"per_source","provider_status":"per_request"}}))
}
pub async fn get(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{let who=crate::access::authenticate(&a,&h)?;Ok(Json(snapshot(&a,&who.role)?))}
#[derive(Deserialize)]pub struct Rename{revision:i64,name:String}
pub async fn update(State(a):State<App>,h:HeaderMap,Json(v):Json<Rename>)->Result<Json<Library>,ApiError>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Local owner access required".into()));}Ok(Json(rename(&a.db,v.revision,&v.name)?))}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn library_identity_survives_reopen_rename_and_backup_but_device_is_not_copied(){
  let t=tempfile::tempdir().unwrap();let state=t.path().join("state");let app=crate::initialize(state.clone()).unwrap();let original=library(&app.db.lock().unwrap()).unwrap();let d=app.device.clone();
  let renamed=rename(&app.db,1,"Fixture collection").unwrap();assert_eq!(renamed.id,original.id);assert_eq!(renamed.revision,2);assert!(rename(&app.db,1,"Stale").is_err());assert!(rename(&app.db,2,"\n").is_err());assert_eq!(rename(&app.db,2,"Fixture collection").unwrap().revision,2);
  let backup=crate::backup::export(&app.db,t.path()).unwrap();assert!(!backup.join("device.json").exists());let raw=fs::read(backup.join("collection.sqlite")).unwrap();assert!(!raw.windows(d.id.len()).any(|w|w==d.id.as_bytes()));
  let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::initialize(restored).unwrap();assert_eq!(library(&restored.db.lock().unwrap()).unwrap(),renamed);assert_ne!(restored.device.id,d.id);
  drop(app);let reopened=crate::initialize(state).unwrap();assert_eq!(reopened.device,d);assert_eq!(library(&reopened.db.lock().unwrap()).unwrap(),renamed);
 }
 #[test]fn same_names_have_distinct_ids_and_device_is_stable_across_catalogs(){
  let t=tempfile::tempdir().unwrap();let machine=t.path().join("machine");let a=crate::initialize_with_device(t.path().join("a"),&machine).unwrap();let b=crate::initialize_with_device(t.path().join("b"),&machine).unwrap();
  let aa=library(&a.db.lock().unwrap()).unwrap();let bb=library(&b.db.lock().unwrap()).unwrap();assert_eq!(aa.name,bb.name);assert_ne!(aa.id,bb.id);assert_eq!(a.device.id,b.device.id);assert_ne!(a.instance_id,b.instance_id);
  assert!(snapshot(&a,"web").unwrap()["capabilities"]["collection_paging"]==true);assert!(snapshot(&a,"web").unwrap()["capabilities"]["work_scoped_reads"]==true);assert!(snapshot(&a,"web").unwrap()["capabilities"]["file_operations"]==false);assert!(snapshot(&a,"owner").unwrap()["capabilities"]["manage_access"]==true);
  fs::write(machine.join("device.json"),b"broken").unwrap();assert!(device(&machine).is_err());assert_eq!(fs::read(machine.join("device.json")).unwrap(),b"broken");
 }
 #[test]fn schema_nine_migrates_without_changing_catalog_content(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=db::open(&path).unwrap();{let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Existing','Existing'); DROP TABLE library_identity; PRAGMA user_version=9;").unwrap();}drop(db);
  let db=db::open(&path).unwrap();let c=db.lock().unwrap();assert!(uuid::Uuid::parse_str(&library(&c).unwrap().id).is_ok());assert_eq!(c.query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Existing");
 }
 #[tokio::test]async fn context_is_private_readable_and_cross_library_writes_are_rejected(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let lib=library(&app.db.lock().unwrap()).unwrap();let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let a=app.clone();let server=tokio::spawn(async move{crate::serve_listener(a,listener).await.unwrap()});let client=reqwest::Client::new();
  assert_eq!(client.get(format!("{base}/api/context")).send().await.unwrap().status(),401);
  let ctx=client.get(format!("{base}/api/context")).bearer_auth(&app.token).send().await.unwrap();assert_eq!(ctx.headers()["x-galroon-library"],lib.id);assert_eq!(ctx.headers()["x-galroon-device"],app.device.id);assert_eq!(ctx.json::<Value>().await.unwrap()["library"]["id"],lib.id);
  for (key,bad) in [("x-galroon-library",db::id()),("x-galroon-device",db::id())]{let response=client.post(format!("{base}/api/works")).bearer_auth(&app.token).header(key,bad).json(&json!({"title":"Wrong collection"})).send().await.unwrap();assert_eq!(response.status(),409);}
  assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);server.abort();
 }
}

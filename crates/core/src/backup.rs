//! Consistent catalog and offline artwork snapshots. Game files are never copied.
use crate::{db::{Db,id,now},plans};
use rusqlite::{Connection,MAIN_DB,OpenFlags};
use serde::{Serialize,Deserialize};
use std::{path::{Path,PathBuf},fs,io::Write};
type R<T>=Result<T,String>;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="lowercase")]pub enum Mode{Portable,Full}
impl Default for Mode{fn default()->Self{Self::Portable}}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="lowercase")]pub enum Origin{Manual,Scheduled}
impl Default for Origin{fn default()->Self{Self::Manual}}
pub struct Options{pub mode:Mode,pub origin:Origin,pub artwork:bool}
impl Default for Options{fn default()->Self{Self{mode:Mode::Portable,origin:Origin::Manual,artwork:true}}}
/// Reserved export directory names are excluded even while a backup is being written.
pub fn protected_path(path:&Path)->bool{path.components().any(|part|{let name=part.as_os_str().to_string_lossy().to_ascii_lowercase();name.starts_with("galroon-backup-")||name.starts_with(".galroon-backup-")})}
#[derive(Serialize,Deserialize,Debug)]pub struct Manifest{pub format:String,pub version:u32,pub created:i64,pub database:String,pub bytes:u64,pub sha256:String,pub includes_game_files:bool,pub includes_credentials:bool,
 #[serde(default)]pub library:Option<crate::context::Library>,#[serde(default)]pub schema:Option<i64>,#[serde(default)]pub tables:Vec<String>,#[serde(default)]pub verification:String,
 #[serde(default)]pub mode:Mode,#[serde(default)]pub origin:Origin,#[serde(default)]pub identity:Option<Identity>,#[serde(default)]pub core_version:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")]pub artwork:Option<Artwork>}
#[derive(Serialize,Deserialize,Debug)]pub struct Identity{pub files:Vec<IdentityFile>,pub device_id:String,pub spki_fingerprint:Option<String>}
#[derive(Serialize,Deserialize,Debug)]pub struct IdentityFile{pub name:String,pub bytes:u64,pub sha256:String}
#[derive(Serialize,Deserialize,Debug)]pub struct Artwork{pub file:String,pub bytes:u64,pub sha256:String,pub images:i64,pub profiles:i64}
fn combined(catalog:&str,artwork:&str)->String{use sha2::{Digest,Sha256};hex::encode(Sha256::digest(format!("{catalog}:{artwork}").as_bytes()))}
fn verify_content(folder:&Path,file:&Path,manifest:&Manifest)->R<()>{
 plans::validate_chain(file)?;let(size,mut hash)=plans::hash(file)?;
 if let Some(art)=&manifest.artwork{
  if !(3..=4).contains(&manifest.version)||art.file!=crate::artwork_library::FILE{return Err("Unsupported artwork backup".into());}
  let path=folder.join(&art.file);plans::validate_chain(&path)?;let(bytes,sha256)=plans::hash(&path)?;
  if bytes!=art.bytes||sha256!=art.sha256{return Err("Backup artwork changed or is incomplete".into());}
  let c=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;c.execute_batch("PRAGMA trusted_schema=OFF;").map_err(|e|e.to_string())?;
  let integrity:String=c.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  let counts:(i64,i64)=c.query_row("SELECT (SELECT count(*) FROM images),(SELECT count(*) FROM profiles)",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
  if integrity!="ok"||counts!=(art.images,art.profiles){return Err("Invalid offline artwork backup".into());}hash=combined(&hash,&sha256);
 }else if manifest.version==3{return Err("Offline artwork is missing from backup".into());}
 if size!=manifest.bytes||hash!=manifest.sha256{return Err("Backup content changed or is incomplete".into());}Ok(())
}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS backup_history(id TEXT PRIMARY KEY,path TEXT NOT NULL,created INTEGER NOT NULL,manifest TEXT NOT NULL);")?;
 let columns=c.prepare("PRAGMA table_info(backup_history)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for (column,value) in [("origin","manual"),("mode","portable")]{if !columns.iter().any(|n|n==column){c.execute(&format!("ALTER TABLE backup_history ADD COLUMN {column} TEXT NOT NULL DEFAULT '{value}'"),[])?;}}Ok(())
}
fn tables(c:&Connection)->R<Vec<String>>{let mut q=c.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<String>>>().map_err(|e|e.to_string())?;Ok(rows)}
pub fn history(db:&Db)->R<serde_json::Value>{let c=db.lock().map_err(|e|e.to_string())?;let mut q=c.prepare("SELECT id,path,created,manifest,origin,mode FROM backup_history ORDER BY created DESC,rowid DESC LIMIT 50").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;Ok(serde_json::json!(rows.into_iter().map(|(id,path,created,manifest,origin,mode)|serde_json::json!({"id":id,"path":path,"created":created,"origin":origin,"mode":mode,"manifest":serde_json::from_str::<serde_json::Value>(&manifest).unwrap_or_default()})).collect::<Vec<_>>()))}
pub(crate) fn clean_credentials(c:&Connection)->R<()>{
 c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA secure_delete=ON; DELETE FROM sessions; DELETE FROM settings WHERE key NOT IN ('schema_version','organization.managed_root');").map_err(|e|e.to_string())?;
 clean_transient(c)
}
fn clean_transient(c:&Connection)->R<()>{
 c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA secure_delete=ON;").map_err(|e|e.to_string())?;
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
fn private_permissions(path:&Path,directory:bool)->R<()>{
 #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(path,fs::Permissions::from_mode(if directory{0o700}else{0o600})).map_err(|e|e.to_string())?;}
 #[cfg(not(unix))]let _=(path,directory);
 Ok(())
}
fn new_file(path:&Path,private:bool)->R<fs::File>{
 let mut options=fs::OpenOptions::new();options.write(true).create_new(true);
 #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;if private{options.mode(0o600);}}
 let file=options.open(path).map_err(|e|e.to_string())?;if private{private_permissions(path,false)?;}Ok(file)
}
fn write_new(path:&Path,bytes:&[u8],private:bool)->R<()>{let mut file=new_file(path,private)?;file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())}
const IDENTITY_FILES:[&str;4]=["device.json","network.json","lan-key.pem","lan-cert.pem"];
fn read_identity(path:&Path)->R<Option<Vec<u8>>>{
 plans::validate_chain(path)?;
 match fs::symlink_metadata(path){
  Ok(m)=>{if !m.is_file()||m.file_type().is_symlink(){return Err("Identity must be a regular file, not a symbolic link".into());}if path.file_name().is_some_and(|n|n=="device.json")&&m.len()>4096{return Err("Invalid device identity size".into());}},
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(e)=>return Err(e.to_string())
 }
 fs::read(path).map(Some).map_err(|e|e.to_string())
}
fn identity_details(folder:&Path,files:&[IdentityFile])->R<(String,Option<String>)>{
 let mut contents=std::collections::BTreeMap::new();
 for file in files{
  if !IDENTITY_FILES.contains(&file.name.as_str())||contents.contains_key(&file.name){return Err("Invalid backup identity filename".into());}
  let path=folder.join(&file.name);let bytes=read_identity(&path)?.ok_or("Backup identity file is missing")?;
  use sha2::{Digest,Sha256};if bytes.len() as u64!=file.bytes||hex::encode(Sha256::digest(&bytes))!=file.sha256{return Err("Backup identity changed or is incomplete".into());}contents.insert(file.name.clone(),bytes);
 }
 let device:crate::context::Device=serde_json::from_slice(contents.get("device.json").ok_or("Backup device identity is missing")?).map_err(|e|e.to_string())?;
 uuid::Uuid::parse_str(&device.id).map_err(|e|e.to_string())?;
 let spki=match(contents.get("lan-key.pem"),contents.get("lan-cert.pem")){
  (Some(key),Some(cert))=>Some(crate::lan_tls::format_spki_fingerprint(&crate::lan_tls::validate_identity(key,cert)?)),
  (None,None)=>None,_=>return Err("Incomplete LAN TLS identity".into())
 };Ok((device.id,spki))
}
fn export_identity(device_dir:&Path,folder:&Path)->R<Identity>{
 fs::create_dir(folder).map_err(|e|e.to_string())?;private_permissions(folder,true)?;let mut files=Vec::new();
 for name in IDENTITY_FILES{if let Some(content)=read_identity(&device_dir.join(name))?{let path=folder.join(name);write_new(&path,&content,true)?;let(bytes,sha256)=plans::hash(&path)?;files.push(IdentityFile{name:name.into(),bytes,sha256});}}
 let(device_id,spki_fingerprint)=identity_details(folder,&files)?;Ok(Identity{files,device_id,spki_fingerprint})
}
pub fn export(db:&Db,destination:&Path)->R<PathBuf>{export_with(db,destination,None,Options::default())}
pub fn export_with(db:&Db,destination:&Path,device_dir:Option<&Path>,options:Options)->R<PathBuf>{
 if options.mode==Mode::Full&&device_dir.is_none(){return Err("Full backups require a device directory".into());}
 let parent=destination.canonicalize().map_err(|e|match e.kind(){std::io::ErrorKind::NotFound=>"Choose an existing backup destination folder".to_owned(),std::io::ErrorKind::PermissionDenied=>"Cannot access the backup destination folder. Check its permissions or choose another folder.".to_owned(),_=>format!("Could not access the backup destination folder: {e}")})?;plans::validate_chain(&parent)?;if !parent.is_dir(){return Err("Choose an existing backup destination folder".into());}
 let folder=parent.join(format!("Galroon-backup-{}-{}",now(),id()));fs::create_dir(&folder).map_err(|e|e.to_string())?;
 let result=(||{
 let private=options.mode==Mode::Full;if private{private_permissions(&folder,true)?;}let file=folder.join("collection.sqlite");if private{new_file(&file,true)?;}
 let data_dir={let c=db.lock().map_err(|e|e.to_string())?;c.backup(MAIN_DB,&file,None).map_err(|e|e.to_string())?;c.path().and_then(|p|Path::new(p).parent()).map(Path::to_path_buf)};
 {let c=Connection::open(&file).map_err(|e|e.to_string())?;if private{clean_transient(&c)?;}else{clean_credentials(&c)?;}check(&c)?;}
 let(bytes,sha256)=plans::hash(&file)?;let c=Connection::open_with_flags(&file,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
 let artwork=if let Some(base)=data_dir.filter(|p|options.artwork&&p.join(crate::artwork_library::FILE).exists()){
  if private{new_file(&folder.join(crate::artwork_library::FILE),true)?;}
  let stats=crate::artwork_library::Library::open(&base)?.snapshot(&folder.join(crate::artwork_library::FILE))?;
  let(bytes,sha256)=plans::hash(&folder.join(crate::artwork_library::FILE))?;
  Some(Artwork{file:crate::artwork_library::FILE.into(),bytes,sha256,images:stats.images,profiles:stats.profiles})
 }else{None};
 let sha256=artwork.as_ref().map(|a|combined(&sha256,&a.sha256)).unwrap_or(sha256);
 let identity=if private{Some(export_identity(device_dir.unwrap(),&folder.join("identity"))?)}else{None};
 let manifest=Manifest{format:"galroon-catalog-backup".into(),version:4,created:now(),database:"collection.sqlite".into(),bytes,sha256,includes_game_files:false,includes_credentials:private,mode:options.mode,origin:options.origin,identity,core_version:Some(env!("CARGO_PKG_VERSION").into()),library:Some(crate::context::library(&c).map_err(|e|e.to_string())?),schema:Some(c.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?),tables:tables(&c)?,verification:"integrity-and-hash".into(),artwork};drop(c);
 write_new(&folder.join("manifest.json"),&serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?,private)?;
 let mut paths=vec!["collection.sqlite".to_owned(),"manifest.json".to_owned()];if let Some(art)=&manifest.artwork{paths.push(art.file.clone());}if let Some(identity)=&manifest.identity{paths.extend(identity.files.iter().map(|f|format!("identity/{}",f.name)));}paths.sort();
 let mut sums=String::new();for path in paths{let(_,hash)=plans::hash(&folder.join(&path))?;sums.push_str(&format!("{hash}  {path}\n"));}write_new(&folder.join("SHA256SUMS"),sums.as_bytes(),private)?;
 inspect(&folder)?;
 db.lock().map_err(|e|e.to_string())?.execute("INSERT INTO backup_history(id,path,created,manifest,origin,mode) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![id(),folder.to_string_lossy(),manifest.created,serde_json::to_string(&manifest).map_err(|e|e.to_string())?,match options.origin{Origin::Manual=>"manual",Origin::Scheduled=>"scheduled"},if private{"full"}else{"portable"}]).map_err(|e|e.to_string())?;Ok(())
 })();if let Err(error)=result{let _=fs::remove_dir_all(&folder);return Err(error);}Ok(folder)
}
pub fn inspect(folder:&Path)->R<Manifest>{
 let folder=folder.canonicalize().map_err(|e|e.to_string())?;plans::validate_chain(&folder)?;let path=folder.join("manifest.json");plans::validate_chain(&path)?;if fs::metadata(&path).map_err(|e|e.to_string())?.len()>16384{return Err("Invalid backup manifest size".into());}
 let manifest:Manifest=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if manifest.format!="galroon-catalog-backup"||!(1..=4).contains(&manifest.version)||manifest.database!="collection.sqlite"||manifest.includes_game_files||manifest.includes_credentials&&(manifest.version!=4||manifest.mode!=Mode::Full)||manifest.version<4&&manifest.mode!=Mode::Portable||(manifest.mode==Mode::Full)!=manifest.identity.is_some(){return Err("Unsupported backup format".into());}
 if let Some(identity)=&manifest.identity{let(device_id,spki)=identity_details(&folder.join("identity"),&identity.files)?;if device_id!=identity.device_id||spki!=identity.spki_fingerprint{return Err("Backup identity does not match its manifest".into());}}
 let file=folder.join(&manifest.database);verify_content(&folder,&file,&manifest)?;
 let c=Connection::open_with_flags(&file,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;check(&c)?;
 if manifest.version>=2 {
  let schema:i64=c.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  if manifest.schema!=Some(schema)||manifest.library.as_ref()!=Some(&crate::context::library(&c).map_err(|e|e.to_string())?)||manifest.tables!=tables(&c)?||manifest.verification!="integrity-and-hash"{return Err("Backup manifest does not match its collection".into());}
 }
 Ok(manifest)
}
/// Restore to a new data directory; switching the running Core is a separate,
/// reviewed lifecycle operation. Never overwrite an existing collection here.
pub fn restore_new(folder:&Path,destination:&Path,expected_hash:Option<&str>)->R<PathBuf>{restore(folder,destination,expected_hash,false)}
/// Preserve credentials and stage identity under the new collection, without installing it into device_dir.
pub fn restore_new_full(folder:&Path,destination:&Path,expected_hash:Option<&str>)->R<PathBuf>{restore(folder,destination,expected_hash,true)}
fn restore(folder:&Path,destination:&Path,expected_hash:Option<&str>,full:bool)->R<PathBuf>{
 let manifest=inspect(folder)?;if full&&manifest.mode!=Mode::Full{return Err("Full restore requires a full backup".into());}if expected_hash.is_some_and(|h|h!=manifest.sha256){return Err("Backup changed since preview. Inspect it again.".into());}plans::validate_chain(destination)?;if destination.exists(){return Err("Restore requires a new, unused data directory".into());}
 fs::create_dir(destination).map_err(|e|e.to_string())?;if full{private_permissions(destination,true)?;}let marker=destination.join("restore-incomplete");new_file(&marker,full)?.sync_all().map_err(|e|e.to_string())?;let file=destination.join("library.sqlite");let mut source=fs::File::open(folder.join("collection.sqlite")).map_err(|e|e.to_string())?;let mut dest=new_file(&file,full)?;std::io::copy(&mut source,&mut dest).map_err(|e|e.to_string())?;dest.sync_all().map_err(|e|e.to_string())?;drop(dest);
 if let Some(art)=&manifest.artwork{
  let mut source=fs::File::open(folder.join(&art.file)).map_err(|e|e.to_string())?;let mut dest=new_file(&destination.join(&art.file),full)?;
  std::io::copy(&mut source,&mut dest).map_err(|e|e.to_string())?;dest.sync_all().map_err(|e|e.to_string())?;
 }
 verify_content(destination,&file,&manifest)?;
 let c=Connection::open(&file).map_err(|e|e.to_string())?;check(&c)?;if full{clean_transient(&c)?;}else{clean_credentials(&c)?;}
 if manifest.artwork.is_some(){
  let tx=c.unchecked_transaction().map_err(|e|e.to_string())?;
  // Work pages back the local relationship index. Entity biographies stay in the
  // permanent library; importing legacy filmographies here just duplicates them.
  for(key,value)in crate::artwork_library::Library::open(destination)?.profiles()?.into_iter().filter(|(key,_)|key.starts_with("exploration.v4.work.")){
   tx.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![key,value]).map_err(|e|e.to_string())?;
  }tx.commit().map_err(|e|e.to_string())?;
 }
 c.execute_batch("UPDATE jobs SET state='interrupted',message='Restored from backup; verify source paths before resuming.' WHERE state IN ('queued','running','pausing','cancelling'); UPDATE frontier SET state='pending' WHERE state='running'; UPDATE plans SET state='ready',approved=NULL WHERE state IN ('approved','executing','partial');").map_err(|e|e.to_string())?;
 // Materialize disabled entries even for legacy backups without watcher rows;
 // startup's ensure() must not recreate enabled watchers for restored roots.
 crate::watching::migrate(&c)?;crate::watching::ensure(&c)?;
 c.execute_batch("UPDATE source_watch SET enabled=0,pending='[]',job_id=NULL,needs_scan=1,status='disabled',revision=revision+1,message='Restored collection. Check source paths before enabling monitoring.';").map_err(|e|e.to_string())?;
 drop(c);
 if full{let identity=manifest.identity.as_ref().unwrap();let target=destination.join("identity");fs::create_dir(&target).map_err(|e|e.to_string())?;private_permissions(&target,true)?;for file in &identity.files{let bytes=read_identity(&folder.join("identity").join(&file.name))?.ok_or("Backup identity file is missing")?;write_new(&target.join(&file.name),&bytes,true)?;}identity_details(&target,&identity.files)?;}
 fs::remove_file(marker).map_err(|e|e.to_string())?;destination.canonicalize().map_err(|e|e.to_string())
}
#[cfg(test)]mod tests{
 use super::*;
 use serde_json::{json,Value};
 const PASSWORD:&str="Generated backup owner password";
 fn full()->Options{Options{mode:Mode::Full,..Default::default()}}
 fn save_manifest(folder:&Path,manifest:&Manifest){fs::write(folder.join("manifest.json"),serde_json::to_vec(manifest).unwrap()).unwrap();}
 fn verify_sums(folder:&Path){
  let sums=fs::read_to_string(folder.join("SHA256SUMS")).unwrap();assert!(sums.ends_with('\n'));let mut paths=Vec::new();
  for line in sums.lines(){let(hash,path)=line.split_once("  ").unwrap();assert_eq!(hash,plans::hash(&folder.join(path)).unwrap().1);paths.push(path.to_owned());}
  let mut expected=vec!["collection.sqlite".to_owned(),"manifest.json".to_owned()];let manifest=inspect(folder).unwrap();if let Some(art)=manifest.artwork{expected.push(art.file);}if let Some(identity)=manifest.identity{expected.extend(identity.files.iter().map(|f|format!("identity/{}",f.name)));}expected.sort();assert_eq!(paths,expected);
 }
 fn verify_permissions(folder:&Path){
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(fs::metadata(folder).unwrap().permissions().mode()&0o777,0o700);for entry in fs::read_dir(folder).unwrap(){let path=entry.unwrap().path();if path.is_dir(){verify_permissions(&path);}else{assert_eq!(fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600,"{}",path.display());}}}
  #[cfg(not(unix))]let _=folder;
 }
 async fn post(app:&crate::App,path:&str,token:&str,value:Value)->(axum::http::StatusCode,Value){
  use tower::ServiceExt;let request=axum::http::Request::builder().method("POST").uri(path).header("Host","127.0.0.1").header("Origin","http://localhost:1420").header("Authorization",format!("Bearer {token}")).header("Content-Type","application/json").body(axum::body::Body::from(value.to_string())).unwrap();
  let response=crate::router(app.clone()).oneshot(request).await.unwrap();let status=response.status();let bytes=axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap();(status,serde_json::from_slice(&bytes).unwrap())
 }
 async fn desktop_token(app:&crate::App)->String{
  let(status,pair)=post(app,"/api/access/pairing",&app.token,json!({"name":"Backup fixture","role":"desktop"})).await;assert_eq!(status,200,"{pair}");
  let(status,session)=post(app,"/api/access/pair","",json!({"code":pair["code"]})).await;assert_eq!(status,200,"{session}");session["token"].as_str().unwrap().into()
 }
 #[test]fn portable_v4_artwork_is_optional_and_legacy_versions_remain_readable(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("active")).unwrap();crate::artwork_library::Library::open(&app.state_dir).unwrap();
  for artwork in [true,false]{let folder=export_with(&app.db,t.path(),None,Options{artwork,..Default::default()}).unwrap();let mut manifest=inspect(&folder).unwrap();assert_eq!(manifest.version,4);assert_eq!(manifest.mode,Mode::Portable);assert_eq!(manifest.origin,Origin::Manual);assert_eq!(manifest.core_version.as_deref(),Some(env!("CARGO_PKG_VERSION")));assert!(!manifest.includes_credentials);assert!(manifest.identity.is_none());assert_eq!(manifest.artwork.is_some(),artwork);assert_eq!(folder.join(crate::artwork_library::FILE).exists(),artwork);verify_sums(&folder);
   assert!(restore_new_full(&folder,&t.path().join("not-full"),None).is_err());assert!(!t.path().join("not-full").exists());
   manifest.includes_credentials=true;save_manifest(&folder,&manifest);assert!(inspect(&folder).is_err());manifest.includes_credentials=false;
   manifest.version=if artwork{3}else{2};save_manifest(&folder,&manifest);assert!(inspect(&folder).is_ok());fs::remove_file(folder.join("SHA256SUMS")).unwrap();assert!(inspect(&folder).is_ok());
   manifest.version=3;manifest.artwork=None;save_manifest(&folder,&manifest);assert!(inspect(&folder).is_err());
  }
  let h=history(&app.db).unwrap();assert_eq!(h[0]["mode"],"portable");assert_eq!(h[0]["origin"],"manual");
 }
 #[tokio::test]async fn full_backup_restores_credentials_identity_and_safe_runtime_state(){
  use argon2::{Argon2,PasswordHash,PasswordVerifier};
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("active")).unwrap();crate::access::setup(&app.db,PASSWORD,false).unwrap();let token=desktop_token(&app).await;
  let(status,_)=post(&app,"/api/access/pairing",&app.token,json!({"name":"Pending fixture"})).await;assert_eq!(status,200);
  let settings=crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830);let tls=crate::lan_tls::LanTls::load_or_generate(&app.device_dir,&settings).unwrap();fs::write(app.device_dir.join("network.json"),serde_json::to_vec(&settings).unwrap()).unwrap();
  let library=crate::artwork_library::Library::open(&app.state_dir).unwrap();library.save_profile("exploration.v4.work.v1.1",&json!({"fixture":true})).unwrap();drop(library);
  let password_hash:String;
  {let c=app.db.lock().unwrap();password_hash=c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)).unwrap();c.execute_batch("INSERT INTO settings(key,value) VALUES('fixture.setting','keep'); INSERT INTO roots(id,path,label) VALUES('r','/generated/source','Fixture'); INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('j','scan','running','{}',1,1); INSERT INTO frontier(job_id,path,state) VALUES('j','/generated/source','running'); INSERT INTO plans(id,kind,state,items,digest,created,approved) VALUES('p','move','approved','[]','fixture',1,1);").unwrap();crate::watching::ensure(&c).unwrap();c.execute_batch("UPDATE source_watch SET enabled=1,job_id='j',pending='[{\"scope\":\"\",\"recursive\":true}]';").unwrap();}
  let folder=export_with(&app.db,t.path(),Some(&app.device_dir),Options{origin:Origin::Scheduled,..full()}).unwrap();let manifest=inspect(&folder).unwrap();assert_eq!(manifest.mode,Mode::Full);assert_eq!(manifest.origin,Origin::Scheduled);assert!(manifest.includes_credentials);let identity=manifest.identity.as_ref().unwrap();assert_eq!(identity.device_id,app.device.id);assert_eq!(identity.files.len(),4);assert_eq!(identity.spki_fingerprint,Some(crate::lan_tls::format_spki_fingerprint(&crate::lan_tls::spki_fingerprint(&tls.cert_der))));assert!(!folder.join("identity/device.lock").exists());assert!(!folder.join("identity/lan-tls.lock").exists());verify_sums(&folder);verify_permissions(&folder);
  {let c=Connection::open(folder.join("collection.sqlite")).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get::<_,String>(0)).unwrap(),password_hash);for table in ["pairing_codes","access_attempts"]{assert_eq!(c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get::<_,i64>(0)).unwrap(),0);}}
  assert_eq!(history(&app.db).unwrap()[0]["origin"],"scheduled");assert_eq!(history(&app.db).unwrap()[0]["mode"],"full");
  let wrong=t.path().join("wrong-hash");assert!(restore_new_full(&folder,&wrong,Some("changed")).is_err());assert!(!wrong.exists());
  let restored=restore_new_full(&folder,&t.path().join("full"),Some(&manifest.sha256)).unwrap();verify_permissions(&restored);assert!(!restored.join("device.json").exists());assert!(!restored.join("restore-incomplete").exists());
  for file in &identity.files{assert_eq!(fs::read(restored.join("identity").join(&file.name)).unwrap(),fs::read(app.device_dir.join(&file.name)).unwrap());}
  let restored=crate::initialize(restored).unwrap();let mut headers=axum::http::HeaderMap::new();headers.insert("Authorization",format!("Bearer {token}").parse().unwrap());assert_eq!(crate::access::authenticate(&restored,&headers).unwrap().role,"desktop");
  {let c=restored.db.lock().unwrap();let hash:String=c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)).unwrap();assert_eq!(hash,password_hash);assert!(Argon2::default().verify_password(PASSWORD.as_bytes(),&PasswordHash::new(&hash).unwrap()).is_ok());assert_eq!(c.query_row("SELECT value FROM settings WHERE key='fixture.setting'",[],|r|r.get::<_,String>(0)).unwrap(),"keep");assert_eq!(c.query_row("SELECT state FROM jobs WHERE id='j'",[],|r|r.get::<_,String>(0)).unwrap(),"interrupted");assert_eq!(c.query_row("SELECT state FROM frontier WHERE job_id='j'",[],|r|r.get::<_,String>(0)).unwrap(),"pending");assert_eq!(c.query_row("SELECT state FROM plans WHERE id='p'",[],|r|r.get::<_,String>(0)).unwrap(),"ready");let watch=crate::watching::status(&c,"r").unwrap();assert_eq!(watch["enabled"],false);assert_eq!(watch["pending"],0);assert_eq!(watch["job_id"],Value::Null);assert!(c.query_row("SELECT value FROM settings WHERE key='exploration.v4.work.v1.1'",[],|r|r.get::<_,String>(0)).is_ok());}
  let portable=restore_new(&folder,&t.path().join("portable"),Some(&manifest.sha256)).unwrap();assert!(!portable.join("identity").exists());let portable=crate::initialize(portable).unwrap();assert!(crate::access::authenticate(&portable,&headers).is_err());let c=portable.db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key='owner_password'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn full_backup_without_pems_and_failed_exports_leave_no_partial_folder(){
  let t=tempfile::tempdir().unwrap();let device=t.path().join("device");let identity=crate::context::device(&device).unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();let target=t.path().join("backups");fs::create_dir(&target).unwrap();
  let folder=export_with(&db,&target,Some(&device),full()).unwrap();let manifest=inspect(&folder).unwrap();let saved=manifest.identity.unwrap();assert_eq!(saved.device_id,identity.id);assert_eq!(saved.files.len(),1);assert!(saved.spki_fingerprint.is_none());fs::remove_dir_all(&folder).unwrap();
  assert!(export_with(&db,&target,None,full()).is_err());fs::write(device.join("lan-key.pem"),b"unpaired").unwrap();assert!(export_with(&db,&target,Some(&device),full()).is_err());assert_eq!(fs::read_dir(&target).unwrap().count(),0);
  fs::remove_file(device.join("lan-key.pem")).unwrap();fs::remove_file(device.join("device.json")).unwrap();assert!(export_with(&db,&target,Some(&device),full()).is_err());assert_eq!(fs::read_dir(&target).unwrap().count(),0);
  db.lock().unwrap().execute_batch("DROP TABLE backup_history;").unwrap();assert!(export(&db,&target).is_err());assert_eq!(fs::read_dir(&target).unwrap().count(),0);
 }
 #[test]fn full_backup_rejects_tampered_identity_and_inconsistent_manifests(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("active")).unwrap();let settings=crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830);crate::lan_tls::LanTls::load_or_generate(&app.device_dir,&settings).unwrap();
  let folder=export_with(&app.db,t.path(),Some(&app.device_dir),full()).unwrap();let original=fs::read(folder.join("manifest.json")).unwrap();let device=folder.join("identity/device.json");let bytes=fs::read(&device).unwrap();fs::write(&device,b"tampered").unwrap();assert!(inspect(&folder).is_err());fs::write(&device,&bytes).unwrap();
  let other=t.path().join("other");fs::create_dir(&other).unwrap();crate::lan_tls::LanTls::load_or_generate(&other,&settings).unwrap();let cert=folder.join("identity/lan-cert.pem");let cert_bytes=fs::read(&cert).unwrap();fs::copy(other.join("lan-cert.pem"),&cert).unwrap();assert!(inspect(&folder).is_err());
  let mut manifest:Manifest=serde_json::from_slice(&original).unwrap();let entry=manifest.identity.as_mut().unwrap().files.iter_mut().find(|f|f.name=="lan-cert.pem").unwrap();(entry.bytes,entry.sha256)=plans::hash(&cert).unwrap();save_manifest(&folder,&manifest);assert!(inspect(&folder).unwrap_err().contains("Invalid LAN TLS identity"));fs::write(&cert,cert_bytes).unwrap();
  for change in 0..8{let mut m:Manifest=serde_json::from_slice(&original).unwrap();match change{0=>{m.mode=Mode::Portable;m.identity=None;},1=>m.version=3,2=>m.identity=None,3=>m.identity.as_mut().unwrap().spki_fingerprint=Some("wrong".into()),4=>m.identity.as_mut().unwrap().device_id=id(),5=>m.identity.as_mut().unwrap().files[0].name="../device.json".into(),6=>m.identity.as_mut().unwrap().files.retain(|f|f.name!="lan-cert.pem"),_=>{m.mode=Mode::Portable;m.includes_credentials=false;}}save_manifest(&folder,&m);assert!(inspect(&folder).is_err(),"case {change}");}
  let mut m:Manifest=serde_json::from_slice(&original).unwrap();let mut value:Value=serde_json::from_slice(&bytes).unwrap();value["platform"]=json!("different-os");fs::write(&device,serde_json::to_vec(&value).unwrap()).unwrap();let entry=m.identity.as_mut().unwrap().files.iter_mut().find(|f|f.name=="device.json").unwrap();(entry.bytes,entry.sha256)=plans::hash(&device).unwrap();save_manifest(&folder,&m);assert!(inspect(&folder).is_ok());
  value["id"]=json!("not-a-uuid");fs::write(&device,serde_json::to_vec(&value).unwrap()).unwrap();let entry=m.identity.as_mut().unwrap().files.iter_mut().find(|f|f.name=="device.json").unwrap();(entry.bytes,entry.sha256)=plans::hash(&device).unwrap();save_manifest(&folder,&m);assert!(inspect(&folder).is_err());
  fs::write(&device,vec![b' ';4097]).unwrap();assert!(inspect(&folder).is_err());
 }
 #[cfg(unix)]#[test]fn full_backup_rejects_identity_symlinks(){
  use std::os::unix::fs::symlink;
  let t=tempfile::tempdir().unwrap();let device=t.path().join("device");crate::context::device(&device).unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();let target=t.path().join("backups");fs::create_dir(&target).unwrap();
  let folder=export_with(&db,&target,Some(&device),full()).unwrap();fs::remove_file(folder.join("identity/device.json")).unwrap();symlink(device.join("device.json"),folder.join("identity/device.json")).unwrap();assert!(inspect(&folder).is_err());fs::remove_dir_all(&folder).unwrap();
  symlink(t.path().join("missing"),device.join("network.json")).unwrap();assert!(export_with(&db,&target,Some(&device),full()).is_err());assert_eq!(fs::read_dir(&target).unwrap().count(),0);fs::remove_file(device.join("network.json")).unwrap();
  let link=t.path().join("linked-device");symlink(&device,&link).unwrap();assert!(export_with(&db,&target,Some(&link),full()).is_err());assert_eq!(fs::read_dir(&target).unwrap().count(),0);
 }
 #[test]fn schema_fifty_backup_history_migrates_existing_rows_once(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("active/library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute_batch("DROP TABLE backup_history; CREATE TABLE backup_history(id TEXT PRIMARY KEY,path TEXT NOT NULL,created INTEGER NOT NULL,manifest TEXT NOT NULL); INSERT INTO backup_history VALUES('old','/old/backup',1,'{}'); PRAGMA user_version=50; UPDATE settings SET value='50' WHERE key='schema_version';").unwrap();drop(db);
  for _ in 0..2{let db=crate::db::open(&path).unwrap();assert_eq!(history(&db).unwrap(),json!([{"id":"old","path":"/old/backup","created":1,"manifest":{},"origin":"manual","mode":"portable"}]));assert_eq!(db.lock().unwrap().query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),51);}
  assert_eq!(fs::read_dir(path.parent().unwrap().join("migration-backups")).unwrap().count(),1);
 }
 #[tokio::test]async fn backup_api_full_requires_owner_and_rejects_invalid_modes(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("active")).unwrap();crate::access::setup(&app.db,PASSWORD,false).unwrap();let desktop=desktop_token(&app).await;
  let(status,error)=post(&app,"/api/backups",&desktop,json!({"destination":t.path(),"mode":"full"})).await;assert_eq!(status,403);assert_eq!(error["error"],"Owner access required for full backups");assert_eq!(history(&app.db).unwrap(),json!([]));
  for mode in [json!("invalid"),json!("Full"),json!(null),json!(1)]{let(status,_)=post(&app,"/api/backups",&app.token,json!({"destination":t.path(),"mode":mode})).await;assert_eq!(status,400);}
  for (token,mode) in [(&app.token,Some("full")),(&desktop,Some("portable")),(&desktop,None)]{let mut body=json!({"destination":t.path()});if let Some(mode)=mode{body["mode"]=json!(mode);}let(status,result)=post(&app,"/api/backups",token,body).await;assert_eq!(status,200,"{result}");let manifest=inspect(Path::new(result["path"].as_str().unwrap())).unwrap();assert_eq!(manifest.mode,if mode==Some("full"){Mode::Full}else{Mode::Portable});assert_eq!(manifest.origin,Origin::Manual);}
 }
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
  let folder=export(&db,t.path()).unwrap();let manifest=inspect(&folder).unwrap();assert_eq!(manifest.version,4);assert_eq!(manifest.schema,Some(crate::db::SCHEMA_VERSION));assert!(manifest.tables.contains(&"custom_tags".into()));assert_eq!(manifest.library.as_ref().unwrap(),&crate::context::library(&db.lock().unwrap()).unwrap());
  let h=history(&db).unwrap();assert_eq!(h.as_array().unwrap().len(),1);assert_eq!(h[0]["manifest"]["verification"],"integrity-and-hash");
  let path=folder.join("manifest.json");let original=fs::read(&path).unwrap();let mut forged:serde_json::Value=serde_json::from_slice(&original).unwrap();forged["library"]["id"]=serde_json::json!("different-library");fs::write(&path,serde_json::to_vec(&forged).unwrap()).unwrap();assert!(inspect(&folder).is_err());
  let mut legacy:serde_json::Value=serde_json::from_slice(&original).unwrap();legacy["version"]=serde_json::json!(1);for key in ["library","schema","tables","verification","mode","origin","identity","core_version"]{legacy.as_object_mut().unwrap().remove(key);}fs::write(&path,serde_json::to_vec(&legacy).unwrap()).unwrap();assert_eq!(inspect(&folder).unwrap().version,1);
  assert!(export(&db,&t.path().join("unavailable")).is_err());assert_eq!(history(&db).unwrap().as_array().unwrap().len(),1);
 }
 #[test]fn backup_is_consistent_private_and_restore_never_overwrites(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("active/library.sqlite")).unwrap();{let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,overrides) VALUES('w','Manual title','Original','{\"title\":\"Manual title\"}'); INSERT INTO sessions(token_hash,role,created) VALUES('test-session-secret','owner',1); INSERT INTO settings(key,value) VALUES('owner_password','test-password-secret');").unwrap();}
 let folder=export(&db,t.path()).unwrap();let m=inspect(&folder).unwrap();assert!(!m.includes_game_files);assert!(!m.includes_credentials);let raw=fs::read(folder.join("collection.sqlite")).unwrap();for secret in [b"test-session-secret".as_slice(),b"test-password-secret".as_slice()]{assert!(!raw.windows(secret.len()).any(|w|w==secret));}
 let restored=restore_new(&folder,&t.path().join("restored"),None).unwrap();let c=Connection::open(restored.join("library.sqlite")).unwrap();assert_eq!(c.query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Manual title");assert!(restore_new(&folder,&restored,None).is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 fs::OpenOptions::new().append(true).open(folder.join("collection.sqlite")).unwrap().write_all(b"corruption").unwrap();assert!(inspect(&folder).is_err());
 }
}



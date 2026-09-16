use std::{path::PathBuf,fs,io::Write,time::Duration};
use galroon_core::{local_core,backup,plans};
pub struct Controller{pub base:PathBuf,pub exe:PathBuf,pub gate:tokio::sync::Mutex<()>}
impl Controller{
 fn launch_lock(&self)->Result<fs::File,String>{fs::create_dir_all(&self.base).map_err(|e|e.to_string())?;plans::validate_chain(&self.base)?;let file=fs::OpenOptions::new().create(true).read(true).write(true).open(self.base.join("launch.lock")).map_err(|e|e.to_string())?;fs2::FileExt::try_lock_exclusive(&file).map_err(|_|"Another desktop is connecting or restoring this Core; try again shortly".to_string())?;Ok(file)}
 fn active(&self)->Result<PathBuf,String>{let pointer=self.base.join("active-library.json");if !pointer.exists(){return Ok(self.base.clone());}plans::validate_chain(&pointer)?;let v:serde_json::Value=serde_json::from_slice(&fs::read(&pointer).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let dir=PathBuf::from(v["current"].as_str().ok_or("Invalid collection pointer")?).canonicalize().map_err(|e|e.to_string())?;if !dir.starts_with(self.base.canonicalize().map_err(|e|e.to_string())?){return Err("Collection pointer is outside its local data directory".into());}plans::validate_chain(&dir)?;Ok(dir)}
 pub async fn session(&self)->Result<local_core::Session,String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;let saved=crate::paired::read(&self.base)?;if let Some(id)=saved.selected {return crate::paired::connect(saved.entries.iter().find(|e|e.id==id).ok_or("Selected connection is missing")?).await;}local_core::connect_or_start_with_device(self.active()?,self.exe.clone(),self.base.clone()).await}
 pub async fn connections(&self)->Result<crate::paired::Connections,String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;crate::paired::read(&self.base)}
 pub async fn restore_history(&self)->Result<serde_json::Value,String>{
  let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;
  if crate::paired::read(&self.base)?.selected.is_some(){return Err("Recovery history belongs to the local owner desktop".into());}
  let pointer=self.base.join("active-library.json");plans::validate_chain(&pointer)?;if !pointer.exists(){return Ok(serde_json::json!([]));}
  if fs::metadata(&pointer).map_err(|e|e.to_string())?.len()>1024*1024{return Err("Collection history is too large".into());}
  let data:serde_json::Value=serde_json::from_slice(&fs::read(pointer).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  let entries=data["previous"].as_array().ok_or("Invalid collection history")?;
  let base=self.base.canonicalize().map_err(|e|e.to_string())?;
  let mut result=Vec::new();for entry in entries.iter().rev().take(50){
   let Some(snapshot)=entry["snapshot"].as_str()else{continue};let path=PathBuf::from(snapshot);
   if !path.starts_with(base.join(".galroon-restore-snapshots"))||path.components().any(|p|matches!(p,std::path::Component::ParentDir)){return Err("Recovery snapshot path is outside the protected directory".into());}
   result.push(serde_json::json!({"path":snapshot,"created":entry["replaced_at"],"library":entry["library"]}));
  }Ok(serde_json::json!(result))
 }
 pub async fn pair(&self,url:String,code:String)->Result<crate::paired::Connections,String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;crate::paired::pair(&self.base,&url,&code).await}
 pub async fn update_endpoint(&self,id:String,url:String)->Result<crate::paired::Connections,String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;crate::paired::update_endpoint(&self.base,&id,&url).await}
 pub async fn select(&self,id:Option<String>)->Result<(),String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;let mut saved=crate::paired::read(&self.base)?;if let Some(id)=&id{crate::paired::connect(saved.entries.iter().find(|e|&e.id==id).ok_or("Connection not found")?).await?;}saved.selected=id;crate::paired::save(&self.base,&saved)}
 pub async fn remove_connection(&self,id:String,forget:bool)->Result<crate::paired::Connections,String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;if forget{return crate::paired::forget(&self.base,&id);}crate::paired::remove(&self.base,&id).await}
 pub async fn stop(&self)->Result<(),String>{let _gate=self.gate.lock().await;let _lock=self.launch_lock()?;if crate::paired::read(&self.base)?.selected.is_some(){return Err("Stop the Core from its local owner desktop".into());}local_core::request(&self.active()?,&self.exe,"stop").await?;Ok(())}
 pub async fn restore(&self,source:PathBuf,expected_hash:String)->Result<PathBuf,String>{
  let _gate=self.gate.lock().await;let _launch=self.launch_lock()?;if crate::paired::read(&self.base)?.selected.is_some(){return Err("Restore from the Core's local owner desktop".into());}let previous=self.active()?;
  let collections=self.base.join("collections");plans::validate_chain(&collections)?;fs::create_dir_all(&collections).map_err(|e|e.to_string())?;let destination=collections.join(galroon_core::db::id());
  let restored=tokio::task::spawn_blocking(move||backup::restore_new(&source,&destination,Some(&expected_hash))).await.map_err(|e|e.to_string())??;
  local_core::request(&previous,&self.exe,"stop").await?;
  let old_lock=fs::OpenOptions::new().read(true).write(true).open(previous.join("core.lock")).map_err(|e|e.to_string())?;
  let deadline=tokio::time::Instant::now()+Duration::from_secs(10);loop{if fs2::FileExt::try_lock_exclusive(&old_lock).is_ok(){break;}if tokio::time::Instant::now()>=deadline{return Err("Core is still stopping. Original collection remains selected.".into());}tokio::time::sleep(Duration::from_millis(100)).await;}
  // Keep the old writer lock until the pointer commit. If protection fails, the
  // old pointer remains authoritative and a normal reconnect reopens it.
  let protected=self.base.join(".galroon-restore-snapshots");plans::validate_chain(&protected)?;fs::create_dir_all(&protected).map_err(|e|e.to_string())?;
  let snapshot_db=galroon_core::db::open(&previous.join("library.sqlite")).map_err(|e|e.to_string())?;
  let snapshot=backup::export(&snapshot_db,&protected)?;drop(snapshot_db);
  let snapshot_manifest=backup::inspect(&snapshot)?;
  let pointer=self.base.join("active-library.json");let pending=self.base.join(format!("active-library-{}.json",galroon_core::db::id()));plans::validate_chain(&pointer)?;
  let mut config=if pointer.exists(){serde_json::from_slice::<serde_json::Value>(&fs::read(&pointer).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?}else{serde_json::json!({"previous":[]})};
  config["previous"].as_array_mut().ok_or("Invalid collection history")?.push(serde_json::json!({"path":previous,"replaced_at":galroon_core::db::now(),"snapshot":snapshot,"sha256":snapshot_manifest.sha256,"library":snapshot_manifest.library}));config["current"]=serde_json::json!(restored);
  let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&pending).map_err(|e|e.to_string())?;file.write_all(&serde_json::to_vec_pretty(&config).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);
  fs::rename(&pending,&pointer).map_err(|e|e.to_string())?;Ok(restored)
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[tokio::test]async fn restored_sources_stay_disabled_in_external_core(){
  let t=tempfile::tempdir().unwrap();let source=t.path().join("generated-source");fs::create_dir(&source).unwrap();
  let db=galroon_core::db::open(&t.path().join("snapshot/library.sqlite")).unwrap();
  {let c=db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated source')",[source.to_string_lossy().to_string()]).unwrap();c.execute("INSERT INTO source_watch(root_id,path,enabled) VALUES('r',?1,1)",[source.to_string_lossy().to_string()]).unwrap();}
  let folder=backup::export(&db,t.path()).unwrap();drop(db);let manifest=backup::inspect(&folder).unwrap();
  let controller=Controller{base:t.path().join("desktop"),exe:test_core(),gate:tokio::sync::Mutex::new(())};controller.session().await.unwrap();
  let restored=controller.restore(folder,manifest.sha256).await.unwrap();controller.session().await.unwrap();
  fs::write(source.join("new.zip"),b"generated restore event").unwrap();
  // Exceed the watcher quiet period, so an accidentally enabled watcher has time to enqueue.
  tokio::time::sleep(Duration::from_secs(5)).await;
  let db=galroon_core::db::open(&restored.join("library.sqlite")).unwrap();let c=db.lock().unwrap();
  assert_eq!(galroon_core::watching::status(&c,"r").unwrap()["enabled"],false);
  assert_eq!(c.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  drop(c);drop(db);controller.stop().await.unwrap();
 }
 fn test_core()->PathBuf{std::env::var_os("GALROON_TEST_CORE").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/galroon-core.exe")).canonicalize().expect("Acceptance Core executable must exist")}
 #[tokio::test]async fn paired_endpoint_recovery_preserves_identity_and_credentials(){
  let t=tempfile::tempdir().unwrap();let exe=test_core();
  let owner=Controller{base:t.path().join("owner"),exe:exe.clone(),gate:tokio::sync::Mutex::new(())};
  let desktop=Controller{base:t.path().join("desktop"),exe:exe.clone(),gate:tokio::sync::Mutex::new(())};
  let s=owner.session().await.unwrap();let client=reqwest::Client::new();
  client.post(format!("{}/api/access/setup",s.url)).bearer_auth(&s.token).json(&serde_json::json!({"password":"Generated endpoint test password"})).send().await.unwrap().error_for_status().unwrap();
  let code=client.post(format!("{}/api/access/pairing",s.url)).bearer_auth(&s.token).json(&serde_json::json!({"name":"Endpoint fixture"})).send().await.unwrap().error_for_status().unwrap().json::<serde_json::Value>().await.unwrap();
  let saved=desktop.pair(s.url.clone(),code["code"].as_str().unwrap().into()).await.unwrap();let id=saved.entries[0].id.clone();
  desktop.select(Some(id.clone())).await.unwrap();let before=desktop.session().await.unwrap();
  owner.stop().await.unwrap();
  let lock=fs::OpenOptions::new().read(true).write(true).open(owner.base.join("core.lock")).unwrap();let until=tokio::time::Instant::now()+Duration::from_secs(10);
  loop{if fs2::FileExt::try_lock_exclusive(&lock).is_ok(){break;}assert!(tokio::time::Instant::now()<until);tokio::time::sleep(Duration::from_millis(100)).await;}drop(lock);
  let restarted=owner.session().await.unwrap();assert_ne!(restarted.instance_id,before.instance_id);
  let original=fs::read(desktop.base.join("connections.json")).unwrap();
  assert!(desktop.update_endpoint(id.clone(),"http://192.168.1.1:14800".into()).await.is_err());
  assert_eq!(fs::read(desktop.base.join("connections.json")).unwrap(),original);
  // Even a valid credential must not permit replacing a pinned identity.
  let mut invalid=crate::paired::read(&desktop.base).unwrap();invalid.entries[0].device_id="wrong-device".into();crate::paired::save(&desktop.base,&invalid).unwrap();
  let invalid_bytes=fs::read(desktop.base.join("connections.json")).unwrap();
  assert!(desktop.update_endpoint(id.clone(),restarted.url.clone()).await.is_err());
  assert_eq!(fs::read(desktop.base.join("connections.json")).unwrap(),invalid_bytes);
  invalid.entries[0].device_id=before.device_id.clone();crate::paired::save(&desktop.base,&invalid).unwrap();
  let recovered=desktop.update_endpoint(id.clone(),restarted.url.clone()).await.unwrap();assert_eq!(recovered.selected,Some(id.clone()));
  let after=desktop.session().await.unwrap();assert_eq!(after.library_id,before.library_id);assert_eq!(after.device_id,before.device_id);assert!(after.token==before.token);assert_eq!(after.url,restarted.url);
  assert!(!desktop.base.join("library.sqlite").exists());
  desktop.select(None).await.unwrap();desktop.remove_connection(id,false).await.unwrap();owner.stop().await.unwrap();
 }
 #[tokio::test]async fn paired_connection_persists_revokes_and_never_falls_back(){
  let t=tempfile::tempdir().unwrap();let exe=test_core();
  let owner=Controller{base:t.path().join("owner"),exe:exe.clone(),gate:tokio::sync::Mutex::new(())};let s=owner.session().await.unwrap();
  let client=reqwest::Client::new();
  let setup=client.post(format!("{}/api/access/setup",s.url)).bearer_auth(&s.token).json(&serde_json::json!({"password":"Generated pairing test password"})).send().await.unwrap();assert!(setup.status().is_success());
  let paircode=client.post(format!("{}/api/access/pairing",s.url)).bearer_auth(&s.token).json(&serde_json::json!({"name":"Generated paired desktop"})).send().await.unwrap().json::<serde_json::Value>().await.unwrap();
  let desktop=Controller{base:t.path().join("desktop"),exe,gate:tokio::sync::Mutex::new(())};
  let saved=desktop.pair(s.url.clone(),paircode["code"].as_str().unwrap().into()).await.unwrap();assert!(saved.selected.is_none());let id=saved.entries[0].id.clone();
  assert!(desktop.pair(s.url.clone(),paircode["code"].as_str().unwrap().into()).await.is_err());
  desktop.select(Some(id.clone())).await.unwrap();let connected=desktop.session().await.unwrap();assert_eq!(connected.library_id,s.library_id);
  let raw=fs::read_to_string(desktop.base.join("connections.json")).unwrap();assert!(!raw.contains(&connected.token));
  let who=client.get(format!("{}/api/access/me",s.url)).bearer_auth(&connected.token).send().await.unwrap().json::<serde_json::Value>().await.unwrap();assert_eq!(who["role"],"desktop");assert_eq!(who["can_manage_access"],false);
  assert!(desktop.stop().await.is_err());assert!(desktop.restore(t.path().join("none"),"unused".into()).await.is_err());assert!(desktop.remove_connection(id.clone(),false).await.is_err());
  let mut changed=crate::paired::read(&desktop.base).unwrap();changed.entries[0].device_id="different-device".into();crate::paired::save(&desktop.base,&changed).unwrap();assert!(desktop.session().await.is_err());
  changed.entries[0].device_id=s.device_id.clone();crate::paired::save(&desktop.base,&changed).unwrap();assert!(desktop.session().await.is_ok());
  let revoke=client.post(format!("{}/api/access/devices/{}/revoke",s.url,who["id"].as_str().unwrap())).bearer_auth(&s.token).json(&serde_json::json!({})).send().await.unwrap();assert!(revoke.status().is_success());
  assert!(desktop.session().await.is_err());assert!(!desktop.base.join("library.sqlite").exists());assert_eq!(desktop.connections().await.unwrap().selected,Some(id.clone()));
  desktop.select(None).await.unwrap();desktop.remove_connection(id,true).await.unwrap();assert!(desktop.connections().await.unwrap().entries.is_empty());
  // A fresh credential can be revoked by the paired desktop itself while unselected.
  let code=client.post(format!("{}/api/access/pairing",s.url)).bearer_auth(&s.token).json(&serde_json::json!({"name":"Generated revoke test"})).send().await.unwrap().json::<serde_json::Value>().await.unwrap();
  let saved=desktop.pair(s.url.clone(),code["code"].as_str().unwrap().into()).await.unwrap();let revoked=crate::paired::connect(&saved.entries[0]).await.unwrap();desktop.remove_connection(saved.entries[0].id.clone(),false).await.unwrap();
  assert_eq!(client.get(format!("{}/api/access/me",s.url)).bearer_auth(&revoked.token).send().await.unwrap().status(),reqwest::StatusCode::UNAUTHORIZED);
  owner.stop().await.unwrap();
 }
 async fn titles(s:&local_core::Session)->Vec<String>{let v:serde_json::Value=reqwest::Client::new().get(format!("{}/api/works",s.url)).bearer_auth(&s.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();v.as_array().unwrap().iter().map(|w|w["title"].as_str().unwrap().into()).collect()}
 #[tokio::test]async fn restore_switches_core_and_preserves_previous_database(){
  let t=tempfile::tempdir().unwrap();let base=t.path().join("controller");let initial=galroon_core::db::open(&base.join("library.sqlite")).unwrap();initial.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('old','Original catalog','Original catalog')",[]).unwrap();drop(initial);
  let snapshot=galroon_core::db::open(&t.path().join("snapshot/library.sqlite")).unwrap();snapshot.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('saved','Saved catalog','Saved catalog')",[]).unwrap();let backup=backup::export(&snapshot,t.path()).unwrap();drop(snapshot);let manifest=backup::inspect(&backup).unwrap();
  let exe=test_core();let controller=Controller{base:base.clone(),exe:exe.clone(),gate:tokio::sync::Mutex::new(())};
  let original=controller.session().await.unwrap();assert_eq!(titles(&original).await,vec!["Original catalog"]);
  assert!(controller.restore(backup.clone(),"wrong preview digest".into()).await.is_err());assert_eq!(controller.session().await.unwrap().instance_id,original.instance_id);
  fs::write(base.join(".galroon-restore-snapshots"),b"generated obstruction").unwrap();
  assert!(controller.restore(backup.clone(),manifest.sha256.clone()).await.is_err());assert!(!base.join("active-library.json").exists());
  assert_eq!(titles(&controller.session().await.unwrap()).await,vec!["Original catalog"]);
  fs::remove_file(base.join(".galroon-restore-snapshots")).unwrap();
  let restored=controller.restore(backup,manifest.sha256).await.unwrap();assert_ne!(restored,base);let current=controller.session().await.unwrap();assert_ne!(current.instance_id,original.instance_id);assert_eq!(current.device_id,original.device_id);assert_ne!(current.library_id,original.library_id);assert_eq!(titles(&current).await,vec!["Saved catalog"]);
  let old=galroon_core::db::open(&base.join("library.sqlite")).unwrap();let old_title:String=old.lock().unwrap().query_row("SELECT title FROM works WHERE id='old'",[],|r|r.get(0)).unwrap();assert_eq!(old_title,"Original catalog");drop(old);
  let history=controller.restore_history().await.unwrap();assert_eq!(history.as_array().unwrap().len(),1);assert_eq!(history[0]["library"]["id"],original.library_id);
  let protection=PathBuf::from(history[0]["path"].as_str().unwrap());let checked=backup::inspect(&protection).unwrap();assert_eq!(checked.library.unwrap().id,original.library_id);
  controller.restore(protection,checked.sha256).await.unwrap();let returned=controller.session().await.unwrap();assert_eq!(returned.library_id,original.library_id);assert_eq!(titles(&returned).await,vec!["Original catalog"]);
  let history=controller.restore_history().await.unwrap();assert_eq!(history.as_array().unwrap().len(),2);assert_eq!(history[0]["library"]["id"],current.library_id);
  let reopened=Controller{base,exe,gate:tokio::sync::Mutex::new(())};assert_eq!(reopened.session().await.unwrap().instance_id,returned.instance_id);reopened.stop().await.unwrap();tokio::time::sleep(Duration::from_millis(300)).await;
 }
 #[tokio::test]async fn reconnect_follows_an_externally_restarted_core(){
  let t=tempfile::tempdir().unwrap();let base=t.path().join("controller");let db=galroon_core::db::open(&base.join("library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('w','Persistent restart fixture','Persistent restart fixture')",[]).unwrap();drop(db);
  let exe=test_core();let controller=Controller{base:base.clone(),exe:exe.clone(),gate:tokio::sync::Mutex::new(())};let original=controller.session().await.unwrap();
  // Another trusted local client requests shutdown and starts the replacement.
  local_core::request(&base,&exe,"stop").await.unwrap();let lock=fs::OpenOptions::new().read(true).write(true).open(base.join("core.lock")).unwrap();let until=tokio::time::Instant::now()+Duration::from_secs(10);
  loop{if fs2::FileExt::try_lock_exclusive(&lock).is_ok(){break;}assert!(tokio::time::Instant::now()<until,"Old Core did not stop");tokio::time::sleep(Duration::from_millis(100)).await;}drop(lock);
  let replacement=local_core::connect_or_start(base.clone(),exe).await.unwrap();assert_ne!(original.instance_id,replacement.instance_id);assert_ne!(original.token,replacement.token);
  let reconnect=controller.session().await.unwrap();assert_eq!(reconnect.instance_id,replacement.instance_id);assert_eq!(reconnect.library_id,original.library_id);assert_eq!(reconnect.device_id,original.device_id);assert_eq!(reconnect.token,replacement.token);assert_eq!(reconnect.url,replacement.url);assert_eq!(titles(&reconnect).await,vec!["Persistent restart fixture"]);controller.stop().await.unwrap();tokio::time::sleep(Duration::from_millis(300)).await;
 }
}


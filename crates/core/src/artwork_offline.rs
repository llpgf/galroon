//! Resumable collection-wide download. Only Core-owned artwork data is written.
use crate::{App,ApiError,db::{id,now},artwork_library::Library};
use axum::{extract::State,http::HeaderMap,Json};
use rusqlite::params;
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::collections::BTreeSet;
type R<T>=Result<T,String>;
#[derive(Serialize,Deserialize)]struct Work{id:String,title:String,cover:String,vndb_id:Option<String>,#[serde(default)]generation:i64}
fn writer(a:&App,h:&HeaderMap)->Result<(),ApiError>{if crate::access::authenticate(a,h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(())}
pub async fn status(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
 writer(&a,&h)?;
 let stats=if a.state_dir.join(crate::artwork_library::FILE).exists(){Library::open(&a.state_dir)?.stats()?}else{crate::artwork_library::Stats{images:0,bytes:0,profiles:0}};
 let job={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let mut s=c.prepare("SELECT id,state,processed,discovered,errors,message FROM jobs WHERE kind='artwork' ORDER BY rowid DESC LIMIT 1")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"processed":r.get::<_,i64>(2)?,"discovered":r.get::<_,i64>(3)?,"errors":r.get::<_,i64>(4)?,"message":r.get::<_,String>(5)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;rows.into_iter().next()};
 let automatic=crate::collection_download::status(&*a.db.lock().map_err(|e|ApiError(e.to_string()))?)?;
 Ok(Json(json!({"images":stats.images,"bytes":stats.bytes,"profiles":stats.profiles,"job":job,"automatic":automatic})))
}
pub async fn start(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
 writer(&a,&h)?;let job=enqueue(&a)?;let run=job.clone();tokio::spawn(async move{run_job(a,run).await;});Ok(Json(json!({"id":job})))
}
fn enqueue(a:&App)->R<String>{
 enqueue_works(a,false)?.ok_or_else(||"No works to download".into())
}
pub(crate) fn enqueue_pending(a:&App)->R<Option<String>>{enqueue_works(a,true)}
fn enqueue_works(a:&App,automatic:bool)->R<Option<String>>{
 let _gate=a.mutation_lock.lock().map_err(|e|e.to_string())?;
 let mut c=a.db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 let busy:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE state IN ('queued','running','pausing','cancelling') OR (kind='artwork' AND state='paused')) OR EXISTS(SELECT 1 FROM plans WHERE state='executing')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if busy{if automatic{return Ok(None);}return Err("Pause running tasks and wait for file operations to finish first".into());}
 let query=if automatic{"SELECT w.id,w.title,w.cover,w.vndb_id,d.generation FROM works w JOIN collection_downloads d ON d.work_id=w.id WHERE w.merged_into IS NULL AND d.retry_at<=?1 ORDER BY w.id LIMIT 20"}else{"SELECT w.id,w.title,w.cover,w.vndb_id,coalesce(d.generation,0) FROM works w LEFT JOIN collection_downloads d ON d.work_id=w.id WHERE w.merged_into IS NULL AND ?1>=0 ORDER BY w.id"};
 let works={let mut s=tx.prepare(query).map_err(|e|e.to_string())?;let rows=s.query_map([now()],|r|Ok(Work{id:r.get(0)?,title:r.get(1)?,cover:r.get(2)?,vndb_id:r.get(3)?,generation:r.get(4)?})).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;rows};
 if automatic&&works.is_empty(){return Ok(None);}
 let job=id();tx.execute("INSERT INTO jobs(id,kind,state,spec,discovered,created,updated) VALUES(?1,'artwork','queued',?2,?3,?4,?4)",params![job,json!({"works":works,"automatic":automatic}).to_string(),works.len() as i64,now()]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(Some(job))
}
fn running(a:&App,job:&str)->R<bool>{
 let c=a.db.lock().map_err(|e|e.to_string())?;let state:String=c.query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get(0)).map_err(|e|e.to_string())?;
 if state=="running"{return Ok(true);}
 let target=match state.as_str(){"pausing"=>"paused","cancelling"=>"cancelled",other=>other};c.execute("UPDATE jobs SET state=?2,updated=?3 WHERE id=?1 AND state=?4",params![job,target,now(),state]).map_err(|e|e.to_string())?;Ok(false)
}
pub async fn run_job(a:App,job:String){
 if let Err(error)=run(&a,&job).await{crate::diagnostics::record("error","artwork.offline.failed",&error,Some(&job));if let Ok(c)=a.db.lock(){let _=c.execute("UPDATE jobs SET state='failed',message=?2,updated=?3 WHERE id=?1",params![job,error,now()]);}}
}
async fn run(a:&App,job:&str)->R<()>{
 let (works,offset)={let c=a.db.lock().map_err(|e|e.to_string())?;if c.execute("UPDATE jobs SET state='running',processed=CASE WHEN errors>0 THEN 0 ELSE processed END,errors=0,message='',updated=?2 WHERE id=?1 AND state='queued'",params![job,now()]).map_err(|e|e.to_string())?!=1{return Ok(());}
  let (raw,offset):(String,i64)=c.query_row("SELECT spec,processed FROM jobs WHERE id=?1",[job],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;let value:Value=serde_json::from_str(&raw).map_err(|e|e.to_string())?;(serde_json::from_value::<Vec<Work>>(value["works"].clone()).map_err(|e|e.to_string())?,offset)};
 for work in works.iter().skip(offset as usize){
  if !running(a,job)?{return Ok(());}
  a.db.lock().map_err(|e|e.to_string())?.execute("UPDATE jobs SET current_path=?2,updated=?3 WHERE id=?1",params![job,work.title,now()]).map_err(|e|e.to_string())?;
  let result=save_work(a,job,work).await;
  if !running(a,job)?{return Ok(());}
  let (bytes,errors,message)=match result{Ok(bytes)=>(bytes,0,String::new()),Err(error)=>(0,1,error)};
  let mut c=a.db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
  crate::collection_download::finish(&tx,&work.id,work.generation,if errors>0{Some(&message)}else{None}).map_err(|e|e.to_string())?;
  tx.execute("UPDATE jobs SET processed=processed+1,bytes=bytes+?2,errors=errors+?3,message=CASE WHEN ?3=1 THEN ?4 ELSE message END,updated=?5 WHERE id=?1",params![job,bytes,errors,message,now()]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;
 }
 a.db.lock().map_err(|e|e.to_string())?.execute("UPDATE jobs SET state=CASE WHEN state='pausing' THEN 'paused' WHEN state='cancelling' THEN 'cancelled' WHEN errors=0 THEN 'completed' ELSE 'failed' END,current_path='',updated=?2 WHERE id=?1 AND state IN ('running','pausing','cancelling')",params![job,now()]).map_err(|e|e.to_string())?;
 Ok(())
}
fn image_url(entity:&Value,out:&mut BTreeSet<String>){if let Some(url)=entity["image"]["url"].as_str(){out.insert(url.into());}}
async fn save_image(a:&App,url:&str)->R<i64>{
 if crate::artwork_library::image(&a.state_dir,url)?.is_some(){return Ok(0);}
 let image=crate::artwork::fetch(a,url).await.map_err(|e|e.0)?;let bytes=image.bytes.len() as i64;let base=a.state_dir.clone();let url=url.to_owned();
 tokio::task::spawn_blocking(move||Library::open(&base)?.put(&url,&image)).await.map_err(|e|e.to_string())??;Ok(bytes)
}
async fn save_work(a:&App,job:&str,work:&Work)->R<i64>{
 let mut bytes=0;let mut images=BTreeSet::new();let mut failure=None;
 if !work.cover.is_empty(){images.insert(work.cover.clone());}
 if let Some(id)=&work.vndb_id{
  let profile=crate::exploration::load(a,"work",id,1,false).await?;
  match complete(&profile){Ok(())=>{Library::open(&a.state_dir)?.save_profile(&format!("exploration.v4.work.{id}.1"),&profile)?;update_catalog(a,work,&profile)?;},Err(error)=>failure=Some(error)}
  image_url(&profile["vn"],&mut images);
  if let Some(characters)=profile["characters"].as_array(){for character in characters{image_url(character,&mut images);}}
  let mut related=entities(&profile);
  {use crate::relation_corrections::Key;let c=a.db.lock().map_err(|e|e.to_string())?;for correction in crate::relation_correction_store::read(&c,id).map_err(|e|e.0)?.1{if let Some(link)=correction.link{match link.key{
   Key::Staff{id,..}=>{related.insert(("person",id));},Key::Voice{id,character,..}=>{related.insert(("person",id));related.insert(("character",character));},Key::Character{id,..}=>{related.insert(("character",id));},Key::Company{id}=>{related.insert(("company",id));},Key::Work{..}=>{}
  }}}}
  let owned=crate::collection_profiles::owned(&*a.db.lock().map_err(|e|e.to_string())?)?;
  for kind in ["character","company","person"]{
   let ids:Vec<_>=related.iter().filter(|(k,_)|*k==kind).map(|(_,id)|id.clone()).collect();
   for chunk in ids.chunks(crate::exploration::ENTITY_BATCH){
    if !running(a,job)?{return Ok(bytes);}
    let (values,error)=crate::exploration::load_entities(a,kind,chunk).await?;
    if let Some(error)=error{failure=Some(format!("{kind}: {error}"));}
    for value in values{
     let id=value[kind]["id"].as_str().ok_or("VNDB biography has no ID")?;
     let value=crate::collection_profiles::basic(&value,kind,&owned);
     Library::open(&a.state_dir)?.save_profile(&crate::collection_profiles::key(kind,id),&value)?;
     image_url(&value[kind],&mut images);
    }
   }
  }
 }
 let images:Vec<_>=images.into_iter().collect();
 for chunk in images.chunks(4){
  if !running(a,job)?{return Ok(bytes);}
  let mut pending=tokio::task::JoinSet::new();for url in chunk{let a=a.clone();let url=url.clone();pending.spawn(async move{save_image(&a,&url).await});}
  while let Some(result)=pending.join_next().await{match result.map_err(|e|e.to_string())?{Ok(size)=>bytes+=size,Err(error)=>failure=Some(error)}}
 }
 match failure{Some(error)=>Err(error),None=>Ok(bytes)}
}
fn complete(value:&Value)->R<()>{if value["partial"]==true||value["stale"]==true||value["more"]==true{return Err("VNDB returned incomplete information; this download will be retried".into());}Ok(())}
fn update_catalog(a:&App,work:&Work,profile:&Value)->R<()>{
 let vn=&profile["vn"];let Some(title)=vn["title"].as_str()else{return Ok(());};
 let snapshot=crate::metadata::snapshot(&a.db,&work.id)?;
 let (remake,current):(bool,Option<String>)=a.db.lock().map_err(|e|e.to_string())?.query_row("SELECT vndb_release IS NOT NULL,vndb_id FROM works WHERE id=?1 AND merged_into IS NULL",[&work.id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
 if current!=work.vndb_id{return Err("Work identity changed during download; the new identity remains queued".into());}
 let names=|field:&str|vn[field].as_array().map(|values|values.iter().filter_map(|v|v["name"].as_str()).map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
 let mut source=json!({});
 if vn.get("description").is_some(){source["description"]=json!(vn["description"].as_str().unwrap_or(""));}if vn.get("tags").is_some(){source["tags"]=json!(names("tags"));}if let Some(aliases)=vn["aliases"].as_array(){source["aliases"]=json!(aliases);}
 if !remake{source["title"]=json!(title);source["original_title"]=json!(vn["alttitle"].as_str().unwrap_or(title));if vn.get("image").is_some(){source["cover"]=json!(vn["image"]["url"].as_str().unwrap_or(""));}if vn.get("released").is_some(){source["released"]=json!(vn["released"].as_str().unwrap_or(""));}if vn.get("developers").is_some(){source["developer"]=json!(names("developers").join(", "));}}
 // Fill newly imported records; a saved provider page must not undo newer catalog edits.
 source.as_object_mut().unwrap().retain(|field,_|snapshot["source"].get(field).is_none());
 if !source.as_object().unwrap().is_empty(){
  crate::metadata::apply(&a.db,&work.id,snapshot["revision"].as_i64().unwrap(),&json!({}),&[],Some(source))?;
 }
 Ok(())
}
pub(crate) fn entities(profile:&Value)->BTreeSet<(&'static str,String)>{
 let mut out=BTreeSet::new();
 for(field,kind,prefix)in [("characters","character",'c'),("staff","person",'s'),("developers","company",'p')]{
  let values=if field=="characters"{&profile[field]}else{&profile["vn"][field]};
  if let Some(values)=values.as_array(){for value in values{if let Some(id)=value["id"].as_str().filter(|id|crate::exploration::valid_id(id,prefix)){out.insert((kind,id.into()));}}}
 }
 if let Some(voices)=profile["vn"]["va"].as_array(){for voice in voices{for(field,kind,prefix)in [("staff","person",'s'),("character","character",'c')]{if let Some(id)=voice[field]["id"].as_str().filter(|id|crate::exploration::valid_id(id,prefix)){out.insert((kind,id.into()));}}}}
 out
}

#[cfg(test)]mod tests{
 use super::*;
 use crate::artwork_cache::Cache;
 use axum::{body::Body,http::Request};
 use tower::ServiceExt;
 #[tokio::test]async fn download_restore_and_expired_profiles_work_without_network(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("source")).unwrap();
  let cover="https://t.vndb.org/cv/01/1.jpg";let portrait="https://t.vndb.org/ch/02/2.png";let png=b"\x89PNG\r\n\x1a\nfixture";
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id,cover,description) VALUES('w','Saved work','Saved work','v1',?1,'Introduction')",[cover]).unwrap();
  let key="exploration.v4.work.v1.1";let profile=json!({"vn":{"id":"v1","title":"Saved work","image":{"url":cover}},"characters":[{"id":"c1","name":"Character","image":{"url":portrait}}],"fetched_at":now(),"more":false});
  crate::exploration::save(&a,key,&profile).unwrap();crate::exploration::save(&a,"exploration.v3.character.c1.1",&json!({"character":{"id":"c1","image":{"url":portrait}},"works":[],"more":false,"fetched_at":now()})).unwrap();let mut cache=Cache::open(&a.state_dir).unwrap();cache.put(cover,png).unwrap();cache.put(portrait,png).unwrap();
  let job=enqueue(&a).unwrap();run_job(a.clone(),job.clone()).await;
  assert_eq!(a.db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
  let library=Library::open(&a.state_dir).unwrap();assert_eq!(library.stats().unwrap().images,2);assert_eq!(library.stats().unwrap().profiles,2);
  let job2=enqueue(&a).unwrap();run_job(a.clone(),job2.clone()).await;assert_eq!(a.db.lock().unwrap().query_row("SELECT bytes FROM jobs WHERE id=?1",[&job2],|r|r.get::<_,i64>(0)).unwrap(),0);
  let mut expired=profile.clone();expired["fetched_at"]=json!(0);library.save_profile(key,&expired).unwrap();cache.clear().unwrap();drop(library);drop(cache);
  let backup=crate::backup::export(&a.db,t.path()).unwrap();let manifest=crate::backup::inspect(&backup).unwrap();assert_eq!(manifest.version,4);assert_eq!(manifest.artwork.as_ref().unwrap().images,2);
  let path=crate::backup::restore_new(&backup,&t.path().join("restored"),Some(&manifest.sha256)).unwrap();let restored=crate::initialize(path.clone()).unwrap();
  assert!(!path.join("artwork-cache.sqlite").exists());assert_eq!(crate::exploration::load(&restored,"work","v1",1,false).await.unwrap(),expired);
  assert_eq!(crate::exploration::cached(&restored,key).unwrap(),Some(expired));
  let request=Request::builder().uri(format!("/api/artwork?url={portrait}")).header("Host","127.0.0.1").header("Authorization",format!("Bearer {}",restored.token)).body(Body::empty()).unwrap();
  let response=crate::router(restored).oneshot(request).await.unwrap();assert_eq!(response.status(),200);assert_eq!(axum::body::to_bytes(response.into_body(),1000).await.unwrap().as_ref(),png);
  let exported=backup.join(crate::artwork_library::FILE);std::fs::OpenOptions::new().append(true).open(exported).unwrap().write_all(b"corrupt").unwrap();assert!(crate::backup::inspect(&backup).is_err());
 }
 #[tokio::test]async fn downloads_only_collection_images_and_direct_entity_basics(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();
  let cover="https://t.vndb.org/cv/01/1.jpg";let portrait="https://t.vndb.org/ch/02/2.jpg";let person_image="https://t.vndb.org/ch/03/3.jpg";
  let outside=json!({"id":"v999","title":"Outside the collection","image":{"url":"https://invalid.example/unowned-cover"},"va":[{"character":{"image":{"url":"https://invalid.example/unowned-character"}}}]});
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id,cover) VALUES('w','Owned','Owned','v1',?1)",[cover]).unwrap();
  let mut cache=Cache::open(&a.state_dir).unwrap();for url in [cover,portrait,person_image]{cache.put(url,b"\x89PNG\r\n\x1a\nfixture").unwrap();}
  let work=json!({"vn":{"id":"v1","image":{"url":cover},"relations":[outside.clone()],"staff":[{"id":"s1"}],"developers":[{"id":"p1"}]},"characters":[{"id":"c1","image":{"url":portrait}}],"more":false,"fetched_at":now()});
  crate::exploration::save(&a,"exploration.v4.work.v1.1",&work).unwrap();
  for(kind,id,entity,pages)in [("character","c1",json!({"id":"c1","image":{"url":portrait}}),1),("person","s1",json!({"id":"s1","image":{"url":person_image}}),2),("company","p1",json!({"id":"p1"}),1)]{
   for page in 1..=pages{let value=json!({(kind):entity,"works":vec![outside.clone();20],"page":page,"more":page<pages,"fetched_at":now()});crate::exploration::save(&a,&format!("exploration.v3.{kind}.{id}.{page}"),&value).unwrap();}
  }
  let job=enqueue(&a).unwrap();run_job(a.clone(),job.clone()).await;
  assert_eq!(a.db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get::<_,String>(0)).unwrap(),"completed");
  let library=Library::open(&a.state_dir).unwrap();assert_eq!(library.stats().unwrap().images,3);assert_eq!(library.stats().unwrap().profiles,4);
  assert!(library.profile("exploration.v3.person.s1.2").unwrap().is_none());
  let person=library.profile("collection.v1.person.s1").unwrap().unwrap();assert_eq!(person["person"]["id"],"s1");assert!(person.get("works").is_none());
  for url in [cover,portrait,person_image]{assert!(crate::artwork_library::contains(&a.state_dir,url).unwrap());}
 }
 #[tokio::test]async fn pause_and_restart_preserve_job_progress_and_owner_permissions(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('w','One','One'),('z','Two','Two')",[]).unwrap();
  let job=enqueue(&a).unwrap();a.db.lock().unwrap().execute("UPDATE jobs SET state='running',processed=1 WHERE id=?1",[&job]).unwrap();
  {let c=a.db.lock().unwrap();crate::timeline::control(&c,&job,"pause").unwrap();}assert!(!running(&a,&job).unwrap());
  {let c=a.db.lock().unwrap();crate::timeline::control(&c,&job,"resume").unwrap();}run_job(a.clone(),job.clone()).await;
  assert_eq!(a.db.lock().unwrap().query_row("SELECT processed FROM jobs WHERE id=?1",[job],|r|r.get::<_,i64>(0)).unwrap(),2);
  let request=Request::builder().method("POST").uri("/api/artwork/offline").header("Host","127.0.0.1").body(Body::empty()).unwrap();assert_eq!(crate::router(a).oneshot(request).await.unwrap().status(),401);
 }
 #[tokio::test]async fn paired_desktop_can_download_collection_artwork(){
  use sha2::{Digest,Sha256};
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();let token="d".repeat(72);
  a.db.lock().unwrap().execute("INSERT INTO sessions(token_hash,role,created,id,name,expires) VALUES(?1,'desktop',0,'device','Device',?2)",params![hex::encode(Sha256::digest(token.as_bytes())),now()+3600]).unwrap();
  for method in ["GET","POST"]{let request=Request::builder().method(method).uri("/api/artwork/offline").header("Host","127.0.0.1").header("Authorization",format!("Bearer {token}")).body(Body::empty()).unwrap();assert_eq!(crate::router(a.clone()).oneshot(request).await.unwrap().status(),200);}
 }
 #[tokio::test]async fn failed_download_resume_reuses_images_and_clears_error_count(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();let cover="https://t.vndb.org/cv/01/1.jpg";let portrait="https://t.vndb.org/ch/02/2.png";let png=b"\x89PNG\r\n\x1a\nfixture";
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id,cover) VALUES('w','One','One','v1',?1)",[cover]).unwrap();let mut cache=Cache::open(&a.state_dir).unwrap();cache.put(cover,png).unwrap();cache.put(portrait,png).unwrap();
  let mut profile=json!({"vn":{"id":"v1"},"characters":[{"image":{"url":"https://invalid.example/picture"}}],"more":false,"fetched_at":now()});let key="exploration.v4.work.v1.1";crate::exploration::save(&a,key,&profile).unwrap();
  let job=enqueue(&a).unwrap();run_job(a.clone(),job.clone()).await;
  assert_eq!(a.db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"failed");assert_eq!(Library::open(&a.state_dir).unwrap().stats().unwrap().images,1);
  profile["characters"][0]["image"]["url"]=json!(portrait);crate::exploration::save(&a,key,&profile).unwrap();{let c=a.db.lock().unwrap();crate::timeline::control(&c,&job,"resume").unwrap();}
  run_job(a.clone(),job.clone()).await;
  let result:(String,i64,i64)=a.db.lock().unwrap().query_row("SELECT state,processed,errors FROM jobs WHERE id=?1",[job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(result,("completed".into(),1,0));assert_eq!(Library::open(&a.state_dir).unwrap().stats().unwrap().images,2);
 }
 use std::io::Write;
}

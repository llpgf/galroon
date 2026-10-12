//! Device-local release notices. Clients only read Core's cached result.
use std::{fs,path::{Path,PathBuf},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
use axum::{extract::State,http::HeaderMap,Json};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{App,ApiError};
type R<T>=Result<T,String>;
struct Checking<'a>(&'a AtomicBool);
impl Drop for Checking<'_>{fn drop(&mut self){self.0.store(false,Ordering::Release);}}
const DAY:i64=24*60*60;
const RELEASES:&str="https://github.com/llpgf/galroon/releases";
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release{version:String,name:String,url:String,notes:String,published_at:Option<String>}
#[derive(Clone,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
struct Stored{version:u32,enabled:bool,last_checked:Option<i64>,last_error:Option<String>,etag:Option<String>,latest:Option<Release>}
impl Default for Stored{fn default()->Self{Self{version:1,enabled:true,last_checked:None,last_error:None,etag:None,latest:None}}}
pub struct Updates{path:PathBuf,stored:Mutex<Stored>,gate:Arc<tokio::sync::Mutex<()>>,wake:tokio::sync::Notify,checking:AtomicBool}
fn validate(path:&Path)->R<()>{crate::plans::validate_chain(path)?;if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){return Err("update-check.json must not be a symbolic link".into());}Ok(())}
fn load(path:&Path)->R<Stored>{
 validate(path)?;let bytes=match fs::read(path){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Stored::default()),Err(e)=>return Err(e.to_string())};
 let mut value:Stored=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;if value.version!=1{return Err("Unsupported update-check.json version".into());}
 if let Some(latest)=&mut value.latest{latest.url=trusted_url(&latest.url);latest.notes=latest.notes.chars().take(20_000).collect();}Ok(value)
}
fn save(path:&Path,value:&Stored)->R<()>{
 validate(path)?;let temp=path.with_file_name(format!(".update-check-{}.tmp",crate::db::id()));crate::plans::validate_chain(&temp)?;
 let result=(||{use std::io::Write;let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
  file.write_all(&serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);validate(path)?;fs::rename(&temp,path).map_err(|e|e.to_string())
 })();if result.is_err(){let _=fs::remove_file(temp);}result
}
fn trusted_url(url:&str)->String{if url.starts_with("https://github.com/llpgf/galroon/releases/"){url.into()}else{RELEASES.into()}}
fn full_version(tag:&str)->Option<semver::Version>{semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok().filter(|v|v.pre.is_empty())}
fn newer(release:&Release)->bool{full_version(&release.version).is_some_and(|v|v.cmp_precedence(&semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()).is_gt())}
fn api_base()->String{std::env::var("GALROON_UPDATE_API").unwrap_or_else(|_|"https://api.github.com".into())}
fn install_kind()->&'static str{if std::env::var("GALROON_INSTALL_KIND").is_ok_and(|v|v=="docker"){"docker"}else if cfg!(windows){"desktop"}else{"standalone"}}
impl Updates{
 pub fn load(device_dir:&Path)->Arc<Self>{let path=device_dir.join("update-check.json");let stored=load(&path).unwrap_or_else(|e|{crate::diagnostics::record("warn","updates.state",&format!("Ignoring damaged update-check.json: {e}"),None);Stored::default()});Arc::new(Self{path,stored:Mutex::new(stored),gate:Arc::new(tokio::sync::Mutex::new(())),wake:tokio::sync::Notify::new(),checking:AtomicBool::new(false)})}
 pub fn snapshot(&self)->Value{let state=self.stored.lock().unwrap();let latest=state.latest.as_ref().filter(|r|newer(r));json!({"current":env!("CARGO_PKG_VERSION"),"install_kind":install_kind(),"enabled":state.enabled,"checking":self.checking.load(Ordering::Acquire),"last_checked":state.last_checked,"last_error":state.last_error,"update_available":latest.is_some(),"latest":latest})}
 fn settings(&self,enabled:bool)->R<()>{let mut state=self.stored.lock().unwrap();let mut next=state.clone();next.enabled=enabled;save(&self.path,&next)?;*state=next;self.wake.notify_one();Ok(())}
 async fn check(self:&Arc<Self>,api:&str,automatic:bool,timeout:Duration)->Value{
  let gate=self.gate.clone().lock_owned().await;
  let previous={let state=self.stored.lock().unwrap();let interval=if !automatic{60}else if state.last_error.is_some(){3600}else{DAY};if automatic&&!state.enabled||state.last_checked.is_some_and(|t|crate::db::now().saturating_sub(t)<interval){drop(gate);drop(state);return self.snapshot();}state.clone()};
  // Once sent, a check finishes even if one HTTP caller disconnects. Other callers
  // wait on the same gate and reuse its result rather than issuing a second request.
  let updates=self.clone();let api=api.to_owned();let task=tokio::spawn(async move{
  updates.checking.store(true,Ordering::Release);let checking=Checking(&updates.checking);
  let result=fetch(&api,previous.etag.as_deref(),timeout).await;
  {let mut state=updates.stored.lock().unwrap();state.last_checked=Some(crate::db::now());match result{
   Ok(Some((latest,etag)))=>{state.latest=latest;state.etag=etag;state.last_error=None;},Ok(None)=>state.last_error=None,Err(e)=>state.last_error=Some(e)
  }if let Err(e)=save(&updates.path,&state){crate::diagnostics::record("warn","updates.save",&e,None);state.last_error=Some(format!("Cannot save update check: {e}"));}}
  drop(checking);drop(gate);updates.wake.notify_one();
  });if let Err(e)=task.await{crate::diagnostics::record("error","updates.check",&e.to_string(),None);}self.snapshot()
 }
 fn startup_delay(&self,startup:Duration)->Duration{let state=self.stored.lock().unwrap();startup.max(Duration::from_secs(state.last_checked.map(|t|t.saturating_add(DAY).saturating_sub(crate::db::now()).max(0) as u64).unwrap_or(0)))}
 fn delay(&self)->Option<Duration>{let state=self.stored.lock().unwrap();if !state.enabled{return None;}let interval=if state.last_error.is_some(){3600}else{DAY};Some(Duration::from_secs(state.last_checked.map(|t|(t.saturating_add(interval).saturating_sub(crate::db::now())).max(0) as u64).unwrap_or(0)))}
}
#[derive(Deserialize)]struct GithubRelease{tag_name:String,#[serde(default)]name:Option<String>,html_url:String,#[serde(default)]body:Option<String>,#[serde(default)]published_at:Option<String>,#[serde(default)]draft:bool,#[serde(default)]prerelease:bool}
async fn fetch(api:&str,etag:Option<&str>,timeout:Duration)->R<Option<(Option<Release>,Option<String>)>>{
 let client=reqwest::Client::builder().timeout(timeout).redirect(reqwest::redirect::Policy::none()).build().map_err(|e|e.to_string())?;
 let mut request=client.get(format!("{}/repos/llpgf/galroon/releases/latest",api.trim_end_matches('/'))).header("User-Agent",format!("Galroon/{}",env!("CARGO_PKG_VERSION"))).header("Accept","application/vnd.github+json");if let Some(etag)=etag{request=request.header("If-None-Match",etag);}
 let response=request.send().await.map_err(|e|e.to_string())?;if response.status()==reqwest::StatusCode::NOT_MODIFIED{return Ok(None);}
 if response.status()==reqwest::StatusCode::NOT_FOUND{return Ok(Some((None,None)));}
 if !response.status().is_success(){return Err(format!("Release check failed: HTTP {}",response.status()));}
 let etag=response.headers().get("etag").and_then(|v|v.to_str().ok()).map(str::to_owned);let release:GithubRelease=response.json().await.map_err(|e|e.to_string())?;
 let latest=full_version(&release.tag_name).filter(|_|!release.draft&&!release.prerelease).map(|version|Release{version:version.to_string(),name:release.name.unwrap_or(release.tag_name),url:trusted_url(&release.html_url),notes:release.body.unwrap_or_default().chars().take(20_000).collect(),published_at:release.published_at});Ok(Some((latest,etag)))
}
pub struct Worker{task:tokio::task::JoinHandle<()>}
impl Worker{pub fn request_stop(&self){self.task.abort();}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();}}
pub fn start(updates:Arc<Updates>)->Worker{
 // Tests exercise the real worker with an explicit fixture base. Other Core tests
 // must never gain an unsolicited public-network request by starting Services.
 #[cfg(test)]if std::env::var_os("GALROON_UPDATE_API").is_none(){return Worker{task:tokio::spawn(std::future::pending())};}
 start_with(updates,api_base(),Duration::from_secs(30))
}
fn start_with(updates:Arc<Updates>,api:String,startup:Duration)->Worker{
 Worker{task:tokio::spawn(async move{
  // Settings wake the worker immediately; an ordinary startup waits at least 30 seconds.
  tokio::select!{_=tokio::time::sleep(updates.startup_delay(startup))=>{},_=updates.wake.notified()=>{}}
  loop{match updates.delay(){
   Some(delay)=>{tokio::select!{_=tokio::time::sleep(delay)=>{updates.check(&api,true,Duration::from_secs(20)).await;},_=updates.wake.notified()=>{}}},
   None=>updates.wake.notified().await
  }}
 })}
}
pub async fn get(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{crate::auth(&a,&h)?;Ok(Json(a.updates.snapshot()))}
pub async fn check(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(Json(a.updates.check(&api_base(),false,Duration::from_secs(20)).await))}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Settings{enabled:bool}
pub async fn settings(State(a):State<App>,h:HeaderMap,Json(v):Json<Settings>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}a.updates.settings(v.enabled)?;Ok(Json(a.updates.snapshot()))}

#[cfg(test)]mod tests{
 use super::*;
 use axum::{http::{StatusCode,Request},response::IntoResponse,body::{Body,to_bytes},routing::get,Router};
 use tower::ServiceExt;
 #[derive(Clone)]struct Reply{status:StatusCode,body:Value,delay:Duration}
 struct Fixture{api:String,reply:Arc<Mutex<Reply>>,requests:Arc<Mutex<Vec<HeaderMap>>>,task:tokio::task::JoinHandle<()>}
 impl Drop for Fixture{fn drop(&mut self){self.task.abort();}}
 impl Fixture{
  async fn new()->Self{
   let reply=Arc::new(Mutex::new(Reply{status:StatusCode::OK,body:release("v9.0.0"),delay:Duration::ZERO}));let requests=Arc::new(Mutex::new(Vec::new()));let r=reply.clone();let seen=requests.clone();
   let router=Router::new().route("/repos/llpgf/galroon/releases/latest",get(move|headers:HeaderMap|{let r=r.lock().unwrap().clone();seen.lock().unwrap().push(headers);async move{tokio::time::sleep(r.delay).await;let mut response=(r.status,Json(r.body)).into_response();response.headers_mut().insert("etag","\"fixture\"".parse().unwrap());response}}));
   let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=format!("http://{}",listener.local_addr().unwrap());let task=tokio::spawn(async move{axum::serve(listener,router).await.unwrap()});Self{api,reply,requests,task}
  }
  fn count(&self)->usize{self.requests.lock().unwrap().len()}
 }
 fn release(tag:&str)->Value{json!({"tag_name":tag,"name":"Fixture release","html_url":"https://github.com/llpgf/galroon/releases/tag/fixture","body":"<b>Plain notes</b>","published_at":"2026-10-12T00:00:00Z","draft":false,"prerelease":false})}
 fn expire(updates:&Updates){updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-DAY-1);}
 async fn checked(updates:&Updates){tokio::time::timeout(Duration::from_secs(2),async{while updates.snapshot()["last_checked"].is_null()||updates.snapshot()["checking"]==true{tokio::time::sleep(Duration::from_millis(2)).await;}}).await.unwrap();}
 #[tokio::test]async fn updates_full_versions_only_and_trusted_plain_text(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  for (tag,available) in [("v9.0.0",true),(env!("CARGO_PKG_VERSION"),false),("0.0.1",false),("v0.6.0-beta.1",false),("garbage",false)]{
   fixture.reply.lock().unwrap().body=release(tag);expire(&updates);let state=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(state["update_available"],available,"{tag}");assert_eq!(state["latest"].is_null(),!available);assert!(state["last_error"].is_null());
  }
  for field in ["draft","prerelease"]{let mut body=release("v9.0.0");body[field]=json!(true);fixture.reply.lock().unwrap().body=body;expire(&updates);assert_eq!(updates.check(&fixture.api,false,Duration::from_secs(1)).await["update_available"],false);}
  let mut body=release("v9.0.0");body["html_url"]=json!("https://evil.example/releases/9");body["body"]=json!("界".repeat(20_001));fixture.reply.lock().unwrap().body=body;expire(&updates);
  let state=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(state["latest"]["url"],RELEASES);assert_eq!(state["latest"]["notes"].as_str().unwrap().chars().count(),20_000);
  assert_eq!(updates.snapshot()["current"],env!("CARGO_PKG_VERSION"));let requests=fixture.requests.lock().unwrap();assert_eq!(requests[0]["user-agent"],format!("Galroon/{}",env!("CARGO_PKG_VERSION")));assert_eq!(requests[0]["accept"],"application/vnd.github+json");
 }
 #[tokio::test]async fn updates_etag_304_404_and_failures_preserve_previous_release(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());let first=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(first["latest"]["notes"],"<b>Plain notes</b>");
  fixture.reply.lock().unwrap().status=StatusCode::NOT_MODIFIED;expire(&updates);let unchanged=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(unchanged["latest"],first["latest"]);assert!(unchanged["last_error"].is_null());assert_eq!(fixture.requests.lock().unwrap()[1]["if-none-match"],"\"fixture\"");
  fixture.reply.lock().unwrap().status=StatusCode::INTERNAL_SERVER_ERROR;expire(&updates);let failed=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(failed["latest"],first["latest"]);assert!(failed["last_error"].as_str().unwrap().contains("500"));
  let persisted=Updates::load(t.path()).snapshot();assert_eq!(persisted["last_error"],failed["last_error"]);assert_eq!(persisted["latest"],first["latest"]);
  {let mut reply=fixture.reply.lock().unwrap();reply.status=StatusCode::OK;reply.delay=Duration::from_millis(100);}expire(&updates);let timeout=updates.check(&fixture.api,false,Duration::from_millis(20)).await;assert!(!timeout["last_error"].is_null());assert_eq!(timeout["latest"],first["latest"]);
  {let mut reply=fixture.reply.lock().unwrap();reply.status=StatusCode::NOT_FOUND;reply.delay=Duration::ZERO;}expire(&updates);let missing=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert!(!missing["last_checked"].is_null());assert!(missing["last_error"].is_null());assert!(missing["latest"].is_null());assert_eq!(missing["update_available"],false);assert!(updates.stored.lock().unwrap().etag.is_none());
 }
 #[tokio::test]async fn updates_concurrent_checks_share_one_request_and_throttle(){
  let fixture=Fixture::new().await;fixture.reply.lock().unwrap().delay=Duration::from_millis(50);let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  let (a,b)=tokio::join!(updates.check(&fixture.api,false,Duration::from_secs(1)),updates.check(&fixture.api,false,Duration::from_secs(1)));assert_eq!(a,b);assert_eq!(fixture.count(),1);assert_eq!(a["checking"],false);
  updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),1);
  updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-61);updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),2);
  expire(&updates);fixture.reply.lock().unwrap().status=StatusCode::INTERNAL_SERVER_ERROR;updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);
  updates.check(&fixture.api,true,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-120);updates.check(&fixture.api,true,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);assert!(updates.delay().unwrap()>Duration::from_secs(3400));
  // A manual check after a failure waits only for the 60 s window, not the hourly retry.
  updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),4);fixture.reply.lock().unwrap().status=StatusCode::OK;
  expire(&updates);fixture.reply.lock().unwrap().status=StatusCode::OK;let shared=updates.clone();let api=fixture.api.clone();let caller=tokio::spawn(async move{shared.check(&api,false,Duration::from_secs(1)).await});
  tokio::time::timeout(Duration::from_secs(1),async{while fixture.count()<5{tokio::task::yield_now().await;}}).await.unwrap();caller.abort();let result=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),5);assert!(result["last_error"].is_null());assert_eq!(result["checking"],false);
 }
 #[tokio::test]async fn updates_disabled_worker_manual_check_and_setting_survive_initialize(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let device=t.path().join("device");let catalog=t.path().join("catalog");let app=crate::initialize_with_device(catalog.clone(),&device).unwrap();app.updates.settings(false).unwrap();
  let worker=start_with(app.updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(30)).await;assert_eq!(fixture.count(),0);
  assert_eq!(app.updates.check(&fixture.api,false,Duration::from_secs(1)).await["update_available"],true);assert_eq!(fixture.count(),1);worker.request_stop();drop(worker);drop(app);
  let app=crate::initialize_with_device(catalog,&device).unwrap();assert_eq!(app.updates.snapshot()["enabled"],false);assert_eq!(app.updates.snapshot()["latest"]["version"],"9.0.0");assert!(!app.state_dir.join("update-check.json").exists());
  let backup=crate::backup::export(&app.db,t.path()).unwrap();assert!(!backup.join("update-check.json").exists());
 }
 #[tokio::test]async fn updates_worker_wakes_on_enable_skips_recent_and_stops_promptly(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());updates.settings(false).unwrap();
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(10)).await;updates.settings(true).unwrap();checked(&updates).await;assert_eq!(fixture.count(),1);
  updates.settings(false).unwrap();updates.settings(true).unwrap();tokio::time::sleep(Duration::from_millis(20)).await;assert_eq!(fixture.count(),1);assert!(updates.delay().unwrap()>Duration::from_secs(DAY as u64-10));
  worker.request_stop();tokio::time::timeout(Duration::from_secs(1),async{while !worker.task.is_finished(){tokio::task::yield_now().await;}}).await.unwrap();
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(20)).await;assert_eq!(fixture.count(),1);worker.request_stop();
 }
 #[tokio::test]async fn updates_worker_delays_startup_and_exposes_inflight_state(){
  let fixture=Fixture::new().await;fixture.reply.lock().unwrap().delay=Duration::from_millis(80);let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::from_millis(50));tokio::time::sleep(Duration::from_millis(10)).await;assert_eq!(fixture.count(),0);
  tokio::time::timeout(Duration::from_secs(1),async{while fixture.count()==0{tokio::task::yield_now().await;}}).await.unwrap();assert_eq!(updates.snapshot()["checking"],true);updates.settings(false).unwrap();checked(&updates).await;assert_eq!(updates.snapshot()["enabled"],false);worker.request_stop();
 }
 #[tokio::test]async fn updates_damaged_state_and_role_boundaries(){
  use sha2::{Digest,Sha256};
  let t=tempfile::tempdir().unwrap();let dir=t.path().join("state");fs::create_dir(&dir).unwrap();fs::write(dir.join("update-check.json"),b"damaged").unwrap();let app=crate::initialize(dir).unwrap();assert_eq!(app.updates.snapshot()["enabled"],true);
  let web="w".repeat(72);let desktop="d".repeat(72);let cookie=format!("galroon_sid_{}={web}",&hex::encode(Sha256::digest(app.state_dir.to_string_lossy().as_bytes()))[..16]);
  for (role,token) in [("web",&web),("desktop",&desktop)]{app.db.lock().unwrap().execute("INSERT INTO sessions(token_hash,role,created,id,name,expires) VALUES(?1,?2,0,?2,?2,?3)",rusqlite::params![hex::encode(Sha256::digest(token.as_bytes())),role,crate::db::now()+3600]).unwrap();}
  // Seed a recent check so the authenticated POST handler cannot use the public network.
  app.updates.stored.lock().unwrap().last_checked=Some(crate::db::now());let router=crate::router(app.clone());
  for (role,method,path,status,body) in [("web","GET","/api/updates",200,json!(null)),("web","POST","/api/updates/check",403,json!({})),("web","POST","/api/updates/settings",403,json!({"enabled":false})),("owner","POST","/api/updates/settings",200,json!({"enabled":false})),("desktop","POST","/api/updates/settings",200,json!({"enabled":true})),("desktop","POST","/api/updates/check",200,json!({})),("owner","POST","/api/updates/check",200,json!({})),("owner","POST","/api/updates/settings",422,json!({"enabled":false,"extra":1}))]{
   let mut request=Request::builder().method(method).uri(path).header("Host","localhost").header("Content-Type","application/json");request=if role=="web"{request.header("Cookie",&cookie)}else{request.header("Authorization",format!("Bearer {}",if role=="owner"{app.token.as_str()}else{desktop.as_str()}))};
   let response=router.clone().oneshot(request.body(Body::from(body.to_string())).unwrap()).await.unwrap();assert_eq!(response.status().as_u16(),status,"{role} {path}");if status==200{let value:Value=serde_json::from_slice(&to_bytes(response.into_body(),10_000).await.unwrap()).unwrap();assert!(value.get("current").is_some());assert!(value.get("etag").is_none());}
  }
  let response=router.oneshot(Request::builder().uri("/api/updates").header("Host","localhost").body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),StatusCode::UNAUTHORIZED);
 }
 #[test]fn updates_startup_and_retry_schedule(){
  let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());let startup=Duration::from_secs(30);assert_eq!(updates.startup_delay(startup),startup);assert_eq!(updates.delay(),Some(Duration::ZERO));
  {let mut state=updates.stored.lock().unwrap();state.last_checked=Some(crate::db::now()-120);state.last_error=Some("Offline".into());}
  assert!(updates.startup_delay(startup)>Duration::from_secs(DAY as u64-130));let retry=updates.delay().unwrap();assert!(retry>Duration::from_secs(3400)&&retry<=Duration::from_secs(3480));updates.settings(false).unwrap();assert_eq!(updates.delay(),None);
 }
 #[cfg(unix)]#[test]fn updates_refuse_symlinks_and_recover_invalid_schema(){
  let t=tempfile::tempdir().unwrap();let outside=t.path().join("outside");fs::write(&outside,b"unchanged").unwrap();let device=t.path().join("device");fs::create_dir(&device).unwrap();let path=device.join("update-check.json");std::os::unix::fs::symlink(&outside,&path).unwrap();let updates=Updates::load(&device);assert!(updates.settings(false).is_err());assert_eq!(fs::read(&outside).unwrap(),b"unchanged");fs::remove_file(&path).unwrap();
  for bytes in [r#"{"version":2,"enabled":true}"#,r#"{"version":1,"enabled":"yes"}"#]{fs::write(&path,bytes).unwrap();assert_eq!(Updates::load(&device).snapshot()["enabled"],true);}
 }
}

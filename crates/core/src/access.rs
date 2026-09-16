//! Single local owner, revocable desktop credentials, and read-only browser cookies.
use crate::{App,ApiError,db::{self,Db}};
use argon2::{Argon2,PasswordHash,PasswordHasher,PasswordVerifier,password_hash::SaltString};
use axum::{extract::{State,Path,Request},http::{HeaderMap,StatusCode,header},middleware::Next,response::{Response,IntoResponse},Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
type R<T>=Result<T,String>;
fn sql<T>(r:rusqlite::Result<T>)->R<T>{r.map_err(|e|e.to_string())}
fn cookie_name(a:&App)->String{format!("galroon_sid_{}",&hash(&a.state_dir.to_string_lossy())[..16])}
const WEB_LIFETIME:i64=12*60*60;
const DEVICE_LIFETIME:i64=90*24*60*60;
#[derive(Clone,Debug)]pub struct Identity{pub id:String,pub role:String}
pub fn migrate(c:&Connection,version:i64)->rusqlite::Result<()>{
 let cols=c.prepare("PRAGMA table_info(sessions)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for(name,definition)in [("id","TEXT NOT NULL DEFAULT ''"),("name","TEXT NOT NULL DEFAULT ''"),("expires","INTEGER NOT NULL DEFAULT 0"),("last_used","INTEGER NOT NULL DEFAULT 0")]{if !cols.iter().any(|col|col==name){c.execute_batch(&format!("ALTER TABLE sessions ADD COLUMN {name} {definition}"))?;}}
 if version<5{c.execute("DELETE FROM sessions",[])?;}
 c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS sessions_id ON sessions(id) WHERE id!=''; CREATE TABLE IF NOT EXISTS pairing_codes(hash TEXT PRIMARY KEY,name TEXT NOT NULL,expires INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS access_attempts(kind TEXT PRIMARY KEY,started INTEGER NOT NULL,count INTEGER NOT NULL);")
}
fn hash(value:&str)->String{hex::encode(Sha256::digest(value.as_bytes()))}
fn secret()->String{format!("{}{}",db::id(),db::id())}
fn configured(c:&Connection)->R<bool>{Ok(sql(c.query_row("SELECT count(*) FROM settings WHERE key='owner_password'",[],|r|r.get::<_,i64>(0)))?==1)}
fn name(value:&str)->R<String>{let v=value.trim();if v.is_empty()||v.chars().count()>80||v.chars().any(char::is_control){return Err("Choose a device name between 1 and 80 characters".into());}Ok(v.into())}
fn password_hash(password:&str)->R<String>{if password.chars().count()<12||password.len()>1024{return Err("Use a password of at least 12 characters and at most 1024 bytes".into());}let salt=SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).map_err(|e|e.to_string())?;Argon2::default().hash_password(password.as_bytes(),&salt).map(|s|s.to_string()).map_err(|e|e.to_string())}
pub fn setup(db:&Db,password:&str,replace:bool)->R<()>{
 {let c=db.lock().map_err(|e|e.to_string())?;if configured(&c)?&&!replace{return Err("An owner is already configured".into());}}
 let encoded=password_hash(password)?;let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;if configured(&tx)?&&!replace{return Err("An owner is already configured".into());}
 sql(tx.execute("INSERT INTO settings(key,value) VALUES('owner_password',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[encoded]))?;
 sql(tx.execute("UPDATE sessions SET revoked=1",[]))?;sql(tx.execute("DELETE FROM pairing_codes",[]))?;sql(tx.execute("DELETE FROM access_attempts",[]))?;sql(tx.commit())?;Ok(())
}
fn budget(c:&Connection,kind:&str)->R<()>{let now=db::now();sql(c.execute("INSERT INTO access_attempts(kind,started,count) VALUES(?1,?2,1) ON CONFLICT(kind) DO UPDATE SET started=CASE WHEN ?2-started>=60 THEN ?2 ELSE started END,count=CASE WHEN ?2-started>=60 THEN 1 ELSE count+1 END",params![kind,now]))?;let count:i64=sql(c.query_row("SELECT count FROM access_attempts WHERE kind=?1",[kind],|r|r.get(0)))?;if count>10{return Err("Too many attempts. Wait one minute and try again.".into());}Ok(())}
fn issue(c:&Connection,name:&str,role:&str,lifetime:i64)->R<(String,Value)>{let token=secret();let id=db::id();let now=db::now();let expires=now+lifetime;sql(c.execute("INSERT INTO sessions(token_hash,role,created,id,name,expires,last_used) VALUES(?1,?2,?3,?4,?5,?6,?3)",params![hash(&token),role,now,id,name,expires]))?;Ok((token,json!({"id":id,"role":role,"expires":expires})))}
fn login(db:&Db,password:&str,device_name:&str)->R<(String,Value)>{
 let device_name=name(device_name)?;let encoded={let c=db.lock().map_err(|e|e.to_string())?;budget(&c,"login")?;if password.len()>1024{return Err("Unable to sign in".into());}sql(c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get::<_,String>(0)).optional())?.ok_or("Set up the owner in the local Windows app first")?};
 let valid=PasswordHash::new(&encoded).ok().is_some_and(|p|Argon2::default().verify_password(password.as_bytes(),&p).is_ok());if !valid{return Err("Unable to sign in".into());}
 let c=db.lock().map_err(|e|e.to_string())?;let current:String=sql(c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)))?;if current!=encoded{return Err("Owner settings changed. Sign in again.".into());}issue(&c,&device_name,"web",WEB_LIFETIME)
}
fn create_pair(db:&Db,device_name:&str)->R<Value>{let device_name=name(device_name)?;let c=db.lock().map_err(|e|e.to_string())?;if !configured(&c)?{return Err("Set up the owner before pairing a device".into());}let code=uuid::Uuid::new_v4().simple().to_string()[..12].to_uppercase();let expires=db::now()+300;sql(c.execute("DELETE FROM pairing_codes WHERE expires<=?1",[db::now()]))?;let count:i64=sql(c.query_row("SELECT count(*) FROM pairing_codes",[],|r|r.get(0)))?;if count>=5{return Err("Five pairing codes are already active. Wait for one to expire.".into());}sql(c.execute("INSERT INTO pairing_codes(hash,name,expires) VALUES(?1,?2,?3)",params![hash(&code),device_name,expires]))?;Ok(json!({"code":format!("{}-{}-{}",&code[..4],&code[4..8],&code[8..]),"expires":expires,"name":device_name}))}
fn redeem(db:&Db,code:&str)->R<Value>{let mut c=db.lock().map_err(|e|e.to_string())?;budget(&c,"pair")?;let code=code.replace('-',"").to_uppercase();if code.len()!=12||!code.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Pairing code is invalid or expired".into());}let tx=sql(c.transaction())?;let verifier=hash(&code);let device_name=sql(tx.query_row("SELECT name FROM pairing_codes WHERE hash=?1 AND expires>?2",params![verifier,db::now()],|r|r.get::<_,String>(0)).optional())?.ok_or("Pairing code is invalid or expired")?;let(token,mut result)=issue(&tx,&device_name,"desktop",DEVICE_LIFETIME)?;sql(tx.execute("DELETE FROM pairing_codes WHERE hash=?1",[verifier]))?;sql(tx.commit())?;result["token"]=json!(token);Ok(result)}
pub fn authenticate(a:&App,h:&HeaderMap)->R<Identity>{
 let bearer=h.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));
 if bearer.is_some_and(|value|value.len()==a.token.len()&&hash(value)==hash(&a.token)){return Ok(Identity{id:"local-owner".into(),role:"owner".into()});}
 let (token,role)=if let Some(b)=bearer{(b,"desktop")}else{let wanted=cookie_name(a);let cookies=h.get(header::COOKIE).and_then(|v|v.to_str().ok()).unwrap_or("");let token=cookies.split(';').filter_map(|part|part.trim().split_once('=')).find_map(|(key,value)|(key==wanted).then_some(value)).ok_or("Authentication required")?;(token,"web")};
 if token.len()!=72{return Err("Authentication required".into());}let c=a.db.lock().map_err(|e|e.to_string())?;let identity=sql(c.query_row("SELECT id,role FROM sessions WHERE token_hash=?1 AND role=?2 AND revoked=0 AND expires>?3",params![hash(token),role,db::now()],|r|Ok(Identity{id:r.get(0)?,role:r.get(1)?})).optional())?.ok_or("Authentication required")?;
 sql(c.execute("UPDATE sessions SET last_used=?2 WHERE id=?1 AND last_used<?2-60",params![identity.id,db::now()]))?;Ok(identity)
}
fn local(a:&App,h:&HeaderMap)->Result<(),ApiError>{if authenticate(a,h)?.role!="owner"{return Err(ApiError("Local owner access required".into()));}Ok(())}
fn origin_allowed(h:&HeaderMap)->bool{
 let Some(origin)=h.get(header::ORIGIN).and_then(|v|v.to_str().ok())else{return false};let host=h.get(header::HOST).and_then(|v|v.to_str().ok()).unwrap_or("");
 // Core currently binds loopback only. Remote TLS termination is a separate deployment boundary.
 (!host.is_empty()&&origin==format!("http://{host}"))||matches!(origin,"http://127.0.0.1:1420"|"http://localhost:1420"|"http://tauri.localhost"|"tauri://localhost")
}
pub async fn response_headers(request:Request,next:Next)->Response{
 let api=request.uri().path().starts_with("/api/");let mut response=next.run(request).await;let h=response.headers_mut();
 h.insert(header::CONTENT_SECURITY_POLICY,header::HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data: blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
 h.insert(header::X_CONTENT_TYPE_OPTIONS,header::HeaderValue::from_static("nosniff"));h.insert(header::REFERRER_POLICY,header::HeaderValue::from_static("no-referrer"));if api{h.insert(header::CACHE_CONTROL,header::HeaderValue::from_static("no-store"));}response
}
pub async fn guard(State(a):State<App>,request:Request,next:Next)->Response{
 let host=request.headers().get(header::HOST).and_then(|v|v.to_str().ok()).and_then(|v|v.parse::<axum::http::uri::Authority>().ok());
 if !host.is_some_and(|h|matches!(h.host(),"localhost"|"127.0.0.1"|"[::1]")){return (StatusCode::FORBIDDEN,Json(json!({"error":"Core currently accepts loopback hosts only"}))).into_response();}
 let path=request.uri().path();if !path.starts_with("/api/")||matches!(path,"/api/health"|"/api/access/status"){return next.run(request).await;}
 if matches!(path,"/api/access/login"|"/api/access/pair"){if !origin_allowed(request.headers()){return (StatusCode::FORBIDDEN,Json(json!({"error":"Origin not allowed"}))).into_response();}return next.run(request).await;}
 match authenticate(&a,request.headers()){
  Err(error)=>return ApiError(error).into_response(),
  Ok(who)=>{if who.role=="web"&&request.method()!=axum::http::Method::GET&&request.method()!=axum::http::Method::HEAD&&path!="/api/access/logout"{return (StatusCode::FORBIDDEN,Json(json!({"error":"This browser session is read-only"}))).into_response();}if who.role=="web"&&path=="/api/access/logout"&&!origin_allowed(request.headers()){return (StatusCode::FORBIDDEN,Json(json!({"error":"Origin not allowed"}))).into_response();}}
 }
 let library=match crate::context::library(&a.db.lock().unwrap()){Ok(v)=>v,Err(e)=>return ApiError(e.to_string()).into_response()};
 for (key,expected) in [("x-galroon-library",library.id.as_str()),("x-galroon-device",a.device.id.as_str())] {
  if request.headers().get(key).is_some_and(|v|v.to_str().ok()!=Some(expected)){
   return (StatusCode::CONFLICT,Json(json!({"error":"Collection connection changed","code":"collection_changed"}))).into_response();
  }
 }
 let mut response=next.run(request).await;
 response.headers_mut().insert(header::HeaderName::from_static("x-galroon-library"),library.id.parse().unwrap());
 response.headers_mut().insert(header::HeaderName::from_static("x-galroon-device"),a.device.id.parse().unwrap());
 response.headers_mut().insert(header::CACHE_CONTROL,header::HeaderValue::from_static("no-store"));response
}
pub async fn status(State(a):State<App>)->Result<Json<Value>,ApiError>{let c=a.db.lock().unwrap();Ok(Json(json!({"configured":configured(&c)?})))}
pub async fn me(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{let who=authenticate(&a,&h)?;Ok(Json(json!({"id":who.id,"role":who.role,"can_write":who.role!="web","can_manage_access":who.role=="owner"})))}
pub async fn configure(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{local(&a,&h)?;let password=crate::text(&v,"password")?;tokio::task::spawn_blocking(move||setup(&a.db,&password,false)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"configured":true})))}
pub async fn change_password(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{local(&a,&h)?;let password=crate::text(&v,"password")?;tokio::task::spawn_blocking(move||setup(&a.db,&password,true)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"changed":true,"sessions_revoked":true})))}
pub async fn web_login(State(a):State<App>,Json(v):Json<Value>)->Result<Response,ApiError>{let cookie_name=cookie_name(&a);let permit=a.login_gate.clone().try_acquire_owned().map_err(|_|ApiError("A sign-in is already being checked. Try again shortly.".into()))?;let password=crate::text(&v,"password")?;let device_name=v["name"].as_str().unwrap_or("Web browser").to_owned();let(token,value)=tokio::task::spawn_blocking(move||{let _permit=permit;login(&a.db,&password,&device_name)}).await.map_err(|e|ApiError(e.to_string()))??;let cookie=format!("{cookie_name}={token}; Path=/api; HttpOnly; SameSite=Strict; Max-Age={WEB_LIFETIME}");Ok(([(header::SET_COOKIE,cookie),(header::CACHE_CONTROL,"no-store".into())],Json(value)).into_response())}
pub async fn pairing(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{local(&a,&h)?;Ok(Json(create_pair(&a.db,&crate::text(&v,"name")?)?))}
pub async fn pair(State(a):State<App>,Json(v):Json<Value>)->Result<Response,ApiError>{Ok(([(header::CACHE_CONTROL,"no-store")],Json(redeem(&a.db,&crate::text(&v,"code")?)?)).into_response())}
pub async fn devices(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{local(&a,&h)?;let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT id,name,role,created,expires,last_used,revoked FROM sessions WHERE id!='' ORDER BY created DESC")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"role":r.get::<_,String>(2)?,"created":r.get::<_,i64>(3)?,"expires":r.get::<_,i64>(4)?,"last_used":r.get::<_,i64>(5)?,"revoked":r.get::<_,bool>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(Json(json!(rows)))}
pub async fn revoke(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Json<Value>,ApiError>{local(&a,&h)?;a.db.lock().unwrap().execute("UPDATE sessions SET revoked=1 WHERE id=?1",[id])?;Ok(Json(json!({"revoked":true})))}
pub async fn logout(State(a):State<App>,h:HeaderMap)->Result<Response,ApiError>{let who=authenticate(&a,&h)?;if who.role!="owner"{a.db.lock().unwrap().execute("UPDATE sessions SET revoked=1 WHERE id=?1",[who.id])?;}Ok(([(header::SET_COOKIE,format!("{}=; Path=/api; HttpOnly; SameSite=Strict; Max-Age=0",cookie_name(&a)))],Json(json!({"signed_out":true}))).into_response())}

#[cfg(test)]mod tests{
 use super::*;
 const PASSWORD:&str="Generated test owner password";
 #[test]fn pairing_is_single_use_expiring_revocable_and_backups_exclude_credentials(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();setup(&app.db,PASSWORD,false).unwrap();assert!(setup(&app.db,PASSWORD,false).is_err());let code=create_pair(&app.db,"Windows fixture").unwrap();let session=redeem(&app.db,code["code"].as_str().unwrap()).unwrap();assert!(redeem(&app.db,code["code"].as_str().unwrap()).is_err());
  let mut headers=HeaderMap::new();headers.insert(header::AUTHORIZATION,format!("Bearer {}",session["token"].as_str().unwrap()).parse().unwrap());assert_eq!(authenticate(&app,&headers).unwrap().role,"desktop");
  let expired=create_pair(&app.db,"Expired fixture").unwrap();app.db.lock().unwrap().execute("UPDATE pairing_codes SET expires=0",[]).unwrap();assert!(redeem(&app.db,expired["code"].as_str().unwrap()).is_err());
  let pending=create_pair(&app.db,"Pending fixture").unwrap();let snapshot=crate::backup::export(&app.db,t.path()).unwrap();let bytes=std::fs::read(snapshot.join("collection.sqlite")).unwrap();let password_hash:String=app.db.lock().unwrap().query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)).unwrap();for secret in [password_hash,hash(&pending["code"].as_str().unwrap().replace('-',"")),hash(session["token"].as_str().unwrap())]{assert!(!bytes.windows(secret.len()).any(|part|part==secret.as_bytes()));}
  setup(&app.db,"Replacement test owner password",true).unwrap();assert!(authenticate(&app,&headers).is_err());assert!(redeem(&app.db,pending["code"].as_str().unwrap()).is_err());
 }
 #[test]fn attempts_and_expiry_are_enforced_and_sessions_survive_restart(){
  let t=tempfile::tempdir().unwrap();let state=t.path().join("state");let app=crate::initialize(state.clone()).unwrap();setup(&app.db,PASSWORD,false).unwrap();let(token,_)=login(&app.db,PASSWORD,"Browser fixture").unwrap();let cookie=format!("{}={token}",cookie_name(&app));let mut h=HeaderMap::new();h.insert(header::COOKIE,cookie.parse().unwrap());assert_eq!(authenticate(&app,&h).unwrap().role,"web");let old_owner=app.token.clone();drop(app);
  let app=crate::initialize(state).unwrap();assert_ne!(app.token,old_owner);assert_eq!(authenticate(&app,&h).unwrap().role,"web");app.db.lock().unwrap().execute("UPDATE sessions SET expires=0",[]).unwrap();assert!(authenticate(&app,&h).is_err());
  let c=app.db.lock().unwrap();for _ in 0..10{budget(&c,"test").unwrap();}assert!(budget(&c,"test").is_err());c.execute("UPDATE access_attempts SET started=0 WHERE kind='test'",[]).unwrap();budget(&c,"test").unwrap();
 }
 #[tokio::test]async fn http_browser_cookie_cannot_mutate_and_device_revocation_is_immediate(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();setup(&app.db,PASSWORD,false).unwrap();let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let server_app=app.clone();let server=tokio::spawn(async move{crate::serve_listener(server_app,listener).await.unwrap()});let c=reqwest::Client::new();
  assert_eq!(c.get(format!("{base}/api/works")).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);
  assert_eq!(c.post(format!("{base}/api/access/login")).header("Origin","https://untrusted.invalid").json(&json!({"password":PASSWORD})).send().await.unwrap().status(),StatusCode::FORBIDDEN);
  assert_eq!(c.get(format!("{base}/api/health")).header("Host","untrusted.invalid").send().await.unwrap().status(),StatusCode::FORBIDDEN);
  let login=c.post(format!("{base}/api/access/login")).header("Origin",&base).json(&json!({"password":PASSWORD,"name":"Read-only fixture"})).send().await.unwrap();assert!(login.status().is_success());let set_cookie=login.headers()[header::SET_COOKIE].to_str().unwrap().to_owned();assert!(set_cookie.contains("HttpOnly")&&set_cookie.contains("SameSite=Strict"));let cookie=set_cookie.split(';').next().unwrap();let body:Value=login.json().await.unwrap();assert!(body.get("token").is_none());
  let read=c.get(format!("{base}/api/works")).header("Cookie",cookie).send().await.unwrap();assert!(read.status().is_success());assert_eq!(read.headers()[header::CACHE_CONTROL],"no-store");assert!(read.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("frame-ancestors 'none'"));
  app.db.lock().unwrap().execute_batch("INSERT INTO roots(id,path,label) VALUES('history-root','generated','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('history-resource','history-root','a','Fixture','archive');").unwrap();
  assert!(!c.get(format!("{base}/api/issues")).header("Cookie",cookie).send().await.unwrap().status().is_success());
  assert!(c.get(format!("{base}/api/issues")).bearer_auth(&app.token).send().await.unwrap().status().is_success());
  {let db=app.db.lock().unwrap();db.execute("INSERT INTO roots(id,path,label) VALUES('empty-source-test','generated-empty-source','Empty generated source')",[]).unwrap();crate::watching::ensure(&db).unwrap();}
  let source_page=c.get(format!("{base}/api/issues")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();let source=source_page["items"].as_array().unwrap().iter().find(|i|i["root_id"]=="empty-source-test").unwrap();assert!(source["resource_id"].is_null());assert_eq!(source["category"],"source");assert_eq!(source["title"],"Empty generated source");
  let preview=c.post(format!("{base}/api/issues/{}/scan-preview",source["id"])).bearer_auth(&app.token).json(&json!({"revision":source["revision"]})).send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();assert_eq!(preview["root_id"],"empty-source-test");
  {let db=app.db.lock().unwrap();db.execute("INSERT INTO jobs(id,kind,state,spec,message,created,updated) VALUES('old-acquire','acquire','failed',?1,'Generated failure',1,1)",[json!({"destination":"generated-destination","final_folder":"generated-target","files":[],"password":"synthetic-not-a-real-secret"}).to_string()]).unwrap();crate::acquire_issues::reconcile(&db).unwrap();}
  let issue_id:i64=app.db.lock().unwrap().query_row("SELECT seq FROM issues WHERE job_id='old-acquire'",[],|r|r.get(0)).unwrap();let task_url=format!("{base}/api/issues/{issue_id}/task");assert!(!c.get(&task_url).header("Cookie",cookie).send().await.unwrap().status().is_success());let task=c.get(&task_url).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().text().await.unwrap();assert!(!task.contains("synthetic-not-a-real-secret"));assert!(!task.contains("password"));assert_eq!(serde_json::from_str::<Value>(&task).unwrap()["id"],"old-acquire");
  assert!(!c.get(format!("{base}/api/timeline")).header("Cookie",cookie).send().await.unwrap().status().is_success());assert!(c.get(format!("{base}/api/timeline")).bearer_auth(&app.token).send().await.unwrap().status().is_success());
  assert!(!c.get(format!("{base}/api/custom-tags?include_deleted=true")).header("Cookie",cookie).send().await.unwrap().status().is_success());
  let history_url=format!("{base}/api/resources/history-resource/matching-history");
  assert_eq!(c.get(&history_url).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);
  let denied=c.get(&history_url).header("Cookie",cookie).send().await.unwrap();assert_eq!(denied.status().as_u16(),403);assert_eq!(denied.json::<Value>().await.unwrap()["error"],"Desktop access required");
  let history=c.get(&history_url).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();assert_eq!(history["items"],json!([]));
  assert_eq!(c.get(format!("{history_url}?before=0")).bearer_auth(&app.token).send().await.unwrap().status(),StatusCode::BAD_REQUEST);
  for path in ["/smart-lists/s/to-manual","/smart-lists/s/compute","/smart-lists/s","/lists/l","/custom-tags/t","/issues/1/scan-preview","/issues/1/scan","/issues/1/retry","/issues/1","/custom-tags","/matching/start","/matching/refresh","/context","/roots","/roots/r/watch","/scans","/works","/works/w","/works/w/preferred-edition","/resources/r/bind","/releases","/releases/r","/resources/r/references","/resources/r/regroup","/resources/r/regroup/preview","/resources/r/regroup/preview/page","/works/w/metadata","/works/w/refresh","/works/w/merge","/works/w/split","/roots/r/remap","/backups","/plans/organize","/plans/p/undo","/plans/p/approve","/plans/p/execute","/duplicates","/quarantine","/quarantine/q/restore","/acquire","/jobs/j/resume","/access/setup","/access/password","/access/pairing","/access/devices/x/revoke"]{let status=c.post(format!("{base}/api{path}")).header("Cookie",cookie).header("Origin",&base).json(&json!({})).send().await.unwrap().status();assert_eq!(status,StatusCode::FORBIDDEN,"Read cookie mutated {path}");}
  let code=create_pair(&app.db,"Desktop fixture").unwrap();let paired=c.post(format!("{base}/api/access/pair")).header("Origin",&base).json(&json!({"code":code["code"]})).send().await.unwrap().json::<Value>().await.unwrap();let token=paired["token"].as_str().unwrap();assert!(c.post(format!("{base}/api/works")).bearer_auth(token).json(&json!({"title":"Paired work","original_title":"Paired work"})).send().await.unwrap().status().is_success());assert_eq!(c.get(format!("{base}/api/access/devices")).bearer_auth(token).send().await.unwrap().status(),StatusCode::FORBIDDEN);
  c.post(format!("{base}/api/access/devices/{}/revoke",paired["id"].as_str().unwrap())).bearer_auth(&app.token).json(&json!({})).send().await.unwrap().error_for_status().unwrap();assert_eq!(c.get(format!("{base}/api/works")).bearer_auth(token).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);
  assert_eq!(c.post(format!("{base}/api/access/logout")).header("Cookie",cookie).header("Origin","https://untrusted.invalid").json(&json!({})).send().await.unwrap().status(),StatusCode::FORBIDDEN);c.post(format!("{base}/api/access/logout")).header("Cookie",cookie).header("Origin",&base).json(&json!({})).send().await.unwrap().error_for_status().unwrap();assert_eq!(c.get(format!("{base}/api/works")).header("Cookie",cookie).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);server.abort();
 }
}







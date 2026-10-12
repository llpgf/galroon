//! Single local owner, revocable desktop credentials, and read-only browser cookies.
use crate::{App,ApiError,db::{self,Db}};
use argon2::{Argon2,PasswordHash,PasswordHasher,PasswordVerifier,password_hash::SaltString};
use axum::{extract::{State,Path,Request},http::{HeaderMap,StatusCode,header},middleware::Next,response::{Response,IntoResponse},Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::net::{IpAddr,Ipv4Addr};
use crate::Transport;
#[derive(Clone,Copy)]pub(crate) struct SourceIp(IpAddr);
type R<T>=Result<T,String>;
fn sql<T>(r:rusqlite::Result<T>)->R<T>{r.map_err(|e|e.to_string())}
fn cookie_name(a:&App)->String{format!("galroon_sid_{}",&hash(&a.state_dir.to_string_lossy())[..16])}
pub(crate) const CREDENTIAL_INVALID:&str="Credential is no longer valid. Pair again.";
const WEB_LIFETIME:i64=12*60*60;
const DEVICE_LIFETIME:i64=90*24*60*60;
#[derive(Clone,Debug)]pub struct Identity{pub id:String,pub role:String}
pub fn migrate(c:&Connection,version:i64)->rusqlite::Result<()>{
 let cols=c.prepare("PRAGMA table_info(sessions)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for(name,definition)in [("id","TEXT NOT NULL DEFAULT ''"),("name","TEXT NOT NULL DEFAULT ''"),("expires","INTEGER NOT NULL DEFAULT 0"),("last_used","INTEGER NOT NULL DEFAULT 0")]{if !cols.iter().any(|col|col==name){c.execute_batch(&format!("ALTER TABLE sessions ADD COLUMN {name} {definition}"))?;}}
 if version<5{c.execute("DELETE FROM sessions",[])?;}
 c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS sessions_id ON sessions(id) WHERE id!=''; CREATE TABLE IF NOT EXISTS pairing_codes(hash TEXT PRIMARY KEY,name TEXT NOT NULL,expires INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS access_attempts(kind TEXT PRIMARY KEY,started INTEGER NOT NULL,count INTEGER NOT NULL);")?;
 // Schema 45: owner-selected roles and a public identifier unrelated to the code hash.
 let cols=c.prepare("PRAGMA table_info(pairing_codes)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
 if !cols.iter().any(|v|v=="role"){c.execute_batch("ALTER TABLE pairing_codes ADD COLUMN role TEXT NOT NULL DEFAULT 'desktop' CHECK(role IN ('desktop','web'));")?;}
 if !cols.iter().any(|v|v=="id"){c.execute_batch("ALTER TABLE pairing_codes ADD COLUMN id TEXT NOT NULL DEFAULT ''; UPDATE pairing_codes SET id=lower(hex(randomblob(16)));")?;}
 c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS pairing_codes_id ON pairing_codes(id);")
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
const SOURCE_RECORD_CAP:i64=1000;
fn normalize_source(source:IpAddr)->IpAddr{match source{IpAddr::V6(ip)=>ip.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(source),_=>source}}
fn budget_from(c:&Connection,kind:&str,source:IpAddr)->R<()>{
 let now=db::now();sql(c.execute("DELETE FROM access_attempts WHERE started<=?1-60",[now]))?;
 let source=normalize_source(source);let mut keys=vec![(format!("{kind}:ip:{source}"),10)];
 if let IpAddr::V6(ip)=source{let prefix=u128::from(ip)>>64;keys.push((format!("{kind}:v6-64:{prefix:016x}"),30));}
 keys.push((format!("{kind}:global"),100));
 // Reject new sources at capacity instead of evicting live budgets (which would allow rotation to reset them).
 let records:i64=sql(c.query_row("SELECT count(*) FROM access_attempts WHERE kind NOT LIKE '%:global'",[],|r|r.get(0)))?;let mut missing=0;
 for (key,limit) in &keys{let count:Option<i64>=sql(c.query_row("SELECT count FROM access_attempts WHERE kind=?1",[key],|r|r.get(0)).optional())?;
  if count.unwrap_or(0)>=*limit{return Err("Too many attempts. Wait one minute and try again.".into());}
  if count.is_none()&&!key.ends_with(":global"){missing+=1;}
 }
 if records+missing>SOURCE_RECORD_CAP{return Err("Too many attempt sources. Wait one minute and try again.".into());}
 for (key,_) in keys{sql(c.execute("INSERT INTO access_attempts(kind,started,count) VALUES(?1,?2,1) ON CONFLICT(kind) DO UPDATE SET count=count+1",params![key,now]))?;}Ok(())
}
#[cfg(test)]fn budget(c:&Connection,kind:&str)->R<()>{budget_from(c,kind,Ipv4Addr::LOCALHOST.into())}

fn issue(c:&Connection,name:&str,role:&str,lifetime:i64)->R<(String,Value)>{let token=secret();let id=db::id();let now=db::now();let expires=now+lifetime;sql(c.execute("INSERT INTO sessions(token_hash,role,created,id,name,expires,last_used) VALUES(?1,?2,?3,?4,?5,?6,?3)",params![hash(&token),role,now,id,name,expires]))?;Ok((token,json!({"id":id,"role":role,"expires":expires})))}
fn login_from(db:&Db,password:&str,device_name:&str,source:IpAddr)->R<(String,Value)>{
 let device_name=name(device_name)?;let encoded={let c=db.lock().map_err(|e|e.to_string())?;budget_from(&c,"login",source)?;if password.len()>1024{return Err("Unable to sign in".into());}sql(c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get::<_,String>(0)).optional())?.ok_or("Set up the owner in the local Windows app first")?};
 let valid=PasswordHash::new(&encoded).ok().is_some_and(|p|Argon2::default().verify_password(password.as_bytes(),&p).is_ok());if !valid{return Err("Unable to sign in".into());}
 let c=db.lock().map_err(|e|e.to_string())?;let current:String=sql(c.query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)))?;if current!=encoded{return Err("Owner settings changed. Sign in again.".into());}issue(&c,&device_name,"web",WEB_LIFETIME)
}
fn create_pair(db:&Db,device_name:&str,role:&str)->R<Value>{if !matches!(role,"desktop"|"web"){return Err("Role must be desktop or web".into());}let id=db::id();let device_name=name(device_name)?;let c=db.lock().map_err(|e|e.to_string())?;if !configured(&c)?{return Err("Set up the owner before pairing a device".into());}let code=uuid::Uuid::new_v4().simple().to_string()[..12].to_uppercase();let expires=db::now()+300;sql(c.execute("DELETE FROM pairing_codes WHERE expires<=?1",[db::now()]))?;let count:i64=sql(c.query_row("SELECT count(*) FROM pairing_codes",[],|r|r.get(0)))?;if count>=5{return Err("Five pairing codes are already active. Wait for one to expire.".into());}sql(c.execute("INSERT INTO pairing_codes(hash,name,expires,role,id) VALUES(?1,?2,?3,?4,?5)",params![hash(&code),device_name,expires,role,id]))?;Ok(json!({"id":id,"role":role,"code":format!("{}-{}-{}",&code[..4],&code[4..8],&code[8..]),"expires":expires,"name":device_name}))}
fn redeem_from(db:&Db,code:&str,role:&str,source:IpAddr)->R<(String,Value)>{let mut c=db.lock().map_err(|e|e.to_string())?;budget_from(&c,"pair",source)?;let code=code.replace('-',"").to_uppercase();if code.len()!=12||!code.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Pairing code is invalid or expired".into());}let tx=sql(c.transaction())?;let verifier=hash(&code);let device_name=sql(tx.query_row("SELECT name FROM pairing_codes WHERE hash=?1 AND expires>?2 AND role=?3",params![verifier,db::now(),role],|r|r.get::<_,String>(0)).optional())?.ok_or("Pairing code is invalid or expired")?;let(token,result)=issue(&tx,&device_name,role,if role=="web"{WEB_LIFETIME}else{DEVICE_LIFETIME})?;sql(tx.execute("DELETE FROM pairing_codes WHERE hash=?1",[verifier]))?;sql(tx.commit())?;Ok((token,result))}
#[cfg(test)]fn login(db:&Db,password:&str,device_name:&str)->R<(String,Value)>{login_from(db,password,device_name,Ipv4Addr::LOCALHOST.into())}
#[cfg(test)]fn redeem(db:&Db,code:&str)->R<Value>{let(token,mut value)=redeem_from(db,code,"desktop",Ipv4Addr::LOCALHOST.into())?;value["token"]=json!(token);Ok(value)}
pub fn authenticate(a:&App,h:&HeaderMap)->R<Identity>{
 let bearer=h.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));
 if bearer.is_some_and(|value|value.len()==a.token.len()&&hash(value)==hash(&a.token)){return Ok(Identity{id:"local-owner".into(),role:"owner".into()});}
 let (token,role)=if let Some(b)=bearer{(b,"desktop")}else{let wanted=cookie_name(a);let cookies=h.get(header::COOKIE).and_then(|v|v.to_str().ok()).unwrap_or("");let token=cookies.split(';').filter_map(|part|part.trim().split_once('=')).find_map(|(key,value)|(key==wanted).then_some(value)).ok_or("Authentication required")?;(token,"web")};
 if token.len()!=72{return Err("Authentication required".into());}let c=a.db.lock().map_err(|e|e.to_string())?;let identity=sql(c.query_row("SELECT id,role FROM sessions WHERE token_hash=?1 AND role=?2 AND revoked=0 AND expires>?3",params![hash(token),role,db::now()],|r|Ok(Identity{id:r.get(0)?,role:r.get(1)?})).optional())?.ok_or(CREDENTIAL_INVALID)?;
 sql(c.execute("UPDATE sessions SET last_used=?2 WHERE id=?1 AND last_used<?2-60",params![identity.id,db::now()]))?;Ok(identity)
}
fn local(a:&App,h:&HeaderMap)->Result<(),ApiError>{if authenticate(a,h)?.role!="owner"{return Err(ApiError("Local owner access required".into()));}Ok(())}
fn origin_allowed(h:&HeaderMap,transport:Transport)->bool{
 let Some(origin)=h.get(header::ORIGIN).and_then(|v|v.to_str().ok())else{return false};let host=h.get(header::HOST).and_then(|v|v.to_str().ok()).unwrap_or("");
 if transport==Transport::Lan{return !host.is_empty()&&origin==format!("https://{host}");}
 (!host.is_empty()&&origin==format!("http://{host}"))||matches!(origin,"http://127.0.0.1:1420"|"http://localhost:1420"|"http://tauri.localhost"|"tauri://localhost")
}
pub async fn response_headers(request:Request,next:Next)->Response{
 let api=request.uri().path().starts_with("/api/");let mut response=next.run(request).await;let h=response.headers_mut();
 h.insert(header::CONTENT_SECURITY_POLICY,header::HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data: blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
 h.insert(header::X_CONTENT_TYPE_OPTIONS,header::HeaderValue::from_static("nosniff"));h.insert(header::REFERRER_POLICY,header::HeaderValue::from_static("no-referrer"));if api{h.insert(header::CACHE_CONTROL,header::HeaderValue::from_static("no-store"));}response
}
pub(crate) async fn transport_host(State((a,transport)):State<(App,Transport)>,mut request:Request,next:Next)->Response{
 request.extensions_mut().insert(transport);
 let host=request.headers().get(header::HOST).and_then(|v|v.to_str().ok());
 let allowed=match transport{
  Transport::Loopback=>host.and_then(|v|v.parse::<axum::http::uri::Authority>().ok()).is_some_and(|h|matches!(h.host(),"localhost"|"127.0.0.1"|"[::1]")),
  Transport::Lan=>host.is_some_and(|host|a.network.as_ref().is_some_and(|n|n.enabled()&&n.allows_host(host)))
 };
 if !allowed{return (StatusCode::FORBIDDEN,Json(json!({"error":if transport==Transport::Loopback{"Core currently accepts loopback hosts only"}else{"LAN Host not allowed"}}))).into_response();}
 // Outside CORS: credentials and LAN login restrictions also cover OPTIONS and static files.
 if transport==Transport::Lan&&(request.uri().path()=="/api/access/login"||is_owner_bearer(&a,request.headers())){return (StatusCode::FORBIDDEN,Json(json!({"error":"Local owner and password login are not allowed over LAN"}))).into_response();}
 next.run(request).await
}
fn is_owner_bearer(a:&App,h:&HeaderMap)->bool{h.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer ")).is_some_and(|v|v.len()==a.token.len()&&hash(v)==hash(&a.token))}
pub async fn guard(State(a):State<App>,mut request:Request,next:Next)->Response{
 let Some(&transport)=request.extensions().get::<Transport>()else{return (StatusCode::FORBIDDEN,Json(json!({"error":"Core currently accepts loopback hosts only"}))).into_response();};
 if transport==Transport::Lan&&(request.uri().path()=="/api/access/login"||is_owner_bearer(&a,request.headers())){return StatusCode::FORBIDDEN.into_response();}
 let source=if transport==Transport::Loopback{Some(Ipv4Addr::LOCALHOST.into())}else{request.extensions().get::<axum::extract::ConnectInfo<crate::lan_tls::PeerAddr>>().map(|peer|peer.0.0.ip())};
 if let Some(source)=source{request.extensions_mut().insert(SourceIp(source));}
 let path=request.uri().path();if !path.starts_with("/api/")||matches!(path,"/api/health"|"/api/access/status"){return next.run(request).await;}
 if matches!(path,"/api/access/login"|"/api/access/pair"|"/api/access/pair-web"){if source.is_none(){return (StatusCode::FORBIDDEN,Json(json!({"error":"Socket peer address required"}))).into_response();}
  if !origin_allowed(request.headers(),transport){return (StatusCode::FORBIDDEN,Json(json!({"error":"Origin not allowed"}))).into_response();}return next.run(request).await;}
 let web;
 let route=request.extensions().get::<axum::extract::MatchedPath>().map(|v|v.as_str()).unwrap_or("").to_owned();
 match authenticate(&a,request.headers()){
  Err(error)=>return ApiError(error).into_response(),
  Ok(who)=>{web=who.role=="web";if web&&!route.is_empty()&&crate::web_projection::access(&route)==crate::web_projection::WebAccess::Deny&&matches!(*request.method(),axum::http::Method::GET|axum::http::Method::HEAD){return crate::web_projection::denied();}
   if who.role=="web"&&request.method()!=axum::http::Method::GET&&request.method()!=axum::http::Method::HEAD&&path!="/api/access/logout"{return (StatusCode::FORBIDDEN,Json(json!({"error":"This browser session is read-only"}))).into_response();}
   if who.role=="web"&&path=="/api/access/logout"&&!origin_allowed(request.headers(),transport){return (StatusCode::FORBIDDEN,Json(json!({"error":"Origin not allowed"}))).into_response();}}
 }
 let library=match crate::context::library(&a.db.lock().unwrap()){Ok(v)=>v,Err(e)=>return ApiError(e.to_string()).into_response()};
 for (key,expected) in [("x-galroon-library",library.id.as_str()),("x-galroon-device",a.device.id.as_str())] {
  if request.headers().get(key).is_some_and(|v|v.to_str().ok()!=Some(expected)){
   return (StatusCode::CONFLICT,Json(json!({"error":"Collection connection changed","code":"collection_changed"}))).into_response();
  }
 }
 let response=next.run(request).await;
 let mut response=if web{crate::web_projection::response(&a,&route,response).await}else{response};
 response.headers_mut().insert(header::HeaderName::from_static("x-galroon-library"),library.id.parse().unwrap());
 response.headers_mut().insert(header::HeaderName::from_static("x-galroon-device"),a.device.id.parse().unwrap());
 response.headers_mut().insert(header::CACHE_CONTROL,header::HeaderValue::from_static("no-store"));response
}
pub async fn status(State(a):State<App>,axum::Extension(transport):axum::Extension<Transport>)->Result<Json<Value>,ApiError>{let c=a.db.lock().unwrap();Ok(Json(json!({"configured":configured(&c)?,"transport":if transport==Transport::Lan{"lan"}else{"loopback"}})))}
pub async fn me(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{let who=authenticate(&a,&h)?;Ok(Json(json!({"id":who.id,"role":who.role,"can_write":who.role!="web","can_manage_access":who.role=="owner"})))}
pub async fn configure(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{local(&a,&h)?;let password=crate::text(&v,"password")?;tokio::task::spawn_blocking(move||setup(&a.db,&password,false)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"configured":true})))}
pub async fn change_password(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{local(&a,&h)?;let password=crate::text(&v,"password")?;tokio::task::spawn_blocking(move||setup(&a.db,&password,true)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"changed":true,"sessions_revoked":true})))}
pub(crate) async fn web_login_from(State(a):State<App>,axum::Extension(SourceIp(source)):axum::Extension<SourceIp>,axum::Extension(transport):axum::Extension<Transport>,Json(v):Json<Value>)->Result<Response,ApiError>{let cookie_name=cookie_name(&a);let permit=a.login_gate.clone().try_acquire_owned().map_err(|_|ApiError("A sign-in is already being checked. Try again shortly.".into()))?;let password=crate::text(&v,"password")?;let device_name=v["name"].as_str().unwrap_or("Web browser").to_owned();let(token,value)=tokio::task::spawn_blocking(move||{let _permit=permit;login_from(&a.db,&password,&device_name,source)}).await.map_err(|e|ApiError(e.to_string()))??;let cookie=session_cookie(&cookie_name,&token,WEB_LIFETIME,transport);Ok(([(header::SET_COOKIE,cookie),(header::CACHE_CONTROL,"no-store".into())],Json(value)).into_response())}
pub async fn pairing(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Json<Value>,ApiError>{
 local(&a,&h)?;let role=match v.get("role"){None=>"desktop",Some(v)=>v.as_str().ok_or(ApiError("Role must be desktop or web".into()))?};
 let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;
 let network=crate::network::NetworkSettings::load(&a.device_dir)?;
 let fingerprints=if let Some(settings)=network.as_ref().filter(|n|n.enabled()){let tls=crate::lan_tls::LanTls::load_or_generate(&a.device_dir,settings)?;Some((crate::lan_tls::format_spki_fingerprint(&crate::lan_tls::spki_fingerprint(&tls.cert_der)),crate::lan_tls::certificate_fingerprint(&tls.cert_der)))}else{None};
 let mut result=create_pair(&a.db,&crate::text(&v,"name")?,role)?;
 if let Some((fp,cert))=fingerprints{let addr=network.unwrap().socket_addr();let host=match addr.ip(){IpAddr::V4(ip)=>ip.to_string(),IpAddr::V6(ip)=>format!("[{ip}]")};result["pairing_string"]=json!(crate::pairing::format_pairing_string(&host,addr.port(),&result["code"].as_str().unwrap().replace('-',""),&fp)?);result["certificate_sha256"]=json!(cert);}
 Ok(Json(result))
}
pub(crate) async fn pair_from(State(a):State<App>,axum::Extension(SourceIp(source)):axum::Extension<SourceIp>,Json(v):Json<Value>)->Result<Response,ApiError>{let(token,mut value)=redeem_from(&a.db,v["code"].as_str().unwrap_or(""),"desktop",source)?;value["token"]=json!(token);Ok(([(header::CACHE_CONTROL,"no-store")],Json(value)).into_response())}
pub(crate) async fn pair_web(State(a):State<App>,axum::Extension(SourceIp(source)):axum::Extension<SourceIp>,axum::Extension(transport):axum::Extension<Transport>,Json(v):Json<Value>)->Result<Response,ApiError>{let(token,value)=redeem_from(&a.db,v["code"].as_str().unwrap_or(""),"web",source)?;Ok(([(header::SET_COOKIE,session_cookie(&cookie_name(&a),&token,WEB_LIFETIME,transport)),(header::CACHE_CONTROL,"no-store".into())],Json(value)).into_response())}
pub async fn pairing_codes(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{local(&a,&h)?;let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT id,name,role,expires FROM pairing_codes WHERE expires>?1 ORDER BY expires,id")?;let rows=s.query_map([db::now()],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"role":r.get::<_,String>(2)?,"expires":r.get::<_,i64>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(Json(json!(rows)))}
pub async fn revoke_pairing_code(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Json<Value>,ApiError>{local(&a,&h)?;a.db.lock().unwrap().execute("DELETE FROM pairing_codes WHERE id=?1",[id])?;Ok(Json(json!({"revoked":true})))}
pub async fn network_show(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{local(&a,&h)?;Ok(Json(crate::network::show(&a.device_dir)?))}
pub async fn network_set(State(a):State<App>,h:HeaderMap,Json(edit):Json<crate::network::NetworkEdit>)->Result<Json<Value>,ApiError>{local(&a,&h)?;let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;let settings=crate::network::save(&a.device_dir,edit)?;Ok(Json(json!({"settings":settings,"restart_required":true,"message":"Core restart is needed for network settings to take effect."})))}
pub async fn devices(State(a):State<App>,h:HeaderMap)->Result<Json<Value>,ApiError>{local(&a,&h)?;let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT id,name,role,created,expires,last_used,revoked FROM sessions WHERE id!='' ORDER BY created DESC")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"role":r.get::<_,String>(2)?,"created":r.get::<_,i64>(3)?,"expires":r.get::<_,i64>(4)?,"last_used":r.get::<_,i64>(5)?,"revoked":r.get::<_,bool>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(Json(json!(rows)))}
pub async fn revoke(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Json<Value>,ApiError>{local(&a,&h)?;a.db.lock().unwrap().execute("UPDATE sessions SET revoked=1 WHERE id=?1",[id])?;Ok(Json(json!({"revoked":true})))}
pub async fn logout(State(a):State<App>,axum::Extension(transport):axum::Extension<Transport>,h:HeaderMap)->Result<Response,ApiError>{let who=authenticate(&a,&h)?;if who.role!="owner"{a.db.lock().unwrap().execute("UPDATE sessions SET revoked=1 WHERE id=?1",[who.id])?;}Ok(([(header::SET_COOKIE,session_cookie(&cookie_name(&a),"",0,transport))],Json(json!({"signed_out":true}))).into_response())}

fn session_cookie(name:&str,token:&str,lifetime:i64,transport:Transport)->String{format!("{name}={token}; Path=/api; HttpOnly; SameSite=Strict; Max-Age={lifetime}{}",if transport==Transport::Lan{"; Secure"}else{""})}
#[cfg(test)]pub async fn web_login(a:State<App>,v:Json<Value>)->Result<Response,ApiError>{web_login_from(a,axum::Extension(SourceIp(Ipv4Addr::LOCALHOST.into())),axum::Extension(Transport::Loopback),v).await}

#[cfg(test)]pub async fn pair(a:State<App>,v:Json<Value>)->Result<Response,ApiError>{pair_from(a,axum::Extension(SourceIp(Ipv4Addr::LOCALHOST.into())),v).await}

#[cfg(test)]mod tests{
 use super::*;
 use tower::{Layer,ServiceExt};
 const PASSWORD:&str="Generated test owner password";
 #[tokio::test]async fn transport_host_rejects_preflight_static_unknown_and_head(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let router=crate::router(app);
  let error=json!({"error":"Core currently accepts loopback hosts only"}).to_string();
  for host in [Some("untrusted.invalid"),Some("bad host"),None]{for (method,path) in [("OPTIONS","/api/works"),("GET","/"),("GET","/api/does-not-exist"),("HEAD","/api/health")]{
   let mut request=Request::builder().method(method).uri(path).header(header::ORIGIN,"http://localhost:1420").header(header::ACCESS_CONTROL_REQUEST_METHOD,"POST");if let Some(host)=host{request=request.header(header::HOST,host);}
   let response=router.clone().oneshot(request.body(axum::body::Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),StatusCode::FORBIDDEN,"{method} {path} {host:?}");assert!(!response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
   let headers=response.headers();assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS],"nosniff");assert_eq!(headers[header::REFERRER_POLICY],"no-referrer");if path.starts_with("/api/"){assert_eq!(headers[header::CACHE_CONTROL],"no-store");}
   if method=="HEAD"{assert_eq!(response.headers()[header::CONTENT_LENGTH],error.len().to_string());}
   let body=axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap();assert_eq!(body.as_ref(),if method=="HEAD"{b""}else{error.as_bytes()});
  }}
 }
 #[tokio::test]async fn loopback_preserves_preflight_health_and_authenticated_method_not_allowed(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let token=app.token.clone();let router=crate::router(app);
  for host in ["localhost:14230","127.0.0.1:14230","[::1]:14230"]{
   let request=Request::builder().method("OPTIONS").uri("/api/works").header(header::HOST,host).header(header::ORIGIN,"http://localhost:1420").header(header::ACCESS_CONTROL_REQUEST_METHOD,"POST").header(header::ACCESS_CONTROL_REQUEST_HEADERS,"authorization,content-type").body(axum::body::Body::empty()).unwrap();
   let response=router.clone().oneshot(request).await.unwrap();assert_eq!(response.status(),StatusCode::OK);let headers=response.headers();
   assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],"http://localhost:1420");assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_CREDENTIALS],"true");assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_METHODS],"GET,POST");assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_HEADERS],"content-type,authorization,x-galroon-library,x-galroon-device");assert_eq!(headers[header::VARY],"origin, access-control-request-method, access-control-request-headers");assert!(axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap().is_empty());
   for (method,path,status) in [("GET","/api/health",StatusCode::OK),("POST","/api/access/me",StatusCode::METHOD_NOT_ALLOWED)]{
    let request=Request::builder().method(method).uri(path).header(header::HOST,host).header(header::AUTHORIZATION,format!("Bearer {token}")).body(axum::body::Body::empty()).unwrap();let response=router.clone().oneshot(request).await.unwrap();assert_eq!(response.status(),status);if status==StatusCode::METHOD_NOT_ALLOWED{assert_eq!(response.headers()[header::ALLOW],"GET,HEAD");}
   }
  }
 }
 #[tokio::test]async fn guard_rejects_requests_without_transport_even_with_owner_auth(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let token=app.token.clone();
  let router=axum::Router::new().route("/api/health",axum::routing::get(||async{"reachable"})).fallback(||async{"reachable"}).layer(axum::middleware::from_fn_with_state(app,guard));
  for path in ["/api/health","/","/api/does-not-exist"]{
   let request=Request::builder().uri(path).header(header::HOST,"localhost").header(header::AUTHORIZATION,format!("Bearer {token}")).body(axum::body::Body::empty()).unwrap();let response=router.clone().oneshot(request).await.unwrap();assert_eq!(response.status(),StatusCode::FORBIDDEN);let body=axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap();assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(),json!({"error":"Core currently accepts loopback hosts only"}));
  }
 }
 #[tokio::test]async fn transport_wrapper_marks_requests_and_preserves_head_error_before_body_stripping(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();
  let service=axum::middleware::from_fn_with_state((app,crate::Transport::Loopback),transport_host).layer(tower::service_fn(|request:Request|async move{assert_eq!(request.extensions().get::<crate::Transport>(),Some(&crate::Transport::Loopback));Ok::<_,std::convert::Infallible>(StatusCode::OK.into_response())}));
  let request=Request::builder().uri("/api/health").header(header::HOST,"localhost").extension(crate::Transport::Lan).body(axum::body::Body::empty()).unwrap();assert_eq!(service.clone().oneshot(request).await.unwrap().status(),StatusCode::OK);
  // Axum strips HEAD bodies after middleware; check the original error here as well.
  let request=Request::builder().method("HEAD").uri("/api/health").header(header::HOST,"untrusted.invalid").body(axum::body::Body::empty()).unwrap();let response=service.oneshot(request).await.unwrap();assert_eq!(response.status(),StatusCode::FORBIDDEN);let body=axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap();assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(),json!({"error":"Core currently accepts loopback hosts only"}));
 }
 #[tokio::test]async fn reserved_lan_transport_cannot_serve_requests(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let router=crate::router_for(app,crate::Transport::Lan);
  for (method,path) in [("GET","/api/health"),("GET","/"),("OPTIONS","/api/works")]{
   let request=Request::builder().method(method).uri(path).header(header::HOST,"localhost").header(header::ORIGIN,"http://localhost:1420").header(header::ACCESS_CONTROL_REQUEST_METHOD,"POST").body(axum::body::Body::empty()).unwrap();assert_eq!(router.clone().oneshot(request).await.unwrap().status(),StatusCode::FORBIDDEN);
  }
 }
 #[test]fn pairing_is_single_use_expiring_revocable_and_backups_exclude_credentials(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();setup(&app.db,PASSWORD,false).unwrap();assert!(setup(&app.db,PASSWORD,false).is_err());let code=create_pair(&app.db,"Windows fixture","desktop").unwrap();let session=redeem(&app.db,code["code"].as_str().unwrap()).unwrap();assert!(redeem(&app.db,code["code"].as_str().unwrap()).is_err());
  let mut headers=HeaderMap::new();headers.insert(header::AUTHORIZATION,format!("Bearer {}",session["token"].as_str().unwrap()).parse().unwrap());assert_eq!(authenticate(&app,&headers).unwrap().role,"desktop");
  let expired=create_pair(&app.db,"Expired fixture","desktop").unwrap();app.db.lock().unwrap().execute("UPDATE pairing_codes SET expires=0",[]).unwrap();assert!(redeem(&app.db,expired["code"].as_str().unwrap()).is_err());
  let pending=create_pair(&app.db,"Pending fixture","desktop").unwrap();let snapshot=crate::backup::export(&app.db,t.path()).unwrap();let bytes=std::fs::read(snapshot.join("collection.sqlite")).unwrap();let password_hash:String=app.db.lock().unwrap().query_row("SELECT value FROM settings WHERE key='owner_password'",[],|r|r.get(0)).unwrap();for secret in [password_hash,hash(&pending["code"].as_str().unwrap().replace('-',"")),hash(session["token"].as_str().unwrap())]{assert!(!bytes.windows(secret.len()).any(|part|part==secret.as_bytes()));}
  setup(&app.db,"Replacement test owner password",true).unwrap();assert!(authenticate(&app,&headers).is_err());assert!(redeem(&app.db,pending["code"].as_str().unwrap()).is_err());
 }
 #[test]fn attempts_and_expiry_are_enforced_and_sessions_survive_restart(){
  let t=tempfile::tempdir().unwrap();let state=t.path().join("state");let app=crate::initialize(state.clone()).unwrap();setup(&app.db,PASSWORD,false).unwrap();let(token,_)=login(&app.db,PASSWORD,"Browser fixture").unwrap();let cookie=format!("{}={token}",cookie_name(&app));let mut h=HeaderMap::new();h.insert(header::COOKIE,cookie.parse().unwrap());assert_eq!(authenticate(&app,&h).unwrap().role,"web");let old_owner=app.token.clone();drop(app);
  let app=crate::initialize(state).unwrap();assert_ne!(app.token,old_owner);assert_eq!(authenticate(&app,&h).unwrap().role,"web");app.db.lock().unwrap().execute("UPDATE sessions SET expires=0",[]).unwrap();assert!(authenticate(&app,&h).is_err());
  let c=app.db.lock().unwrap();for _ in 0..10{budget(&c,"test").unwrap();}assert!(budget(&c,"test").is_err());c.execute("UPDATE access_attempts SET started=0 WHERE kind LIKE 'test:%'",[]).unwrap();budget(&c,"test").unwrap();
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
  crate::issues::refresh(&app.db.lock().unwrap()).unwrap();
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
  let code=create_pair(&app.db,"Desktop fixture","desktop").unwrap();let paired=c.post(format!("{base}/api/access/pair")).header("Origin",&base).json(&json!({"code":code["code"]})).send().await.unwrap().json::<Value>().await.unwrap();let token=paired["token"].as_str().unwrap();assert!(c.post(format!("{base}/api/works")).bearer_auth(token).json(&json!({"title":"Paired work","original_title":"Paired work"})).send().await.unwrap().status().is_success());assert_eq!(c.get(format!("{base}/api/access/devices")).bearer_auth(token).send().await.unwrap().status(),StatusCode::FORBIDDEN);
  c.post(format!("{base}/api/access/devices/{}/revoke",paired["id"].as_str().unwrap())).bearer_auth(&app.token).json(&json!({})).send().await.unwrap().error_for_status().unwrap();assert_eq!(c.get(format!("{base}/api/works")).bearer_auth(token).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);
  assert_eq!(c.post(format!("{base}/api/access/logout")).header("Cookie",cookie).header("Origin","https://untrusted.invalid").json(&json!({})).send().await.unwrap().status(),StatusCode::FORBIDDEN);c.post(format!("{base}/api/access/logout")).header("Cookie",cookie).header("Origin",&base).json(&json!({})).send().await.unwrap().error_for_status().unwrap();assert_eq!(c.get(format!("{base}/api/works")).header("Cookie",cookie).send().await.unwrap().status(),StatusCode::UNAUTHORIZED);server.abort();
 }
}






#[cfg(test)]#[path="access_lan_tests.rs"]mod lan_tests;

#[cfg(test)]#[path="access_pairing_tests.rs"]mod pairing_tests;

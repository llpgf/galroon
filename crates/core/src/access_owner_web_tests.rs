use super::*;
use axum::{body::{Body,to_bytes},extract::ConnectInfo};
use tower::ServiceExt;
const PASSWORD:&str="Owner browser fixture password";
fn fixture()->(tempfile::TempDir,App){let t=tempfile::tempdir().unwrap();let mut a=crate::initialize(t.path().join("state")).unwrap();a.network=Some(crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830));setup(&a.db,PASSWORD,false).unwrap();(t,a)}
async fn body(response:Response)->Value{serde_json::from_slice(&to_bytes(response.into_body(),65536).await.unwrap()).unwrap()}
async fn request(a:&App,transport:Transport,method:&str,path:&str,cookie:&str,origin:Option<&str>,value:Value)->Response{
 let mut req=Request::builder().method(method).uri(path).header(header::HOST,if transport==Transport::Lan{"nas.local"}else{"localhost"}).header(header::CONTENT_TYPE,"application/json").header(header::COOKIE,cookie).extension(ConnectInfo(crate::lan_tls::PeerAddr("192.168.1.2:40000".parse().unwrap())));
 if let Some(origin)=origin{req=req.header(header::ORIGIN,origin);}
 crate::router_for(a.clone(),transport).oneshot(req.body(Body::from(value.to_string())).unwrap()).await.unwrap()
}
#[tokio::test]async fn password_login_on_both_transports_has_owner_browser_lifetime_and_cookie_only_identity(){
 let(_t,a)=fixture();
 for transport in [Transport::Loopback,Transport::Lan]{let origin=if transport==Transport::Lan{"https://nas.local"}else{"http://localhost"};let before=db::now();
  let response=request(&a,transport,"POST","/api/access/login","",Some(origin),json!({"password":PASSWORD,"role":"owner"})).await;assert_eq!(response.status(),200);let cookie=response.headers()[header::SET_COOKIE].to_str().unwrap().to_owned();assert!(cookie.contains("Path=/api; HttpOnly; SameSite=Strict; Max-Age=2592000"));assert_eq!(cookie.contains("; Secure"),transport==Transport::Lan);let cookie=cookie.split(';').next().unwrap();let value=body(response).await;assert_eq!(value["role"],"web_owner");assert!(value.get("token").is_none());assert!((before+OWNER_WEB_LIFETIME..=db::now()+OWNER_WEB_LIFETIME).contains(&value["expires"].as_i64().unwrap()));
  let stored:(String,i64)=a.db.lock().unwrap().query_row("SELECT role,expires-created FROM sessions WHERE id=?1",[value["id"].as_str().unwrap()],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(stored,("web_owner".into(),2592000));
  let response=request(&a,transport,"GET","/api/access/me",cookie,None,Value::Null).await;assert_eq!(response.status(),200);assert_eq!(body(response).await,json!({"id":value["id"],"role":"web_owner","can_write":true,"can_manage_access":false}));
  let context=body(request(&a,transport,"GET","/api/context",cookie,None,Value::Null).await).await;for capability in ["edit_catalog","file_operations","acquire_on_core"]{assert_eq!(context["capabilities"][capability],true);}assert_eq!(context["capabilities"]["manage_access"],false);
  let mut h=HeaderMap::new();h.insert(header::AUTHORIZATION,format!("Bearer {}",cookie.split_once('=').unwrap().1).parse().unwrap());h.insert(header::COOKIE,cookie.parse().unwrap());assert_eq!(authenticate(&a,&h).unwrap_err(),CREDENTIAL_INVALID);
 }
}
#[tokio::test]async fn lan_login_requires_exact_origin_socket_peer_and_keeps_login_budget(){
 let(_t,a)=fixture();
 for origin in [None,Some("http://nas.local"),Some("https://evil.local"),Some("https://nas.local:443")]{assert_eq!(request(&a,Transport::Lan,"POST","/api/access/login","",origin,json!({"password":PASSWORD})).await.status(),403);}
 let req=Request::builder().method("POST").uri("/api/access/login").header(header::HOST,"nas.local").header(header::ORIGIN,"https://nas.local").header(header::CONTENT_TYPE,"application/json").body(Body::from(json!({"password":PASSWORD}).to_string())).unwrap();assert_eq!(crate::router_for(a.clone(),Transport::Lan).oneshot(req).await.unwrap().status(),403);
 for _ in 0..10{let response=request(&a,Transport::Lan,"POST","/api/access/login","",Some("https://nas.local"),json!({"password":"wrong"})).await;assert_eq!(response.status(),400);assert_eq!(body(response).await["error"],"Unable to sign in");}
 let response=request(&a,Transport::Lan,"POST","/api/access/login","",Some("https://nas.local"),json!({"password":PASSWORD})).await;assert_eq!(response.status(),400);assert!(body(response).await["error"].as_str().unwrap().contains("Too many attempts"));
 assert_eq!(a.db.lock().unwrap().query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}
#[tokio::test]async fn cookie_writes_require_origin_and_owner_browser_can_edit(){
 let(_t,a)=fixture();let(token,_)=login(&a.db,PASSWORD,"Browser").unwrap();let cookie=format!("{}={token}",cookie_name(&a));
 for transport in [Transport::Loopback,Transport::Lan]{let origin=if transport==Transport::Lan{"https://nas.local"}else{"http://localhost"};
  for method in ["POST","PUT","PATCH","DELETE"]{for origin in [None,Some("https://foreign.local"),Some(if transport==Transport::Lan{"https://nas.local:443"}else{"http://localhost:9999"})]{let response=request(&a,transport,method,"/api/works",&cookie,origin,json!({"title":"Rejected"})).await;assert_eq!(response.status(),403);assert_eq!(body(response).await["error"],"Origin not allowed");}}
  assert_eq!(request(&a,transport,"POST","/api/works",&cookie,Some(origin),json!({"title":"Browser edit"})).await.status(),200);
  for role in ["web","web_owner"]{let(token,_)=issue(&a.db.lock().unwrap(),"Logout",role,WEB_LIFETIME).unwrap();let cookie=format!("{}={token}",cookie_name(&a));for origin in [None,Some("https://foreign.local")]{assert_eq!(request(&a,transport,"POST","/api/access/logout",&cookie,origin,json!({})).await.status(),403);}assert_eq!(request(&a,transport,"POST","/api/access/logout",&cookie,Some(origin),json!({})).await.status(),200);assert_eq!(request(&a,transport,"GET","/api/access/me",&cookie,None,Value::Null).await.status(),401);}
 }
 assert_eq!(a.db.lock().unwrap().query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}
#[tokio::test]async fn owner_browser_never_gets_owner_only_routes(){
 let(_t,a)=fixture();let(token,_)=login(&a.db,PASSWORD,"Browser").unwrap();let cookie=format!("{}={token}",cookie_name(&a));
 for transport in [Transport::Loopback,Transport::Lan]{let origin=if transport==Transport::Lan{"https://nas.local"}else{"http://localhost"};
  for (method,path,value) in [("POST","/api/access/setup",json!({"password":PASSWORD})),("POST","/api/access/password",json!({"password":PASSWORD})),("POST","/api/access/pairing",json!({"name":"No"})),("GET","/api/access/pairing-codes",json!({})),("POST","/api/access/pairing-codes/id/revoke",json!({})),("GET","/api/access/network",json!({})),("POST","/api/access/network",json!({"address":"192.168.1.1"})),("GET","/api/access/devices",json!({})),("POST","/api/access/devices/id/revoke",json!({})),("GET","/api/diagnostics",json!({})),("POST","/api/artwork/cache",json!({})),("POST","/api/backups",json!({"mode":"full","destination":"unused"}))]{let response=request(&a,transport,method,path,&cookie,Some(origin),value).await;assert_eq!(response.status(),403,"{transport:?} {method} {path}");}
 }
}
#[tokio::test]async fn old_web_sessions_stay_read_only_after_restart(){
 let(t,a)=fixture();let(token,value)=issue(&a.db.lock().unwrap(),"Old password login","web",WEB_LIFETIME).unwrap();let cookie=format!("{}={token}",cookie_name(&a));drop(a);let a=crate::initialize(t.path().join("state")).unwrap();
 let me=body(request(&a,Transport::Loopback,"GET","/api/access/me",&cookie,None,Value::Null).await).await;assert_eq!(me["role"],"web");assert_eq!(me["can_write"],false);assert_eq!(me["can_manage_access"],false);assert_eq!(request(&a,Transport::Loopback,"POST","/api/works",&cookie,Some("http://localhost"),json!({"title":"No"})).await.status(),403);
 assert_eq!(a.db.lock().unwrap().query_row("SELECT expires FROM sessions WHERE id=?1",[value["id"].as_str().unwrap()],|r|r.get::<_,i64>(0)).unwrap(),value["expires"].as_i64().unwrap());
}

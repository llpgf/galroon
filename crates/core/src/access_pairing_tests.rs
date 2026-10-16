use super::*;
use axum::{body::Body,extract::ConnectInfo};
use tower::ServiceExt;
fn fixture()->(tempfile::TempDir,App){let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("data")).unwrap();setup(&app.db,"Pairing test owner password",false).unwrap();(t,app)}
fn owner(app:&App)->HeaderMap{let mut h=HeaderMap::new();h.insert(header::AUTHORIZATION,format!("Bearer {}",app.token).parse().unwrap());h}
async fn body(response:Response)->Value{serde_json::from_slice(&axum::body::to_bytes(response.into_body(),65536).await.unwrap()).unwrap()}
async fn post(app:&App,transport:Transport,path:&str,value:Value,origin:bool)->Response{
 let host=if transport==Transport::Lan{"nas.local"}else{"localhost"};let mut request=Request::builder().method("POST").uri(path).header(header::HOST,host).header(header::CONTENT_TYPE,"application/json").extension(ConnectInfo(crate::lan_tls::PeerAddr("192.168.1.2:40000".parse().unwrap())));
 if origin{request=request.header(header::ORIGIN,format!("{}://{host}",if transport==Transport::Lan{"https"}else{"http"}));}
 crate::router_for(app.clone(),transport).oneshot(request.body(Body::from(value.to_string())).unwrap()).await.unwrap()
}
#[tokio::test]async fn roles_are_fixed_by_owner_cookie_only_and_single_use(){
 for transport in [Transport::Loopback,Transport::Lan]{let(_t,mut app)=fixture();app.network=Some(crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830));
  let status=crate::router_for(app.clone(),transport).oneshot(Request::builder().uri("/api/access/status").header(header::HOST,if transport==Transport::Lan{"nas.local"}else{"localhost"}).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(body(status).await["transport"],if transport==Transport::Lan{"lan"}else{"loopback"});
  let desktop=create_pair(&app.db,"Desktop","desktop").unwrap();let web=create_pair(&app.db,"Browser","web").unwrap();
  for (path,code,role) in [("/api/access/pair",&web,"desktop"),("/api/access/pair-web",&desktop,"web")]{let response=post(&app,transport,path,json!({"code":code["code"],"role":role}),true).await;assert_eq!(response.status(),400);assert_eq!(body(response).await,json!({"error":"Pairing code is invalid or expired"}));}
  let response=post(&app,transport,"/api/access/pair-web",json!({"code":web["code"],"role":"web_owner"}),true).await;assert_eq!(response.status(),200);let cookie=response.headers()[header::SET_COOKIE].to_str().unwrap().to_owned();assert!(cookie.contains("HttpOnly; SameSite=Strict; Max-Age=43200"));assert_eq!(cookie.contains("; Secure"),transport==Transport::Lan);let value=body(response).await;assert_eq!(value["role"],"web");assert!((db::now()+WEB_LIFETIME-5..=db::now()+WEB_LIFETIME).contains(&value["expires"].as_i64().unwrap()));assert!(value.get("token").is_none());assert!(value.get("bearer").is_none());
  let mut h=HeaderMap::new();h.insert(header::COOKIE,cookie.split(';').next().unwrap().parse().unwrap());assert_eq!(authenticate(&app,&h).unwrap().role,"web");
  assert_eq!(post(&app,transport,"/api/access/pair-web",json!({"code":web["code"]}),true).await.status(),400);
  let response=post(&app,transport,"/api/access/pair",json!({"code":desktop["code"],"role":"web"}),true).await;assert_eq!(response.status(),200);assert!(!response.headers().contains_key(header::SET_COOKIE));let value=body(response).await;assert_eq!(value["role"],"desktop");assert!(value["token"].is_string());
 }
}
#[tokio::test]async fn pairing_endpoints_share_origin_rules_and_budget(){
 let(_t,mut app)=fixture();app.network=Some(crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830));let code=create_pair(&app.db,"Browser","web").unwrap();
 for transport in [Transport::Loopback,Transport::Lan]{for path in ["/api/access/pair","/api/access/pair-web"]{assert_eq!(post(&app,transport,path,json!({"code":code["code"]}),false).await.status(),403);}}
 for i in 0..10{let response=post(&app,Transport::Loopback,if i%2==0{"/api/access/pair"}else{"/api/access/pair-web"},json!({"code":"invalid"}),true).await;assert_eq!(response.status(),400);}
 let response=post(&app,Transport::Loopback,"/api/access/pair-web",json!({"code":code["code"]}),true).await;assert!(body(response).await["error"].as_str().unwrap().contains("Too many attempts"));
}
#[tokio::test]async fn list_and_revoke_pending_codes_are_owner_only_and_never_expose_secrets(){
 let(_t,app)=fixture();let code=pairing(State(app.clone()),owner(&app),Json(json!({"name":"Default desktop"}))).await.map_err(|e|e.0).unwrap().0;assert_eq!(code["role"],"desktop");assert!(code.get("pairing_string").is_none());assert!(code.get("certificate_sha256").is_none());
 let router=crate::router(app.clone());let list=router.clone().oneshot(Request::builder().uri("/api/access/pairing-codes").header(header::HOST,"localhost").header(header::AUTHORIZATION,format!("Bearer {}",app.token)).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(list.status(),200);let list=body(list).await;assert_eq!(list,json!([{"id":code["id"],"role":"desktop","name":"Default desktop","expires":code["expires"]}]));
 let response=router.oneshot(Request::builder().method("POST").uri(format!("/api/access/pairing-codes/{}/revoke",code["id"].as_str().unwrap())).header(header::HOST,"localhost").header(header::AUTHORIZATION,format!("Bearer {}",app.token)).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),200);assert!(redeem(&app.db,code["code"].as_str().unwrap()).is_err());assert_eq!(pairing_codes(State(app.clone()),owner(&app)).await.map_err(|e|e.0).unwrap().0,json!([]));
 for role in [json!("web_owner"),json!("owner"),json!(null),json!(1)]{assert!(pairing(State(app.clone()),owner(&app),Json(json!({"name":"Bad role","role":role}))).await.is_err());}
 let web=pairing(State(app.clone()),owner(&app),Json(json!({"name":"Revoke browser","role":"web"}))).await.map_err(|e|e.0).unwrap().0;
 assert_eq!(revoke_pairing_code(State(app.clone()),owner(&app),Path(web["id"].as_str().unwrap().into())).await.map_err(|e|e.0).unwrap().0,json!({"revoked":true}));let response=post(&app,Transport::Loopback,"/api/access/pair-web",json!({"code":web["code"]}),true).await;assert_eq!(response.status(),400);assert_eq!(body(response).await,json!({"error":"Pairing code is invalid or expired"}));
 let (token,_)=issue(&app.db.lock().unwrap(),"desktop","desktop",DEVICE_LIFETIME).unwrap();let mut h=HeaderMap::new();h.insert(header::AUTHORIZATION,format!("Bearer {token}").parse().unwrap());
 for (method,path) in [("GET","/api/access/pairing-codes"),("POST","/api/access/pairing-codes/id/revoke"),("GET","/api/access/network"),("POST","/api/access/network")]{let response=crate::router(app.clone()).oneshot(Request::builder().method(method).uri(path).header(header::HOST,"localhost").header(header::AUTHORIZATION,h[header::AUTHORIZATION].clone()).header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"address":"192.168.1.1"}"#)).unwrap()).await.unwrap();assert_eq!(response.status(),403,"{path}");}
}
#[tokio::test]async fn invalid_credentials_have_distinct_401_and_absent_credentials_keep_message(){
 let(_t,app)=fixture();let mut credentials=Vec::new();
 for (role,expired) in [("desktop",false),("desktop",true),("web",false),("web",true),("web_owner",false),("web_owner",true)]{let c=app.db.lock().unwrap();let(token,value)=issue(&c,"fixture",role,if expired{-1}else{100}).unwrap();if !expired{c.execute("UPDATE sessions SET revoked=1 WHERE id=?1",[value["id"].as_str().unwrap()]).unwrap();}credentials.push((role,token));}
 credentials.push(("desktop",secret()));credentials.push(("web",secret()));
 for (role,token) in credentials{let request=Request::builder().uri("/api/access/me").header(header::HOST,"localhost");let request=if matches!(role,"web"|"web_owner"){request.header(header::COOKIE,format!("{}={token}",cookie_name(&app)))}else{request.header(header::AUTHORIZATION,format!("Bearer {token}"))};let response=crate::router(app.clone()).oneshot(request.body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),401);assert_eq!(body(response).await,json!({"error":CREDENTIAL_INVALID,"code":"credential_invalid"}));}
 let response=crate::router(app).oneshot(Request::builder().uri("/api/access/me").header(header::HOST,"localhost").body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),401);assert_eq!(body(response).await,json!({"error":"Authentication required"}));
}

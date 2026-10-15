//! Per-launch loopback transport for the WebView; the LAN credential stays native.
use crate::lan_client::ProxyRequest;
use axum::{body::{Body,to_bytes},http::{Request,StatusCode,HeaderValue},response::{IntoResponse,Response},routing::any,Router,Json};
use tower_http::cors::CorsLayer;
use std::{future::Future,time::Duration};
const ORIGINS:[&str;4]=["http://tauri.localhost","tauri://localhost","http://localhost:1420","http://127.0.0.1:1420"];
pub struct LocalProxy{pub url:String,pub key:String,task:tokio::task::JoinHandle<()>}
impl Drop for LocalProxy{fn drop(&mut self){self.task.abort();}}
fn error(status:StatusCode,code:&str,message:&str)->Response{(status,Json(serde_json::json!({"code":code,"error":message}))).into_response()}
impl LocalProxy{
 pub async fn start<F,T>(call:F)->Result<Self,String> where F:Fn(ProxyRequest)->T+Clone+Send+Sync+'static,T:Future<Output=Result<Response,String>>+Send+'static{
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e|e.to_string())?;let host=listener.local_addr().map_err(|e|e.to_string())?.to_string();let url=format!("http://{host}");let key=format!("{}{}",crate::db::id(),crate::db::id());let authorization=format!("Bearer {key}");
  let router=Router::new().fallback(any(move|request:Request<Body>|{let host=host.clone();let authorization=authorization.clone();let call=call.clone();async move{
   let headers=request.headers();
   if headers.get("host").and_then(|h|h.to_str().ok())!=Some(host.as_str())||headers.get("origin").is_some_and(|h|!h.to_str().is_ok_and(|s|ORIGINS.contains(&s))){return error(StatusCode::FORBIDDEN,"local_proxy_denied","Invalid local proxy origin or host");}
   if headers.get("authorization").and_then(|h|h.to_str().ok())!=Some(authorization.as_str()){return error(StatusCode::UNAUTHORIZED,"local_proxy_denied","Local proxy authorization required");}
   if !request.uri().path().starts_with("/api/")||!matches!(request.method().as_str(),"GET"|"POST"|"HEAD"){return error(StatusCode::BAD_REQUEST,"local_proxy_denied","Only Core API GET, POST and HEAD requests are allowed");}
   let field=|key:&str|headers.get(key).and_then(|h|h.to_str().ok()).unwrap_or("").to_owned();
   let mut input=ProxyRequest{method:request.method().to_string(),path:request.uri().to_string(),body:None,content_type:headers.get("content-type").and_then(|h|h.to_str().ok()).map(str::to_owned),library_id:field("x-galroon-library"),device_id:field("x-galroon-device")};
   let result=tokio::time::timeout(Duration::from_secs(60),async move{
    let body=to_bytes(request.into_body(),32*1024*1024).await.map_err(|_|"Local proxy request body exceeds size limit")?;if !body.is_empty(){input.body=Some(body.to_vec());}call(input).await
   }).await.unwrap_or_else(|_|Err("Core request timed out".into()));
   match result{
    Ok(mut response)=>{response.headers_mut().insert("cache-control",HeaderValue::from_static("no-store"));response},
    Err(message)=>{let(status,code)=if message=="credential_invalid"{(StatusCode::UNAUTHORIZED,"credential_invalid")}else if message.starts_with("409 collection_changed"){(StatusCode::CONFLICT,"collection_changed")}else{(StatusCode::BAD_GATEWAY,"transport_unavailable")};error(status,code,&message)}
   }
  }})).layer(CorsLayer::new().allow_origin(ORIGINS.map(|s|s.parse::<HeaderValue>().unwrap())).allow_methods([axum::http::Method::GET,axum::http::Method::POST,axum::http::Method::HEAD]).allow_headers(["authorization","content-type","x-galroon-library","x-galroon-device"].map(|s|s.parse::<axum::http::HeaderName>().unwrap())).expose_headers(["x-galroon-library","x-galroon-device","retry-after"].map(|s|s.parse::<axum::http::HeaderName>().unwrap())));
  let task=tokio::spawn(async move{let _=axum::serve(listener,router).await;});Ok(Self{url,key,task})
 }
}

#[cfg(test)]mod tests{
 use super::*;use crate::lan_client::ProxyResponse;use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
 #[tokio::test]async fn loopback_key_origin_scope_and_status_are_preserved(){
  let calls=Arc::new(AtomicUsize::new(0));let counted=calls.clone();
  let proxy=LocalProxy::start(move|request|{let calls=counted.clone();async move{calls.fetch_add(1,Ordering::SeqCst);assert_eq!(request.library_id,"library");assert_eq!(request.device_id,"device");assert_eq!(request.content_type.as_deref(),Some("application/json"));let status=request.path.rsplit('/').next().unwrap().parse().unwrap();Ok(ProxyResponse{status,headers:std::collections::BTreeMap::from([("content-type".into(),"application/json".into()),("x-galroon-library".into(),"library".into()),("x-galroon-device".into(),"device".into())]),body:request.body.unwrap()}.into_http())}}).await.unwrap();
  let client=reqwest::Client::new();let url=format!("{}/api/test/400",proxy.url);
  for(case,expected)in [("missing",401),("wrong",401),("origin",403),("host",403),("method",400),("path",400)]{
   let mut request=client.request(if case=="method"{reqwest::Method::PUT}else{reqwest::Method::POST},if case=="path"{format!("{}/outside",proxy.url)}else{url.clone()});
   if case!="missing"{request=request.bearer_auth(if case=="wrong"{"wrong"}else{&proxy.key});}if case=="origin"{request=request.header("origin","https://untrusted.example");}if case=="host"{request=request.header("host","untrusted.example");}
   assert_eq!(request.send().await.unwrap().status().as_u16(),expected,"{case}");
  }
  assert_eq!(calls.load(Ordering::SeqCst),0);
  for status in [400,401,403,409,503]{let response=client.post(format!("{}/api/test/{status}",proxy.url)).bearer_auth(&proxy.key).header("origin","http://tauri.localhost").header("x-galroon-library","library").header("x-galroon-device","device").header("content-type","application/json").body("{ \"error\": \"unchanged\" }").send().await.unwrap();assert_eq!(response.status().as_u16(),status);assert_eq!(response.headers()["x-galroon-library"],"library");assert_eq!(response.headers()["access-control-allow-origin"],"http://tauri.localhost");assert_eq!(response.text().await.unwrap(),"{ \"error\": \"unchanged\" }");}
  assert_eq!(calls.load(Ordering::SeqCst),5);
  let other=LocalProxy::start(|_|async{Err("unused".into())}).await.unwrap();assert_ne!(proxy.key,other.key);let address=proxy.url.clone();drop(proxy);tokio::task::yield_now().await;assert!(client.get(address).send().await.is_err());
 }
 #[tokio::test]async fn native_failures_keep_repair_identity_and_transport_distinct(){
  let proxy=LocalProxy::start(|request|async move{Err(match request.path.as_str(){"/api/repair"=>"credential_invalid","/api/identity"=>"409 collection_changed: saved identity",_=>"Connection changed"}.into())}).await.unwrap();
  let client=reqwest::Client::new();
  for(path,status,code)in [("repair",401,"credential_invalid"),("identity",409,"collection_changed"),("closed",502,"transport_unavailable")]{let response=client.get(format!("{}/api/{path}",proxy.url)).bearer_auth(&proxy.key).send().await.unwrap();assert_eq!(response.status().as_u16(),status);assert_eq!(response.json::<serde_json::Value>().await.unwrap()["code"],code);}
 }
}

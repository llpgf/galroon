//! Installed-Core contract check. Uses only the generated web acceptance fixture.
//! Session credentials stay inside this process and are never included in reports.
use galroon_core::{local_core::{self,Session},plans};
use reqwest::{Client,Method,header};
use serde_json::{json,Value};
use std::{collections::BTreeSet,fs,path::PathBuf,time::Duration};

async fn catalog(client:&Client,session:&Session)->Value{
 let mut result=serde_json::Map::new();
 for path in ["/works","/resources","/releases","/roots","/jobs","/lists","/custom-tags"]{
  let value:Value=client.get(format!("{}/api{path}",session.url)).bearer_auth(&session.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  result.insert(path.into(),value);
 }
 Value::Object(result)
}

#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let state=PathBuf::from(&args[1]).canonicalize().unwrap();let exe=PathBuf::from(&args[2]).canonicalize().unwrap();let output=PathBuf::from(&args[3]);
 assert!(state.to_string_lossy().contains("web-installed-002-r1"),"Only the known generated fixture is allowed");
 let session:Session=serde_json::from_value(local_core::request(&state,&exe,"session").await.unwrap()).unwrap();
 let client=Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
 let health:Value=client.get(format!("{}/api/health",session.url)).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
 assert_eq!(health["version"],env!("CARGO_PKG_VERSION"));
 let login=client.post(format!("{}/api/access/login",session.url)).header(header::ORIGIN,&session.url).json(&json!({"password":"Generated-Web-Acceptance-002!","name":"Generated write-denial acceptance"})).send().await.unwrap().error_for_status().unwrap();
 let cookie=login.headers().get(header::SET_COOKIE).expect("Web cookie").to_str().unwrap().split(';').next().unwrap().to_owned();
 let identity:Value=client.get(format!("{}/api/access/me",session.url)).header(header::COOKIE,&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
 assert_eq!(identity["role"],"web");
 let before=catalog(&client,&session).await;
 let route=regex::Regex::new(r#"\.route\("([^"]+)""#).unwrap();let parameter=regex::Regex::new(r"\{[^}]+\}").unwrap();
 let all:BTreeSet<String>=route.captures_iter(include_str!("../src/lib.rs")).map(|x|x[1].to_owned()).collect();
 let bootstrap=["/api/health","/api/access/status","/api/access/login","/api/access/pair","/api/access/logout"];
 assert!(all.len()>70);let mut results=vec![];
 for template in &all{
  if bootstrap.contains(&template.as_str()){continue;}
  let path=parameter.replace_all(template,"generated-matrix-id");
  for method in [Method::POST,Method::PUT,Method::PATCH,Method::DELETE]{
   let response=client.request(method.clone(),format!("{}{path}",session.url)).header(header::COOKIE,&cookie).header(header::ORIGIN,&session.url).json(&json!({})).send().await.unwrap();
   let status=response.status().as_u16();let body:Value=response.json().await.unwrap();
   assert_eq!(status,403,"{method} {template}");assert_eq!(body["error"],"This browser session is read-only","{method} {template}");
   results.push(json!({"route":template,"method":method.as_str(),"status":status}));
  }
 }
 assert_eq!(catalog(&client,&session).await,before,"Web probes changed catalog responses");
 let report=json!({"health":health,"installed_core_sha256":plans::hash(&exe).unwrap().1,"declared_routes":all.len(),"denied_requests":results.len(),"bootstrap_and_logout_exceptions":bootstrap,"all_denials_are_readonly_middleware":true,"catalog_responses_unchanged":true,"results":results,"scope":"Generated installed Core HTTP contract; no native/browser interaction or universal future-route claim"});
 fs::write(&output,serde_json::to_vec_pretty(&report).unwrap()).unwrap();
 println!("{}",json!({"health":report["health"],"routes":report["declared_routes"],"denied":report["denied_requests"],"catalog_unchanged":true}));
}

//! Generated task-history acceptance. Never prints credentials or job specs.
use galroon_core::{access,db,local_core::{self,Session},plans};
use reqwest::{Client,header};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,time::Instant};

#[tokio::main]
async fn main(){
 let args:Vec<_>=std::env::args().collect();
 let state=PathBuf::from(&args[1]);
 assert!(state.to_string_lossy().contains("task-history"),"Generated task fixture only");
 if args[2]=="seed"{
  assert!(!state.exists(),"Never overwrite an existing fixture");fs::create_dir_all(&state).unwrap();
  let database=db::open(&state.join("library.sqlite")).unwrap();access::setup(&database,"Generated-Task-History-002!",false).unwrap();
  let mut c=database.lock().unwrap();let tx=c.transaction().unwrap();let created=db::now()-10000;
  for i in 0..205 {tx.execute("INSERT INTO jobs(id,kind,state,spec,created,updated,current_path) VALUES(?1,'scan','completed',?2,?3,?3,?4)",rusqlite::params![format!("generated-task-{i:03}"),"{\"password\":\"generated-secret-not-for-response\"}",created+i/110,format!("Generated task {i:03} — 日本語と繁體中文")]).unwrap();}
  tx.commit().unwrap();println!("Created205 generated tasks");return;
 }
 let exe=PathBuf::from(&args[2]).canonicalize().unwrap();let output=PathBuf::from(&args[3]);
 let session:Session=serde_json::from_value(local_core::request(&state,&exe,"session").await.unwrap()).unwrap();
 let client=Client::new();
 let login=client.post(format!("{}/api/access/login",session.url)).header(header::ORIGIN,&session.url).json(&json!({"password":"Generated-Task-History-002!","name":"Generated task history acceptance"})).send().await.unwrap().error_for_status().unwrap();
 let cookie=login.headers()[header::SET_COOKIE].to_str().unwrap().split(';').next().unwrap().to_owned();
 let start=Instant::now();
 let first:Value=client.get(format!("{}/api/jobs?paged=true",session.url)).header(header::COOKIE,&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
 let first_ms=start.elapsed().as_secs_f64()*1000.;assert_eq!(first["items"].as_array().unwrap().len(),100);
 let legacy:Value=client.get(format!("{}/api/jobs",session.url)).header(header::COOKIE,&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(legacy,first["items"]);
 let mut all=first["items"].as_array().unwrap().clone();let mut next=first["next"].as_str().map(str::to_owned);let mut sizes=vec![100];
 while let Some(cursor)=next {
  let p:Value=client.get(format!("{}/api/jobs",session.url)).query(&[("paged","true"),("before",&cursor)]).header(header::COOKIE,&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
  sizes.push(p["items"].as_array().unwrap().len());all.extend(p["items"].as_array().unwrap().clone());next=p["next"].as_str().map(str::to_owned);
 }
 assert_eq!(all.iter().map(|j|j["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>(),(0..205).rev().map(|i|format!("generated-task-{i:03}")).collect::<Vec<_>>());
 assert!(!json!(all).to_string().contains("generated-secret"));
 let invalid=client.get(format!("{}/api/jobs?paged=true&before=invalid",session.url)).header(header::COOKIE,&cookie).send().await.unwrap();assert_eq!(invalid.status(),400);
 let denied=client.post(format!("{}/api/jobs/generated-task-000/resume",session.url)).header(header::COOKIE,&cookie).header(header::ORIGIN,&session.url).json(&json!({})).send().await.unwrap();assert_eq!(denied.status(),403);
 let anonymous=client.get(format!("{}/api/jobs?paged=true",session.url)).send().await.unwrap();assert_eq!(anonymous.status(),401);
 let report=json!({"core_sha256":plans::hash(&exe).unwrap().1,"tasks":205,"page_sizes":sizes,"order_and_exact_once":true,"legacy_array_equal":true,"spec_excluded":true,"invalid_cursor_status":400,"anonymous_status":401,"web_resume_status":403,"first_page_ms":first_ms,"scope":"Generated installed Core HTTP, not native UI or scale threshold"});
 fs::write(output,serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{}",report);
}

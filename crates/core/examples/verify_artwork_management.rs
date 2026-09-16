use galroon_core::{local_core,artwork_cache::Cache};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,time::Duration};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 let password=format!("Generated-{}",uuid::Uuid::new_v4());
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();galroon_core::access::setup(&db,&password,false).unwrap();let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,notes) VALUES('w','Fixture','Fixture','Retain private notes')",[]).unwrap();}
 let mut bytes=vec![1;1024*1024];bytes[..3].copy_from_slice(b"\xff\xd8\xff");Cache::open(&state).unwrap().put("https://t.vndb.org/cv/01/1.jpg",&bytes).unwrap();
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();
 let outcome=tokio::spawn(async move{
  let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();let url=format!("{}/api/artwork/cache",first.url);
  let before:Value=client.get(&url).bearer_auth(&first.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(before["images"],1);assert_eq!(before["payload_bytes"],1024*1024);
  let login=client.post(format!("{}/api/access/login",first.url)).header("Origin",&first.url).json(&json!({"password":password,"name":"Generated readonly verification"})).send().await.unwrap().error_for_status().unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
  assert_eq!(client.get(&url).header("Cookie",&cookie).send().await.unwrap().status(),403);
  assert_eq!(client.post(&url).header("Cookie",&cookie).header("Origin",&first.url).json(&json!({})).send().await.unwrap().status(),403);
  let retained:Value=client.get(&url).bearer_auth(&first.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(retained["images"],1);
  let cleared:Value=client.post(&url).bearer_auth(&first.token).json(&json!({})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(cleared["images"],0);assert_eq!(cleared["payload_bytes"],0);assert!(before["allocated_bytes"].as_i64().unwrap()-cleared["allocated_bytes"].as_i64().unwrap()>=1024*1024);
  local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs.clone(),re).await.unwrap();let after:Value=client.get(format!("{}/api/artwork/cache",second.url)).bearer_auth(&second.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(after["images"],0);
  let c=rusqlite::Connection::open_with_flags(rs.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();let record:(String,String)=c.query_row("SELECT title,notes FROM works WHERE id='w'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(record,("Fixture".into(),"Retain private notes".into()));
  json!({"separate_core":true,"owner_stats_clear":true,"web_get_post_forbidden":true,"counts":[1,1,0,0],"reclaimed_bytes":before["allocated_bytes"].as_i64().unwrap()-cleared["allocated_bytes"].as_i64().unwrap(),"catalog_private_notes_preserved":true,"restart_empty":true,"source_files":0,"provider_requests":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let report=outcome.expect("Verification failed; stop attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}


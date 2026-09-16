use galroon_core::local_core;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,path::PathBuf,time::{Duration,Instant}};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 let client=reqwest::Client::builder().timeout(Duration::from_secs(55)).build().unwrap();
 let source:Value=client.post("https://api.vndb.org/kana/vn").json(&json!({"filters":["id","=","v1"],"fields":"image.url,image.sexual,image.violence","results":1})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
 let image=source["results"][0]["image"]["url"].as_str().expect("Source artwork").to_owned();fs::write(output.join("public-source.json"),serde_json::to_vec_pretty(&source).unwrap()).unwrap();
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO settings(key,value) VALUES('discovery.v1.artwork-fixture',?1)",[source.to_string()]).unwrap();}let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();
 let outcome=tokio::spawn(async move{
  let url=format!("{}/api/artwork",first.url);let denied=client.get(&url).query(&[("url",&image)]).send().await.unwrap();assert_eq!(denied.status(),401);
  let started=Instant::now();let mut tasks=tokio::task::JoinSet::new();for _ in 0..12{let client=client.clone();let url=url.clone();let image=image.clone();let token=first.token.clone();tasks.spawn(async move{client.get(url).query(&[("url",image)]).bearer_auth(token).send().await.unwrap()});}let response=tasks.join_next().await.unwrap().unwrap();assert!(response.status().is_success(),"{}",response.status());let mime=response.headers()["content-type"].to_str().unwrap().to_owned();let bytes=response.bytes().await.unwrap();let cold_ms=started.elapsed().as_millis();while let Some(result)=tasks.join_next().await{let response=result.unwrap().error_for_status().unwrap();assert_eq!(response.bytes().await.unwrap(),bytes);}assert!(!bytes.is_empty());assert!(bytes.len()<=galroon_core::artwork_cache::MAX_IMAGE_BYTES);
  let cached=galroon_core::artwork_cache::Cache::open(&rs).unwrap().get(&image).unwrap().unwrap();assert_eq!(cached.bytes,bytes);assert_eq!(cached.mime,mime);
  local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs.clone(),re).await.unwrap();
  let started=Instant::now();let response=client.get(format!("{}/api/artwork",second.url)).query(&[("url",&image)]).bearer_auth(&second.token).send().await.unwrap().error_for_status().unwrap();let restored=response.bytes().await.unwrap();let warm_ms=started.elapsed().as_millis();assert_eq!(restored,bytes);
  let c=rusqlite::Connection::open_with_flags(rs.join("artwork-cache.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM images",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  let logs=fs::read_to_string(rs.join("logs/core.jsonl")).unwrap();let starts=logs.lines().filter(|line|serde_json::from_str::<Value>(line).unwrap()["code"]=="artwork.download.started").count();assert_eq!(starts,1);json!({"simultaneous_requests":12,"download_starts":starts,"separate_core":true,"live_provider_download":true,"restart_exact_bytes":true,"anonymous_denied":true,"bytes":bytes.len(),"mime":mime,"sha256":hex::encode(Sha256::digest(&bytes)),"cold_ms":cold_ms,"warm_ms":warm_ms,"cache_entries":1,"network_disabled":false,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let report=outcome.expect("Validation failed; stop attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}



//! Verify a supplied real cover through collection completion, offline reads and backup restore.
use galroon_core::{artwork_cache::Cache,artwork_library::Library,App};
use axum::{body::Body,http::Request};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{path::PathBuf,time::Duration};
use tower::ServiceExt;
async fn call(a:&App,path:&str,post:bool)->Vec<u8>{
 let response=galroon_core::router(a.clone()).oneshot(Request::builder().method(if post{"POST"}else{"GET"}).uri(path).header("Host","127.0.0.1").header("Authorization",format!("Bearer {}",a.token)).header("Content-Type","application/json").body(if post{Body::from("{}")}else{Body::empty()}).unwrap()).await.unwrap();
 assert_eq!(response.status(),200);axum::body::to_bytes(response.into_body(),20*1024*1024).await.unwrap().to_vec()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let image=std::fs::read(args.get(1).expect("Image file")).unwrap();let output=PathBuf::from(args.get(2).expect("Fresh output directory"));assert!(!output.exists());assert!(image.len()>8*1024*1024);assert!(image.len()<=galroon_core::artwork_cache::MAX_IMAGE_BYTES);
 std::fs::create_dir_all(&output).unwrap();let a=galroon_core::initialize(output.join("state")).unwrap();let url="https://t.vndb.org/cv/01/102501.jpg";
 let profile=json!({"vn":{"id":"v47642","title":"Large cover fixture","image":{"url":url}},"characters":[],"more":false,"fetched_at":galroon_core::db::now()});
 {let c=a.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id,cover,source_json) VALUES('fixture','Large cover fixture','Large cover fixture','v47642',?1,?2)",rusqlite::params![url,json!({"title":"Large cover fixture","original_title":"Large cover fixture","cover":url}).to_string()]).unwrap();c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v47642.1',?1)",[profile.to_string()]).unwrap();}
 Cache::open(&a.state_dir).unwrap().put(url,&image).unwrap();call(&a,"/api/artwork/offline",true).await;
 tokio::time::timeout(Duration::from_secs(20),async{loop{let s:Value=serde_json::from_slice(&call(&a,"/api/artwork/offline",false).await).unwrap();assert_ne!(s["job"]["state"],"failed","{s}");if s["job"]["state"]=="completed"{assert_eq!(s["automatic"]["pending"],0);assert_eq!(s["job"]["errors"],0);break;}tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();
 Cache::open(&a.state_dir).unwrap().clear().unwrap();assert_eq!(Library::open(&a.state_dir).unwrap().image(url).unwrap().unwrap().bytes,image);
 let path=format!("/api/artwork?url={url}");assert_eq!(call(&a,&path,false).await,image);
 let backup=galroon_core::backup::export(&a.db,&output).unwrap();let restored=galroon_core::backup::restore_new(&backup,&output.join("restored"),None).unwrap();let restored=galroon_core::initialize(restored).unwrap();assert_eq!(call(&restored,&path,false).await,image);
 let library=Library::open(&restored.state_dir).unwrap();assert_eq!(library.stats().unwrap().images,1);assert_eq!(library.profile("exploration.v4.work.v47642.1").unwrap().unwrap()["vn"]["id"],"v47642");
 let report=json!({"bytes":image.len(),"sha256":hex::encode(Sha256::digest(&image)),"collection_completion":true,"pending":0,"cache_clear_preserves_permanent_image":true,"offline_api_exact_bytes":true,"backup_restore_exact_bytes":true});std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

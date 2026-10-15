//! Identical cold fixture for measuring single-ID and batched entity downloads.
use galroon_core::{artwork_library::Library,artwork_cache::Cache};
use serde_json::{json,Value};
use std::{path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use axum::{body::Body,http::Request};use tower::ServiceExt;
async fn call(a:&galroon_core::App,path:&str,post:bool)->Value{
 let response=galroon_core::router(a.clone()).oneshot(Request::builder().method(if post{"POST"}else{"GET"}).uri(path).header("Host","127.0.0.1").header("Authorization",format!("Bearer {}",a.token)).header("Content-Type","application/json").body(if post{Body::from("{}")}else{Body::empty()}).unwrap()).await.unwrap();assert_eq!(response.status(),200);serde_json::from_slice(&axum::body::to_bytes(response.into_body(),10_000_000).await.unwrap()).unwrap()
}
fn ids(v:&Value,out:&mut Vec<String>){if v[0]=="id"&&v[1]=="="{out.push(v[2].as_str().unwrap().into());}else if let Some(rows)=v.as_array(){for row in rows{if row.is_array(){ids(row,out);}}}}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output directory"));let mode=args.get(3).map(String::as_str).unwrap_or("normal").to_owned();let expected:usize=args.get(2).expect("Expected request count").parse().unwrap();assert!(!output.exists());std::fs::create_dir_all(&output).unwrap();
 let calls=Arc::new(Mutex::new(Vec::<Value>::new()));let offline=Arc::new(AtomicBool::new(false));let observed=calls.clone();let fail=offline.clone();let start=Instant::now();
 let entered=Arc::new(tokio::sync::Notify::new());let release=Arc::new(tokio::sync::Notify::new());let arrival=entered.clone();let released=release.clone();let scenario=mode.clone();
 let app=axum::Router::new().fallback(axum::routing::post(move|uri:axum::http::Uri,axum::Json(body):axum::Json<Value>|{let calls=observed.clone();let offline=fail.clone();let entered=arrival.clone();let release=released.clone();let mode=scenario.clone();async move{
  let mut selected=vec![];ids(&body["filters"],&mut selected);let path=uri.path().to_owned();calls.lock().unwrap().push(json!({"path":path,"ids":selected,"milliseconds":start.elapsed().as_millis()}));
  let first=calls.lock().unwrap().len()==1;
  if first&&mode=="pause"{entered.notify_one();release.notified().await;}
  if mode=="rate-limit"{return (axum::http::StatusCode::TOO_MANY_REQUESTS,axum::Json(json!({})));}
  if offline.load(Ordering::SeqCst)||path=="/vn"{return (axum::http::StatusCode::SERVICE_UNAVAILABLE,axum::Json(json!({})));}
  assert!(!selected.is_empty()&&selected.len()<=20);assert!(body["results"].as_u64().unwrap()>=selected.len() as u64);
  let prefix=match path.as_str(){"/staff"=>'s',"/character"=>'c',"/producer"=>'p',_=>panic!("Unexpected request")};
  for id in &selected{assert!(id.starts_with(prefix));let n=id[1..].parse::<usize>().unwrap();assert!((1..=if prefix=='p'{3}else{23}).contains(&n));}
  let mut rows:Vec<_>=selected.iter().rev().map(|id|json!({"id":id,"name":format!("Name {id}"),"description":format!("Biography {id}"),"vns":[{"id":"v1","spoiler":0},{"id":"v2","spoiler":0},{"id":"v999","spoiler":0}]})).collect();
  if first&&mode=="missing"{rows.remove(0);}
  (axum::http::StatusCode::OK,axum::Json(json!({"results":rows,"more":false})))
 }}));
 let socket=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();std::env::set_var("GALROON_VNDB_API",format!("http://{}",socket.local_addr().unwrap()));let server=tokio::spawn(async move{axum::serve(socket,app).await.unwrap()});
 let a=galroon_core::initialize(output.join("state")).unwrap();let cover="https://t.vndb.org/cv/01/1.jpg";
 for vn in ["v1","v2"]{
  let characters:Vec<_>=(1..=23).map(|n|json!({"id":format!("c{n}"),"vns":[{"id":vn,"spoiler":0}]})).collect();let staff:Vec<_>=(1..=23).map(|n|json!({"id":format!("s{n}"),"name":format!("Name s{n}")})).collect();let companies:Vec<_>=(1..=3).map(|n|json!({"id":format!("p{n}")})).collect();
  let profile=json!({"vn":{"id":vn,"title":format!("Owned {vn}"),"staff":staff,"va":[],"developers":companies,"image":{"url":cover},"relations":[{"id":"v999"}]},"characters":characters,"more":false,"fetched_at":galroon_core::db::now()});
  let c=a.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id,cover) VALUES(?1,'Owned','Owned',?1,?2)",rusqlite::params![vn,cover]).unwrap();c.execute("INSERT INTO settings VALUES(?1,?2)",rusqlite::params![format!("exploration.v4.work.{vn}.1"),profile.to_string()]).unwrap();
 }
 let bytes=b"\x89PNG\r\n\x1a\nfixture";Cache::open(&a.state_dir).unwrap().put(cover,bytes).unwrap();let began=Instant::now();
 let mut previous=String::new();
 for pass in 0..2{
  let retry=pass==1&&matches!(mode.as_str(),"missing"|"rate-limit");
  let path=if retry{format!("/api/jobs/{previous}/resume")}else{"/api/artwork/offline".into()};
  let job=call(&a,&path,true).await;previous=job["id"].as_str().unwrap().to_owned();
  if pass==0&&mode=="pause"{
   tokio::time::timeout(Duration::from_secs(5),entered.notified()).await.unwrap();call(&a,&format!("/api/jobs/{previous}/pause"),true).await;release.notify_one();
   tokio::time::timeout(Duration::from_secs(5),async{loop{let s=call(&a,"/api/artwork/offline",false).await;if s["job"]["state"]=="paused"{assert_eq!(s["job"]["processed"],0);break;}tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();
   assert_eq!(calls.lock().unwrap().len(),1);assert_eq!(Library::open(&a.state_dir).unwrap().stats().unwrap().profiles,21);call(&a,&format!("/api/jobs/{previous}/resume"),true).await;
  }
  tokio::time::timeout(Duration::from_secs(160),async{loop{
   let s=call(&a,"/api/artwork/offline",false).await;
   if mode=="rate-limit"||(mode=="missing"&&pass==0){if s["job"]["state"]=="failed"{assert!(s["job"]["errors"].as_i64().unwrap()>0);break;}}else{assert_ne!(s["job"]["state"],"failed","{s}");if s["job"]["state"]=="completed"{assert_eq!(s["job"]["errors"],0);break;}}
   tokio::time::sleep(Duration::from_millis(20)).await;
  }}).await.unwrap();
 }
 if mode=="rate-limit"{assert_eq!(calls.lock().unwrap().len(),1);assert_eq!(Library::open(&a.state_dir).unwrap().stats().unwrap().profiles,2);let report=json!({"requests":1,"immediate_retry_during_429_cooldown_makes_no_requests":true,"incomplete_entities_not_marked_complete":true});std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");server.abort();return;}

 let elapsed=began.elapsed().as_secs_f64();let requests=calls.lock().unwrap().clone();assert_eq!(requests.len(),expected);for pair in requests.windows(2){assert!(pair[1]["milliseconds"].as_u64().unwrap()-pair[0]["milliseconds"].as_u64().unwrap()>=1590);}
 let library=Library::open(&a.state_dir).unwrap();let saved=library.profiles().unwrap();assert_eq!(saved.len(),51);for (key,value) in &saved{let value:Value=serde_json::from_str(value).unwrap();assert!(value.get("works").is_none());if key.starts_with("collection."){let parts:Vec<_>=key.split('.').collect();assert_eq!(value[parts[2]]["id"],parts[3]);assert_eq!(value[parts[2]]["description"],format!("Biography {}",parts[3]));assert!(!value.to_string().contains("v999"));}}
 assert_eq!(library.image(cover).unwrap().unwrap().bytes,bytes);
 let backup=galroon_core::backup::export(&a.db,&output).unwrap();let restored=galroon_core::backup::restore_new(&backup,&output.join("restored"),None).unwrap();let restored=galroon_core::initialize(restored).unwrap();assert_eq!(Library::open(&restored.state_dir).unwrap().profiles().unwrap(),saved);offline.store(true,Ordering::SeqCst);
 let person=call(&restored,"/api/people/s1",false).await;assert_eq!(person["person"]["description"],"Biography s1");assert_eq!(person["collection_only"],true);assert_eq!(person["works"].as_array().unwrap().len(),2);
 let report=json!({"scenario":mode,"elapsed_seconds":elapsed,"requests":requests,"request_count":expected,"entities":49,"works":2,"permanent_profiles":51,"scope_and_identity_valid":true,"shared_entities_and_repeat_need_no_requests":true,"backup_roundtrip_equal":true,"offline_biography_and_collection_credits":true});std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{}",json!({"seconds":elapsed,"requests":expected,"profiles":51}));server.abort();
}

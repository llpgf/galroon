//! Generated fixture: prove background downloads never request entity VN lists.
//! Optional backup mode migrates/restores only a new disposable output directory.
use galroon_core::{artwork_library::Library,artwork_cache::Cache};
use serde_json::{json,Value};
use std::{path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
use axum::{body::Body,http::Request};use tower::ServiceExt;
async fn call(a:&galroon_core::App,path:&str,post:bool)->Value{
 let response=galroon_core::router(a.clone()).oneshot(Request::builder().method(if post{"POST"}else{"GET"}).uri(path).header("Host","127.0.0.1").header("Authorization",format!("Bearer {}",a.token)).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),200);serde_json::from_slice(&axum::body::to_bytes(response.into_body(),10_000_000).await.unwrap()).unwrap()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output directory"));assert!(!output.exists());std::fs::create_dir_all(&output).unwrap();
 if let Some(source)=args.get(2){
  let start=std::time::Instant::now();let restored=galroon_core::backup::restore_new(&PathBuf::from(source),&output.join("state"),None).unwrap();let a=galroon_core::initialize(restored).unwrap();let library=Library::open(&a.state_dir).unwrap();let stats=library.stats().unwrap();let saved=library.profiles().unwrap();assert!(saved.iter().all(|(k,v)|!k.starts_with("exploration.v3.")&&(!k.starts_with("collection.v1.")||serde_json::from_str::<Value>(v).unwrap().get("works").is_none())));
  let backup=galroon_core::backup::export(&a.db,&output).unwrap();let second=galroon_core::backup::restore_new(&backup,&output.join("roundtrip"),None).unwrap();let second=galroon_core::initialize(second).unwrap();assert_eq!(Library::open(&second.state_dir).unwrap().profiles().unwrap(),saved);
  let report=json!({"elapsed_seconds":start.elapsed().as_secs_f64(),"stats":stats,"backup":backup,"roundtrip_profiles_equal":true});std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");return;
 }
 let calls=Arc::new(Mutex::new(Vec::<String>::new()));let offline=Arc::new(AtomicBool::new(false));let observed=calls.clone();let fail=offline.clone();
 let app=axum::Router::new().fallback(axum::routing::post(move|uri:axum::http::Uri|{let calls=observed.clone();let offline=fail.clone();async move{
  let path=uri.path().to_owned();calls.lock().unwrap().push(path.clone());
  if offline.load(Ordering::SeqCst)||path=="/vn"{return (axum::http::StatusCode::SERVICE_UNAVAILABLE,axum::Json(json!({})));}
  let id=match path.as_str(){"/staff"=>"s1","/character"=>"c1","/producer"=>"p1",_=>panic!("Unexpected request")};
  (axum::http::StatusCode::OK,axum::Json(json!({"results":[{"id":id,"name":"Biography","description":"Permanent basics","vns":[{"id":"v1","spoiler":0},{"id":"v999","spoiler":0}]}],"more":false})))
 }}));
 let socket=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();std::env::set_var("GALROON_VNDB_API",format!("http://{}",socket.local_addr().unwrap()));let server=tokio::spawn(async move{axum::serve(socket,app).await.unwrap()});
 let a=galroon_core::initialize(output.join("state")).unwrap();let cover="https://t.vndb.org/cv/01/1.jpg";
 let profile=json!({"vn":{"id":"v1","title":"Owned","staff":[{"id":"s1"}],"va":[],"developers":[{"id":"p1"}],"image":{"url":cover},"relations":[{"id":"v999"}]},"characters":[{"id":"c1","vns":[{"id":"v1","spoiler":0}]}],"more":false,"fetched_at":galroon_core::db::now()});
 {let c=a.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id,cover) VALUES('w','Owned','Owned','v1',?1)",[cover]).unwrap();c.execute("INSERT INTO settings VALUES('exploration.v4.work.v1.1',?1)",[profile.to_string()]).unwrap();}
 let bytes=b"\x89PNG\r\n\x1a\nfixture";Cache::open(&a.state_dir).unwrap().put(cover,bytes).unwrap();
 for _ in 0..2{let job=call(&a,"/api/artwork/offline",true).await;tokio::time::timeout(Duration::from_secs(20),async{loop{let s=call(&a,"/api/artwork/offline",false).await;if s["job"]["state"]=="completed"{assert_eq!(s["job"]["id"],job["id"]);assert_eq!(s["job"]["errors"],0);break;}assert_ne!(s["job"]["state"],"failed","{s}");tokio::time::sleep(Duration::from_millis(20)).await;}}).await.unwrap();}
 let requests=calls.lock().unwrap().clone();assert_eq!(requests.len(),3);assert!(!requests.iter().any(|s|s=="/vn"));
 let library=Library::open(&a.state_dir).unwrap();assert_eq!(library.stats().unwrap().profiles,4);assert_eq!(library.image(cover).unwrap().unwrap().bytes,bytes);assert!(library.profile("collection.v1.character.c1").unwrap().unwrap()["character"]["vns"].as_array().unwrap().iter().all(|v|v["id"]=="v1"));
 let backup=galroon_core::backup::export(&a.db,&output).unwrap();let restored=galroon_core::backup::restore_new(&backup,&output.join("restored"),None).unwrap();let restored=galroon_core::initialize(restored).unwrap();offline.store(true,Ordering::SeqCst);
 let person=call(&restored,"/api/people/s1",false).await;assert_eq!(person["person"]["description"],"Permanent basics");assert_eq!(person["collection_only"],true);assert_eq!(person["stale"],true);assert_eq!(person["works"][0]["id"],"v1");assert!(person["warning"].as_str().unwrap().contains("collection credits only"));assert_eq!(Library::open(&restored.state_dir).unwrap().image(cover).unwrap().unwrap().bytes,bytes);
 let report=json!({"background_requests":requests,"background_vn_list_requests":0,"second_download_requests":0,"permanent_profiles":4,"offline_restored_biography_collection_credits_image":true});std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");server.abort();
}

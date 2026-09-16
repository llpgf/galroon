use galroon_core::local_core;
use std::{fs,path::PathBuf,time::{Instant,Duration}};
use serde_json::{json,Value};
fn events(path:&std::path::Path,code:&str)->usize{fs::read_to_string(path).unwrap_or_default().lines().filter_map(|line|serde_json::from_str::<Value>(line).ok()).filter(|v|v["code"]==code).count()}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh fixture directory"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();for n in 0..100000{let id=format!("w{n:06}");tx.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?1,?1)",[id]).unwrap();}
 let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});
 for id in ["a","b"]{tx.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES(?1,?1,'',1,?2)",rusqlite::params![id,definition.to_string()]).unwrap();}
 // Isolate manual cancellation: leave automatic invalidation clean for this fixture.
 tx.execute("UPDATE smart_catalog_state SET root_digest='{}'",[]).unwrap();
 tx.execute("INSERT INTO smart_compute_state(list_id,catalog_revision,last_attempt,error) SELECT id,(SELECT revision FROM smart_catalog_state),0,'' FROM smart_lists",[]).unwrap();tx.commit().unwrap();}
 let session=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let logs=state.join("logs/core.jsonl");let inspect=state.clone();
 let outcome=tokio::spawn(async move{
 let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();
 let mut tasks=Vec::new();for id in ["a","b"]{let request=client.post(format!("{}/api/smart-lists/{id}/compute",session.url)).bearer_auth(&session.token).json(&json!({"revision":1,"request_id":id}));tasks.push(tokio::spawn(async move{request.send().await}));}
 let deadline=Instant::now()+Duration::from_secs(10);while events(&logs,"smart.manual.started")<2{assert!(Instant::now()<deadline,"Both computations must start");tokio::time::sleep(Duration::from_millis(20)).await;}
 assert_eq!(events(&logs,"smart.manual.completed"),0);
 let rejected=client.post(format!("{}/api/smart-lists/a/compute",session.url)).bearer_auth(&session.token).json(&json!({"revision":1,"request_id":"third"})).send().await.unwrap();assert!(!rejected.status().is_success());
 let start=Instant::now();for task in tasks{task.abort();let _=task.await;}
 let deadline=Instant::now()+Duration::from_secs(3);while events(&logs,"smart.manual.cancelled")<2{assert!(Instant::now()<deadline,"HTTP abort did not cancel computations");tokio::time::sleep(Duration::from_millis(20)).await;}
 let cancellation_ms=start.elapsed().as_millis();
 {let c=rusqlite::Connection::open_with_flags(inspect.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();for table in ["smart_snapshots","smart_snapshot_entries","smart_snapshot_requests"]{assert_eq!(c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get::<_,i64>(0)).unwrap(),0,"Cancellation left data in {table}");}}
 let response=client.post(format!("{}/api/smart-lists/a/compute",session.url)).bearer_auth(&session.token).json(&json!({"revision":1,"request_id":"a"})).send().await.unwrap();assert!(response.status().is_success());let result:Value=response.json().await.unwrap();let snapshot=result["snapshot"].as_str().unwrap();
 let c=rusqlite::Connection::open_with_flags(inspect.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_entries WHERE snapshot_id=?1",[snapshot],|r|r.get::<_,i64>(0)).unwrap(),100000);assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_requests",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 json!({"generated_works":100000,"started_manual_requests":2,"capacity_rejected":true,"http_abort_cancelled_both":true,"cancellation_ms":cancellation_ms,"zero_partial_snapshots_entries_receipts":true,"same_request_retry_completed":true,"background_isolated":true,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let report=outcome.expect("Manual cancellation failed; cleanup attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

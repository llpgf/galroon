use galroon_core::local_core;
use serde_json::json;
use std::{path::PathBuf,fs,time::{Duration,Instant},collections::BTreeMap};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh fixture directory"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();
 assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 let old={let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
 let tx=c.transaction().unwrap();for n in 0..100000{let id=format!("w{n:06}");tx.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?1,?1)",[id]).unwrap();}
 tx.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','Stop fixture','',1,?1)",[json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}}).to_string()]).unwrap();
 let snapshot=galroon_core::smart_snapshots::build(&tx,"s",&galroon_core::smart_snapshots::Build{revision:1,request_id:"baseline".into()},&BTreeMap::new()).ok().unwrap();tx.commit().unwrap();snapshot};
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let library=first.library_id.clone();
 let task_state=state.clone();let task_exe=exe.clone();
 let outcome=tokio::spawn(async move{
 let deadline=Instant::now()+Duration::from_secs(20);
 loop{let log=fs::read_to_string(task_state.join("logs/core.jsonl")).unwrap_or_default();if log.contains("smart.compute.started"){assert!(!log.contains("smart.compute.completed"));break;}assert!(Instant::now()<deadline,"Worker never started");tokio::time::sleep(Duration::from_millis(20)).await;}
 let client=reqwest::Client::builder().timeout(Duration::from_secs(2)).build().unwrap();let mut read_ms=Vec::new();
 for _ in 0..5{let start=Instant::now();let response=client.get(format!("{}/api/smart-lists/s/results?snapshot={old}",first.url)).bearer_auth(&first.token).send().await.unwrap();assert!(response.status().is_success());let page:serde_json::Value=response.json().await.unwrap();assert_eq!(page["total"],100000);assert_eq!(page["entries"].as_array().unwrap().len(),50);read_ms.push(start.elapsed().as_millis());}
 assert!(!fs::read_to_string(task_state.join("logs/core.jsonl")).unwrap().contains("smart.compute.completed"),"Reads must overlap computation");
 let read_max_ms=*read_ms.iter().max().unwrap();assert!(read_max_ms<=300,"Concurrent page reads exceeded300ms");
 let start=Instant::now();local_core::request(&task_state,&task_exe,"stop").await.unwrap();let stop_ms=start.elapsed().as_millis();assert!(stop_ms<5000,"Stop exceeded 5 seconds");
 tokio::time::sleep(Duration::from_millis(300)).await;
 {let c=rusqlite::Connection::open_with_flags(task_state.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_entries",[],|r|r.get::<_,i64>(0)).unwrap(),100000);
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_requests",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
 let log=fs::read_to_string(task_state.join("logs/core.jsonl")).unwrap();assert!(log.contains("smart.compute.cancelled"),"Expected cancellation evidence");assert!(!log.contains("smart.compute.completed"));
 let restarted=local_core::connect_or_start(task_state.clone(),task_exe.clone()).await.unwrap();assert_eq!(restarted.library_id,library);
 let deadline=Instant::now()+Duration::from_secs(30);loop{let log=fs::read_to_string(task_state.join("logs/core.jsonl")).unwrap();if log.contains("smart.compute.completed"){break;}assert!(Instant::now()<deadline,"Restart did not recompute");tokio::time::sleep(Duration::from_millis(100)).await;}
 local_core::request(&task_state,&task_exe,"stop").await.unwrap();tokio::time::sleep(Duration::from_millis(300)).await;
 let c=rusqlite::Connection::open_with_flags(task_state.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_entries WHERE snapshot_id=?1",[old],|r|r.get::<_,i64>(0)).unwrap(),100000);
 json!({"generated_works":100000,"observed_worker_started":true,"stop_ms":stop_ms,"concurrent_page_reads":5,"concurrent_read_max_ms":read_max_ms,"cancelled_transaction_rolled_back":true,"old_entries_retained":100000,"restart_recomputed":true,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 if outcome.is_err(){let _=local_core::request(&state,&exe,"stop").await;}
 let report=outcome.expect("Native stop validation failed; cleanup attempted");fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

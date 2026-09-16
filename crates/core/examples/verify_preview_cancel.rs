use galroon_core::local_core;
use std::{fs,path::PathBuf,time::{Instant,Duration}};
use serde_json::{json,Value};
fn events(path:&std::path::Path,code:&str)->usize{fs::read_to_string(path).unwrap_or_default().lines().filter_map(|line|serde_json::from_str::<Value>(line).ok()).filter(|v|v["code"]==code).count()}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh fixture directory"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();for n in 0..100000{let id=format!("w{n:06}");tx.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?1,?1)",[id]).unwrap();}tx.commit().unwrap();}
 let session=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let logs=state.join("logs/core.jsonl");
 let outcome=tokio::spawn(async move{
 let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();
 let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});
 let mut tasks=Vec::new();for _ in 0..2{let request=client.post(format!("{}/api/smart-lists/preview",session.url)).bearer_auth(&session.token).json(&definition);tasks.push(tokio::spawn(async move{request.send().await}));}
 let deadline=Instant::now()+Duration::from_secs(10);while events(&logs,"smart.preview.started")<2{assert!(Instant::now()<deadline,"Both previews must start");tokio::time::sleep(Duration::from_millis(20)).await;}
 assert_eq!(events(&logs,"smart.preview.completed"),0);
 let rejected=client.post(format!("{}/api/smart-lists/preview",session.url)).bearer_auth(&session.token).json(&definition).send().await.unwrap();assert!(!rejected.status().is_success());
 let start=Instant::now();for task in tasks{task.abort();let _=task.await;}
 let deadline=Instant::now()+Duration::from_secs(3);while events(&logs,"smart.preview.cancelled")<2{assert!(Instant::now()<deadline,"HTTP abort did not cancel server previews");tokio::time::sleep(Duration::from_millis(20)).await;}
 let cancellation_ms=start.elapsed().as_millis();
 let response=client.post(format!("{}/api/smart-lists/preview",session.url)).bearer_auth(&session.token).json(&definition).send().await.unwrap();assert!(response.status().is_success());let result:Value=response.json().await.unwrap();assert_eq!(result["total"],100000);assert_eq!(result["sample"].as_array().unwrap().len(),10);
 json!({"generated_works":100000,"started_previews":2,"capacity_rejected":true,"http_abort_cancelled_both":true,"cancellation_ms":cancellation_ms,"next_preview_completed":true,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let report=outcome.expect("Preview validation failed; cleanup attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

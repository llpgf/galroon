use galroon_core::local_core::{self,Session};
use serde_json::{json,Value};
use std::{path::PathBuf,time::Duration,fs};
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{
 let client=reqwest::Client::new();let url=format!("{}/api{path}",s.url);let request=if let Some(v)=body{client.post(url).json(&v)}else{client.get(url)};let r=request.bearer_auth(&s.token).send().await.unwrap();assert!(r.status().is_success(),"API {path}: {}",r.status());r.json().await.unwrap()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh generated-fixture output directory required"));let exe=PathBuf::from(args.get(2).expect("Core executable required")).canonicalize().unwrap();assert!(!output.exists(),"Use a fresh fixture");fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");let source=output.join("source");fs::create_dir(&source).unwrap();for i in 0..1200{fs::write(source.join(format!("fixture-{i}.bin")),[i as u8;32]).unwrap();}
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.pid,std::process::id());
 let second=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_eq!(first.instance_id,second.instance_id);assert_eq!(first.pid,second.pid);
 assert!(local_core::request(&state,&std::env::current_exe().unwrap(),"session").await.is_err(),"Wrong executable identity must be refused");
 assert!(galroon_core::initialize(state.clone()).is_err(),"A second writer must be refused");
 let root=api(&first,"/roots",Some(json!({"path":source,"label":"Lifecycle fixture"}))).await;
 let job=api(&first,"/scans",Some(json!({"root_id":root["id"],"scope":"","exclude":[]}))).await;
 let stop=local_core::request(&state,&exe,"stop").await;assert!(stop.as_ref().err().is_some_and(|e|e.contains("Pause running tasks")),"Active stop should be refused: {stop:?}");
 let until=tokio::time::Instant::now()+Duration::from_secs(60);loop{let jobs=api(&first,"/jobs",None).await;let j=jobs.as_array().unwrap().iter().find(|j|j["id"]==job["id"]).unwrap();if j["state"]=="completed"{assert_eq!(j["processed"],1200);assert_eq!(j["errors"],0);break;}assert!(tokio::time::Instant::now()<until,"Scan timed out: {j}");tokio::time::sleep(Duration::from_millis(100)).await;}
 let saved_library=first.library_id.clone();let saved_device=first.device_id.clone();let saved_instance=first.instance_id.clone();drop(first);drop(second);
 let reconnect=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_eq!(reconnect.instance_id,saved_instance);local_core::request(&state,&exe,"stop").await.unwrap();
 // The graceful server closes its listener and database before a new instance.
 tokio::time::sleep(Duration::from_millis(300)).await;
 let restarted=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(restarted.instance_id,saved_instance);assert_eq!(restarted.library_id,saved_library);assert_eq!(restarted.device_id,saved_device);assert_eq!(api(&restarted,"/dashboard",None).await["files"],1200);local_core::request(&state,&exe,"stop").await.unwrap();
 let unexpected:Vec<_>=fs::read_dir(&state).unwrap().filter_map(Result::ok).map(|e|e.file_name().to_string_lossy().into_owned()).filter(|n|!matches!(n.as_str(),"library.sqlite"|"library.sqlite-wal"|"library.sqlite-shm"|"core.lock"|"device.json"|"device.lock"|"logs")).collect();assert!(unexpected.is_empty(),"Unexpected credential/state file: {unexpected:?}");
 let report=json!({"separate_process":true,"reconnect_same_instance":true,"wrong_executable_refused":true,"second_writer_refused":true,"active_stop_refused":true,"scan_completed":1200,"restart_keeps_catalog":true,"credential_file_written":false});fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

//! Real-process API acceptance using generated files only; never prints sessions.
use galroon_core::local_core::{self, Session};
use serde_json::{json, Value};
use std::{fs, path::{Path,PathBuf}, time::Duration};
async fn api(s:&Session,path:&str,body:Option<Value>)->Value {
 let c=reqwest::Client::new();let url=format!("{}/api{path}",s.url);let q=if let Some(v)=body {c.post(url).json(&v)}else{c.get(url)};
 let r=q.bearer_auth(&s.token).send().await.unwrap();let status=r.status();let value:Value=r.json().await.unwrap();assert!(status.is_success(),"{path}: {status} {value}");value
}
async fn job(s:&Session,id:&str)->Value {api(s,"/jobs",None).await.as_array().unwrap().iter().find(|j|j["id"]==id).unwrap().clone()}
async fn wait(s:&Session,id:&str,state:&str)->Value {
 let deadline=tokio::time::Instant::now()+Duration::from_secs(120);
 loop {let j=job(s,id).await;if j["state"]==state{return j;}assert!(j["state"]!="failed"&&tokio::time::Instant::now()<deadline,"Waiting for {state}: {j}");tokio::time::sleep(Duration::from_millis(20)).await;}
}
async fn stopped(state:&Path,exe:&Path) {
 local_core::request(state,exe,"stop").await.unwrap();let deadline=tokio::time::Instant::now()+Duration::from_secs(10);
 loop {let f=fs::OpenOptions::new().read(true).write(true).open(state.join("core.lock")).unwrap();if fs2::FileExt::try_lock_exclusive(&f).is_ok(){break;}assert!(tokio::time::Instant::now()<deadline);tokio::time::sleep(Duration::from_millis(25)).await;}
}
fn saved(state:&Path,id:&str)->Vec<(String,i64)> {
 let c=rusqlite::Connection::open_with_flags(state.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();c.busy_timeout(Duration::from_secs(5)).unwrap();
 let mut s=c.prepare("SELECT file_id,attempts FROM hash_entries WHERE job_id=?1 AND state='completed'").unwrap();s.query_map([id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh fixture path"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");
 let content=vec![37u8;4*1024*1024];let mut sources=Vec::new();for part in 0..2 {let source=output.join(format!("source-{part}"));fs::create_dir(&source).unwrap();for i in 0..48 {fs::write(source.join(format!("copy-{i:02}.bin")),&content).unwrap();}sources.push(source);}
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let mut roots=Vec::new();
 for source in &sources {let r=api(&first,"/roots",Some(json!({"path":source,"label":source.file_name().unwrap().to_string_lossy()}))).await;roots.push(r["id"].clone());let j=api(&first,"/scans",Some(json!({"root_id":r["id"],"scope":"","exclude":[]}))).await;assert_eq!(wait(&first,j["id"].as_str().unwrap(),"completed").await["errors"],0);}
 let check=api(&first,"/duplicates",Some(json!({"root_ids":roots}))).await;let id=check["job_id"].as_str().unwrap();
 let deadline=tokio::time::Instant::now()+Duration::from_secs(20);loop {let j=job(&first,id).await;assert_ne!(j["state"],"completed","Fixture completed before pause");if j["processed"].as_i64().unwrap()>0 {break;}assert!(tokio::time::Instant::now()<deadline);tokio::time::sleep(Duration::from_millis(10)).await;}
 api(&first,&format!("/jobs/{id}/pause"),Some(json!({}))).await;let paused=wait(&first,id,"paused").await;let completed=saved(&state,id);assert!(!completed.is_empty()&&completed.len()<96);
 stopped(&state,&exe).await;let second=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.instance_id,second.instance_id);assert_eq!(job(&second,id).await["state"],"paused");
 api(&second,&format!("/jobs/{id}/cancel"),Some(json!({}))).await;assert_eq!(job(&second,id).await["state"],"cancelled");api(&second,&format!("/jobs/{id}/resume"),Some(json!({}))).await;
 let done=wait(&second,id,"completed").await;assert_eq!(done["processed"],96);assert_eq!(done["errors"],0);let result=api(&second,&format!("/duplicate-checks/{id}"),None).await;assert_eq!(result["verified"],96);assert_eq!(result["groups"].as_array().unwrap().len(),1);assert_eq!(result["groups"][0]["files"].as_array().unwrap().len(),96);
 let after=saved(&state,id);for old in &completed {assert!(after.contains(old),"Completed file was rehashed");}
 stopped(&state,&exe).await;for source in &sources {for i in 0..48 {assert_eq!(fs::read(source.join(format!("copy-{i:02}.bin"))).unwrap(),content);}}
 let report=json!({"schema":4,"sources":2,"files":96,"bytes":content.len()*96,"paused_processed":paused["processed"],"restarted_different_core":true,"cancel_then_resume":true,"completed_hashes_reused":completed.len(),"verified":result["verified"],"groups":1,"original_bytes_unchanged":true});fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

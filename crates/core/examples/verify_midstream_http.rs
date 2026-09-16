//! Real Windows byte-range write failure and restart/retry through installed Core HTTP.
//! Generated fixture only. The queued job is prepared through the Core library;
//! both executions and retry are performed by the separately launched executable.
use galroon_core::{acquire, db, local_core::{self,Session}, plans,scan};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},time::Duration,os::windows::io::AsRawHandle};
use windows_sys::Win32::Storage::FileSystem::{LockFile,UnlockFile};

async fn api(s:&Session,path:&str,body:Option<Value>)->Value {
 let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap();
 let url=format!("{}/api{path}",s.url);
 let req=if let Some(body)=body {client.post(url).json(&body)}else{client.get(url)};
 let response=req.bearer_auth(&s.token).send().await.unwrap();let status=response.status();
 let value:Value=response.json().await.unwrap();assert!(status.is_success(),"{path}: {status}: {value}");value
}
async fn terminal(s:&Session,job:&str)->Value {
 let end=tokio::time::Instant::now()+Duration::from_secs(45);
 loop {
  let value=api(s,&format!("/jobs/{job}"),None).await;
  if ["failed","completed","cancelled","paused"].contains(&value["state"].as_str().unwrap_or("")){return value;}
  assert!(tokio::time::Instant::now()<end,"Job timed out: {value}");
  tokio::time::sleep(Duration::from_millis(100)).await;
 }
}
fn save(out:&Path,name:&str,value:&Value){fs::write(out.join(name),serde_json::to_vec_pretty(value).unwrap()).unwrap();}

#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);
 let exe=PathBuf::from(&args[2]).canonicalize().unwrap();assert!(!out.exists(),"Use a new fixture directory");
 fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
 let state=out.join("state");let source=out.join("source");let game=source.join("Generated Game");let destination=out.join("destination");
 fs::create_dir_all(&game).unwrap();fs::create_dir(&destination).unwrap();
 let chunk=1024*1024usize;let data:Vec<u8>=(0..3*chunk+173).map(|n|((n*31+n/chunk)%251)as u8).collect();
 let original=game.join("生成payload.bin");fs::write(&original,&data).unwrap();let original_hash=plans::hash(&original).unwrap();
 let d=db::open(&state.join("library.sqlite")).unwrap();
 d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('generated',?1,'Generated midstream fixture')",[source.to_str().unwrap()]).unwrap();
 let sj=scan::create(&d,scan::ScanSpec{root_id:"generated".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),sj);
 let resource:String=d.lock().unwrap().query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();
 let job=acquire::create(&d,&resource,destination.to_str().unwrap(),false).unwrap();
 let raw:String=d.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();
 let spec:acquire::Spec=serde_json::from_str(&raw).unwrap();assert_eq!(spec.files.len(),1);
 d.lock().unwrap().execute("UPDATE jobs SET state='paused' WHERE id=?1",[&job]).unwrap();drop(d);
 let stage=destination.join(format!(".galroon-{job}"));fs::create_dir(&stage).unwrap();
 let partial=stage.join(format!("{}.part",spec.files[0].id));
 let lock=fs::OpenOptions::new().create_new(true).read(true).write(true).open(&partial).unwrap();
 assert_ne!(unsafe{LockFile(lock.as_raw_handle(),chunk as u32,0,chunk as u32,0)},0,"{}",std::io::Error::last_os_error());
 let cleanup_state=state.clone();let cleanup_exe=exe.clone();let output=out.clone();
 let result=tokio::spawn(async move{
  let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();
  assert_ne!(first.pid,std::process::id());
  api(&first,&format!("/jobs/{job}/resume"),Some(json!({}))).await;
  let failed=terminal(&first,&job).await;save(&output,"failed-job.json",&failed);
  assert_eq!(failed["state"],"failed");assert_eq!(failed["errors"],1);assert_eq!(failed["bytes"],chunk);
  assert_eq!(fs::metadata(&partial).unwrap().len(),chunk as u64);
  assert!(!Path::new(&spec.final_folder).exists());assert!(!stage.join("publication.json").exists());
  assert_ne!(unsafe{UnlockFile(lock.as_raw_handle(),chunk as u32,0,chunk as u32,0)},0);drop(lock);
  assert_eq!(fs::read(&partial).unwrap(),data[..chunk]);assert_eq!(plans::hash(&original).unwrap(),original_hash);
  local_core::request(&state,&exe,"stop").await.unwrap();
  let second=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.instance_id,second.instance_id);
  let reopened=api(&second,&format!("/jobs/{job}"),None).await;assert_eq!(reopened["state"],"failed");
  assert_eq!(fs::metadata(&partial).unwrap().len(),chunk as u64);
  api(&second,&format!("/jobs/{job}/resume"),Some(json!({}))).await;
  let completed=terminal(&second,&job).await;assert_eq!(completed["state"],"completed");save(&output,"completed-job.json",&completed);
  let published=Path::new(&spec.final_folder).join(&spec.files[0].relative);
  assert_eq!(fs::read(&published).unwrap(),data);assert_eq!(plans::hash(&published).unwrap(),original_hash);
  assert_eq!(plans::hash(&original).unwrap(),original_hash);assert!(!partial.exists());
  let timeline=api(&second,"/timeline",None).await;save(&output,"timeline.json",&timeline);
  let events=timeline["items"].as_array().unwrap();
  assert!(events.iter().any(|e|e["code"]=="file.transfer.state"&&e["details"]["to"]=="failed"),"{timeline}");
  save(&output,"hashes.json",&json!({"original":original,"published":published,"sha256":original_hash.1,"bytes":original_hash.0}));
  json!({"schema":db::SCHEMA_VERSION,"core_sha256":plans::hash(&exe).unwrap().1,"generated_bytes":data.len(),"failed_after_bytes":chunk,"partial_prefix_exact":true,"no_publication_on_failure":true,"failed_state_and_prefix_survive_core_restart":true,"http_resume_after_unlock_completed":true,"original_and_published_hash_equal":true,"failure_audit_retained":true,"initial_job_preparation":"Core library scan/create with paused fixture job","execution":"Separate installed Core via HTTP resume; Windows LockFile on second MiB","native_ui":false,"disk_full":false})
 }).await;
 let stopped=local_core::request(&cleanup_state,&cleanup_exe,"stop").await;
 save(&out,"cleanup.json",&json!({"identity_stop_acknowledged":stopped.is_ok(),"unrelated_native_editor_core_left_running":true}));
 let report=result.expect("Acceptance failed; scoped Core stop attempted");assert!(stopped.is_ok());save(&out,"report.json",&report);println!("{report}");
}

//! Installed-Core HTTP execution of generated queued-job path and journal faults.
use galroon_core::{acquire,db,local_core::{self,Session},plans,scan};
use serde_json::{json,Value};
use std::{collections::BTreeMap,fs,path::{Path,PathBuf},time::Duration};

fn save(out:&Path,name:&str,v:&Value){fs::write(out.join(name),serde_json::to_vec_pretty(v).unwrap()).unwrap();}
fn receipt(root:&Path)->BTreeMap<String,Value>{
 fn walk(root:&Path,dir:&Path,result:&mut BTreeMap<String,Value>){
  for e in fs::read_dir(dir).unwrap(){let p=e.unwrap().path();let m=fs::symlink_metadata(&p).unwrap();assert!(!scan::link(&m),"Unexpected link in receipt");
   if m.is_dir(){walk(root,&p,result)}else{assert!(m.is_file());let h=plans::hash(&p).unwrap();result.insert(p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),json!({"bytes":h.0,"sha256":h.1}));}
  }
 }
 let mut result=BTreeMap::new();walk(root,root,&mut result);result
}
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{
 let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap();let url=format!("{}/api{path}",s.url);
 let q=if let Some(v)=body{c.post(url).json(&v)}else{c.get(url)};let r=q.bearer_auth(&s.token).send().await.unwrap();let status=r.status();let v:Value=r.json().await.unwrap();assert!(status.is_success(),"{path}: {status}: {v}");v
}
async fn terminal(s:&Session,id:&str)->Value{
 let deadline=tokio::time::Instant::now()+Duration::from_secs(45);
 loop{let v=api(s,&format!("/jobs/{id}"),None).await;if ["failed","completed","paused","cancelled"].contains(&v["state"].as_str().unwrap_or("")){return v}assert!(tokio::time::Instant::now()<deadline,"{v}");tokio::time::sleep(Duration::from_millis(100)).await;}
}
fn assert_inside(out:&Path,p:&Path){assert!(p.is_absolute()&&p.starts_with(out)&&p!=out,"Fixture boundary violation: {}",p.display());}
struct Replacement{link:PathBuf,retained:PathBuf,restored:bool}
impl Replacement{
 fn restore(&mut self){
  assert!(scan::link(&fs::symlink_metadata(&self.link).unwrap()));
  fs::remove_dir(&self.link).unwrap(); // Nonrecursive unlink of the verified NTFS junction only.
  fs::rename(&self.retained,&self.link).unwrap();self.restored=true;
 }
}
impl Drop for Replacement{fn drop(&mut self){if !self.restored&&fs::symlink_metadata(&self.link).is_ok_and(|m|scan::link(&m)){let _=fs::remove_dir(&self.link);let _=fs::rename(&self.retained,&self.link);}}}

async fn case(out:PathBuf,exe:PathBuf,kind:&'static str)->Value{
 fs::create_dir(&out).unwrap();let state=out.join("state");let source=out.join("source");let game=source.join("Generated Game");let destination=out.join("destination");let external=out.join("outside-sentinel");
 for p in [&game,&destination,&external]{fs::create_dir_all(p).unwrap();}
 fs::create_dir(game.join("extras")).unwrap();fs::write(game.join("base.bin"),b"Generated source bytes only").unwrap();fs::write(game.join("extras/説明.txt"),b"Generated companion data\n").unwrap();fs::write(external.join("sentinel.txt"),b"Do not modify this generated sentinel").unwrap();
 let before=receipt(&game);let outside_before=receipt(&external);
 let d=db::open(&state.join("library.sqlite")).unwrap();d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('source',?1,'Generated fault fixture')",[source.to_str().unwrap()]).unwrap();
 let scan_job=scan::create(&d,scan::ScanSpec{root_id:"source".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),scan_job);
 let rid:String=d.lock().unwrap().query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();let job=acquire::create(&d,&rid,destination.to_str().unwrap(),false).unwrap();
 let raw:String=d.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();let spec:acquire::Spec=serde_json::from_str(&raw).unwrap();assert_eq!(spec.files.len(),2);
 d.lock().unwrap().execute("UPDATE jobs SET state='paused' WHERE id=?1",[&job]).unwrap();drop(d);
 let stage=destination.join(format!(".galroon-{job}"));let journal=stage.join("publication.json");let mut replacement=None;
 if kind=="journal-directory"{assert_inside(&out,&journal);fs::create_dir_all(&journal).unwrap();}else{
  let link=if kind=="source-junction"{game.clone()}else{assert_eq!(kind,"destination-junction");destination.clone()};let retained=out.join("retained-original");
  assert_inside(&out,&link);assert_inside(&out,&retained);assert_inside(&out,&external);assert!(!retained.exists());
  let boundary=out.canonicalize().unwrap();assert!(link.canonicalize().unwrap().starts_with(&boundary));assert!(retained.parent().unwrap().canonicalize().unwrap().starts_with(&boundary));assert!(external.canonicalize().unwrap().starts_with(&boundary));fs::rename(&link,&retained).unwrap();
  let mut command=std::process::Command::new("powershell.exe");
  {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
  let r=command.args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GALROON_FAULT_LINK -Target $env:GALROON_FAULT_TARGET | Out-Null"])
   .env("GALROON_FAULT_LINK",&link).env("GALROON_FAULT_TARGET",&external).output().unwrap();
  if !r.status.success(){fs::rename(&retained,&link).unwrap();panic!("Junction setup failed: {}",String::from_utf8_lossy(&r.stderr));}
  assert!(scan::link(&fs::symlink_metadata(&link).unwrap()));replacement=Some(Replacement{link,retained,restored:false});
 }
 let cleanup_state=state.clone();let cleanup_exe=exe.clone();let output=out.clone();
 let result=tokio::spawn(async move{
  let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();api(&first,&format!("/jobs/{job}/resume"),Some(json!({}))).await;
  let failed=terminal(&first,&job).await;save(&output,"failed-job.json",&failed);assert_eq!(failed["state"],"failed");assert_eq!(failed["errors"],1);
  assert!(!Path::new(&spec.final_folder).exists());assert_eq!(receipt(&external),outside_before);
  if kind=="journal-directory"{assert_eq!(receipt(&stage.join("files")),before);}else{assert!(failed["message"].as_str().unwrap().contains("Linked paths"),"{failed}");}
  local_core::request(&state,&exe,"stop").await.unwrap();
  if let Some(ref mut r)=replacement{r.restore();}else{assert_inside(&output,&journal);assert!(fs::read_dir(&journal).unwrap().next().is_none());fs::remove_dir(&journal).unwrap();}
  assert_eq!(receipt(&game),before);assert_eq!(receipt(&external),outside_before);
  let second=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.instance_id,second.instance_id);
  assert_eq!(api(&second,&format!("/jobs/{job}"),None).await["state"],"failed");api(&second,&format!("/jobs/{job}/resume"),Some(json!({}))).await;
  let completed=terminal(&second,&job).await;save(&output,"completed-job.json",&completed);assert_eq!(completed["state"],"completed");
  let published=receipt(Path::new(&spec.final_folder));assert_eq!(published,before);assert_eq!(receipt(&game),before);assert_eq!(receipt(&external),outside_before);
  let timeline=api(&second,"/timeline",None).await;assert!(timeline["items"].as_array().unwrap().iter().any(|e|e["code"]=="file.transfer.state"&&e["details"]["to"]=="failed"));save(&output,"timeline.json",&timeline);
  save(&output,"hashes.json",&json!({"source":game,"published":spec.final_folder,"source_files":before,"outside":external,"outside_files":outside_before}));
  json!({"case":kind,"failed_without_publication":true,"source_and_sentinel_unchanged":true,"failed_state_survives_restart":true,"http_retry_after_fault_removed":"completed","published_matches_original":true,"failure_history_retained":true,"staged_files_preserved":kind=="journal-directory"})
 }).await;
 let stopped=local_core::request(&cleanup_state,&cleanup_exe,"stop").await;save(&out,"cleanup.json",&json!({"identity_stop_acknowledged":stopped.is_ok()}));
 let report=result.expect("Case failed; scoped Core stop attempted");assert!(stopped.is_ok());save(&out,"report.json",&report);report
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);assert!(!out.exists());fs::create_dir_all(&out).unwrap();
 let canonical=out.canonicalize().unwrap();let raw=canonical.to_string_lossy();let out=PathBuf::from(raw.strip_prefix("\\\\?\\").unwrap_or(&raw));let exe=PathBuf::from(&args[2]).canonicalize().unwrap();
 let mut cases=vec![];for kind in ["source-junction","destination-junction","journal-directory"]{cases.push(case(out.join(kind),exe.clone(),kind).await);}
 let report=json!({"core_sha256":plans::hash(&exe).unwrap().1,"schema":db::SCHEMA_VERSION,"cases":cases,"initial_job_preparation":"Core library scan/create; paused generated job","execution":"Separate installed Core via HTTP resume and real filesystem faults","native_ui":false,"disk_full":false,"unrelated_native_editor_left_running":true});save(&out,"report.json",&report);println!("{report}");
}

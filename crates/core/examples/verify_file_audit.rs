//! Installed HTTP file audit, generated files only. Credentials remain internal.
use galroon_core::{access,backup,db,scan,plans,local_core::{self,Session}};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},time::Duration};
async fn request(s:&Session,path:&str,body:Option<Value>)->(u16,Value){
 let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();let url=format!("{}/api{path}",s.url);
 let req=if let Some(b)=body{c.post(url).json(&b)}else{c.get(url)};let r=req.bearer_auth(&s.token).send().await.unwrap();let status=r.status().as_u16();(status,r.json().await.unwrap())
}
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{let(status,v)=request(s,path,body).await;assert!((200..300).contains(&status),"{path}: {status}: {v}");v}
async fn run_plan(s:&Session,p:&Value){let id=p["id"].as_str().unwrap();api(s,&format!("/plans/{id}/approve"),Some(json!({"digest":p["digest"]}))).await;assert_eq!(api(s,&format!("/plans/{id}/execute"),Some(json!({}))).await["state"],"completed");}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();assert!(out.to_string_lossy().contains("file-audit"));assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
 let state=out.join("state");let source=out.join("source");let payload=source.join("Game/日本語-payload.bin");let keep=source.join("Keep/retained.bin");for p in [&payload,&keep]{fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,b"Generated file-audit bytes").unwrap();}let original=plans::hash(&payload).unwrap();
 let managed=out.join("managed");let quarantine=out.join("quarantine");let downloads=out.join("downloads");let backups=out.join("backups");for p in [&managed,&quarantine,&downloads,&backups]{fs::create_dir(p).unwrap();}
 let(rid,fid,retained)={let d=db::open(&state.join("library.sqlite")).unwrap();access::setup(&d,"Generated-File-Audit-002!",false).unwrap();d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated source')",[source.to_str().unwrap()]).unwrap();let job=scan::create(&d,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),job);
  let c=d.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','Generated work','Generated work')",[]).unwrap();let rid:String=c.query_row("SELECT id FROM resources WHERE relative_path='Game'",[],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='w' WHERE id=?1",[&rid]).unwrap();c.execute("INSERT OR IGNORE INTO resource_bindings(resource_id,work_id,role) VALUES(?1,'w','main')",[&rid]).unwrap();
  let fid:String=c.query_row("SELECT file_id FROM resource_files WHERE resource_id=?1",[&rid],|r|r.get(0)).unwrap();let retained:String=c.query_row("SELECT id FROM files WHERE id!=?1",[&fid],|r|r.get(0)).unwrap();(rid,fid,retained)};
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();let output=out.clone();
 let run=tokio::spawn(async move{
  let isolated=api(&first,"/quarantine",Some(json!({"file_id":fid,"retained_id":retained,"destination":quarantine}))).await;run_plan(&first,&isolated).await;assert!(!payload.exists());assert_eq!(plans::hash(&keep).unwrap(),original);
  let restored_plan=api(&first,&format!("/quarantine/{}/restore",isolated["items"][0]["id"].as_str().unwrap()),Some(json!({}))).await;run_plan(&first,&restored_plan).await;assert_eq!(plans::hash(&payload).unwrap(),original);
  let organize=api(&first,"/plans/organize",Some(json!({"resource_id":rid,"destination":managed}))).await;run_plan(&first,&organize).await;assert!(!payload.exists());
  let undo=api(&first,&format!("/plans/{}/undo",organize["id"].as_str().unwrap()),Some(json!({}))).await;run_plan(&first,&undo).await;assert_eq!(plans::hash(&payload).unwrap(),original);
  let preview=api(&first,&format!("/resources/{rid}/acquire-preview"),None).await;
  let copy=api(&first,"/acquire",Some(json!({"resource_id":rid,"destination":downloads,"extract":false,"manifest_digest":preview["manifest_digest"],"file_ids":preview["files"].as_array().unwrap().iter().map(|f|f["id"].clone()).collect::<Vec<_>>()}))).await;let job_path=format!("/jobs/{}",copy["id"].as_str().unwrap());
  let deadline=tokio::time::Instant::now()+Duration::from_secs(30);let job=loop{let job=api(&first,&job_path,None).await;if job["state"]=="completed"{break job;}assert_ne!(job["state"],"failed","{job}");assert!(tokio::time::Instant::now()<deadline);tokio::time::sleep(Duration::from_millis(100)).await;};assert!(job.get("spec").is_none());
  let timeline=api(&first,"/timeline",None).await;assert!(timeline["next"].is_null());let rows=timeline["items"].as_array().unwrap();
  for code in ["file.plan.created","file.plan.state","file.operation.recorded","file.operation.state","file.transfer.created","file.transfer.state"]{assert!(rows.iter().any(|e|e["code"]==code),"{code}");}
  for kind in ["quarantine","restore","organize","undo_organize"]{assert!(rows.iter().any(|e|e["details"]["kind"]==kind&&e["details"]["to"]=="completed"),"{kind}");}
  let detail=api(&first,&format!("/plans/{}",undo["id"].as_str().unwrap()),None).await;assert_eq!(detail["state"],"completed");
  let client=reqwest::Client::new();let login=client.post(format!("{}/api/access/login",first.url)).header(reqwest::header::ORIGIN,&first.url).json(&json!({"password":"Generated-File-Audit-002!","name":"Generated file audit Web check"})).send().await.unwrap().error_for_status().unwrap();let cookie=login.headers()[reqwest::header::SET_COOKIE].to_str().unwrap().split(';').next().unwrap().to_owned();
  let denied=client.get(format!("{}/api/timeline",first.url)).header(reqwest::header::COOKIE,&cookie).send().await.unwrap();assert_eq!(denied.status(),403);
  let web_job:Value=client.get(format!("{}/api{job_path}",first.url)).header(reqwest::header::COOKIE,&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(web_job,job);
  let anonymous=client.get(format!("{}/api{job_path}",first.url)).send().await.unwrap();assert_eq!(anonymous.status(),401);
  let publication:Vec<_>=fs::read_dir(&downloads).unwrap().map(|e|e.unwrap().path()).filter(|p|!p.file_name().unwrap().to_string_lossy().starts_with('.')).collect();assert_eq!(publication.len(),1);assert_eq!(plans::hash(&publication[0].join(payload.file_name().unwrap())).unwrap(),original);
  for(name,v)in [("timeline.json",&timeline),("task.json",&job),("plan.json",&detail)]{fs::write(output.join(name),serde_json::to_vec_pretty(v).unwrap()).unwrap();}
  let saved=api(&first,"/backups",Some(json!({"destination":backups}))).await;local_core::request(&rs,&re,"stop").await.unwrap();let restored=backup::restore_new(Path::new(saved["path"].as_str().unwrap()),&output.join("restored"),None).unwrap();let second=local_core::connect_or_start(restored.clone(),re.clone()).await.unwrap();assert_eq!(api(&second,"/timeline",None).await,timeline);assert_eq!(api(&second,&job_path,None).await,job);assert_eq!(plans::hash(&payload).unwrap(),original);assert_eq!(plans::hash(&keep).unwrap(),original);local_core::request(&restored,&re,"stop").await.unwrap();
  json!({"schema":db::SCHEMA_VERSION,"separate_installed_core_http":true,"generated_files":2,"events":rows.len(),"plan_kinds":["quarantine","restore","organize","undo_organize"],"copy_and_source_hashes":true,"event_and_job_equal_after_backup_restore":true,"web_timeline_status":403,"web_task_projection_equal":true,"anonymous_task_status":401,"source_hash":original,"native_ui":false})
 }).await;
 let _=local_core::request(&state,&exe,"stop").await;let _=local_core::request(&out.join("restored"),&exe,"stop").await;
 let mut report=run.expect("Acceptance failed; fixture Core stop attempted");report["core_sha256"]=json!(plans::hash(&exe).unwrap().1);fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

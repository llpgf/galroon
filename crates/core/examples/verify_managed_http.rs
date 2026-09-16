//! Generated files only: separate Core organization, smart facts, undo and restore.
use galroon_core::{db,scan,plans,backup,local_core::{self,Session}};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},time::Duration};
async fn request(s:&Session,path:&str,body:Option<Value>)->(u16,Value){
    let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();
    let url=format!("{}/api{path}",s.url);let req=if let Some(body)=body{c.post(url).json(&body)}else{c.get(url)};
    let r=req.bearer_auth(&s.token).send().await.unwrap();let status=r.status().as_u16();let body=r.json().await.unwrap();(status,body)
}
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{let(status,v)=request(s,path,body).await;assert!((200..300).contains(&status),"{path}: {status}: {v}");v}
fn definition(state:&str)->Value{json!({"schema_version":1,"scope":"collection","sort":"title","descending":false,"rule":{"kind":"field","predicate":{"field":"organizing_state","mode":"any","values":[state]}}})}
async fn expect(s:&Session,state:&str,total:u64){let p=api(s,"/smart-lists/preview",Some(definition(state))).await;assert_eq!(p["total"],total,"{state}: {p}");assert_eq!(p["unknown_count"],0);}
async fn approve(s:&Session,p:&Value){api(s,&format!("/plans/{}/approve",p["id"].as_str().unwrap()),Some(json!({"digest":p["digest"]}))).await;}
async fn execute(s:&Session,p:&Value){assert_eq!(api(s,&format!("/plans/{}/execute",p["id"].as_str().unwrap()),Some(json!({}))).await["state"],"completed");}
#[tokio::main]async fn main(){
    let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
    let state=out.join("state");let source=out.join("source");let payload=source.join("Game/日本語-payload.bin");fs::create_dir_all(payload.parent().unwrap()).unwrap();fs::write(&payload,b"Generated-only native organization acceptance").unwrap();let original=plans::hash(&payload).unwrap();
    let managed=out.join("managed");let alternate=out.join("alternate");let downloads=out.join("downloads");let backups=out.join("backups");for p in [&managed,&alternate,&downloads,&backups]{fs::create_dir(p).unwrap();}
    let rid={let d=db::open(&state.join("library.sqlite")).unwrap();d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated source')",[source.to_str().unwrap()]).unwrap();let job=scan::create(&d,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),job);
        let c=d.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','Generated work','Generated work')",[]).unwrap();let rid:String=c.query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='w' WHERE id=?1",[&rid]).unwrap();c.execute("INSERT OR IGNORE INTO resource_bindings(resource_id,work_id,role) VALUES(?1,'w','main')",[&rid]).unwrap();rid};
    let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.pid,std::process::id());let rs=state.clone();let re=exe.clone();let output=out.clone();
    let result=tokio::spawn(async move{
        expect(&first,"unorganized",1).await;expect(&first,"organized",0).await;
        let a=api(&first,"/plans/organize",Some(json!({"resource_id":rid,"destination":managed}))).await;
        let b=api(&first,"/plans/organize",Some(json!({"resource_id":rid,"destination":alternate}))).await;
        assert_eq!(plans::hash(&payload).unwrap(),original);approve(&first,&a).await;
        let rejected=request(&first,&format!("/plans/{}/approve",b["id"].as_str().unwrap()),Some(json!({"digest":b["digest"]}))).await;assert_eq!(rejected.0,400);assert!(rejected.1.to_string().contains("one managed folder"));
        assert_eq!(api(&first,&format!("/plans/{}",b["id"].as_str().unwrap()),None).await["state"],"ready");expect(&first,"pending",1).await;
        api(&first,"/smart-lists/organization",Some(json!({"revision":0,"request_id":"create-organization","edit":{"action":"save","name":"Move pending","description":"Generated fixture","pinned":false,"definition":definition("pending")}}))).await;
        let computed=api(&first,"/smart-lists/organization/compute",Some(json!({"revision":1,"request_id":"pending-before-move"}))).await;let snapshot=computed["snapshot"].as_str().unwrap().to_owned();
        execute(&first,&a).await;expect(&first,"organized",1).await;expect(&first,"pending",0).await;
        let target=Path::new(a["items"][0]["target"].as_str().unwrap());assert_eq!(plans::hash(target).unwrap(),original);assert!(!payload.exists());
        assert_eq!(api(&first,&format!("/smart-lists/organization/results?snapshot={snapshot}"),None).await["total"],1);
        local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs.clone(),re.clone()).await.unwrap();assert_eq!(second.library_id,first.library_id);expect(&second,"organized",1).await;
        let reverse=api(&second,&format!("/plans/{}/undo",a["id"].as_str().unwrap()),Some(json!({}))).await;approve(&second,&reverse).await;expect(&second,"pending",1).await;execute(&second,&reverse).await;expect(&second,"unorganized",1).await;assert_eq!(plans::hash(&payload).unwrap(),original);
        let saved=api(&second,"/backups",Some(json!({"destination":backups}))).await;local_core::request(&rs,&re,"stop").await.unwrap();
        let restored=backup::restore_new(Path::new(saved["path"].as_str().unwrap()),&output.join("restored"),None).unwrap();let third=local_core::connect_or_start(restored.clone(),re.clone()).await.unwrap();expect(&third,"unorganized",1).await;
        let rejected=request(&third,"/plans/organize",Some(json!({"resource_id":rid,"destination":alternate}))).await;assert_eq!(rejected.0,400);assert!(rejected.1.to_string().contains("one managed folder"));
        assert_eq!(api(&third,&format!("/smart-lists/organization/results?snapshot={snapshot}"),None).await["total"],1);
        let preview=api(&third,&format!("/resources/{rid}/acquire-preview"),None).await;
        let copy=api(&third,"/acquire",Some(json!({"resource_id":rid,"destination":downloads,"extract":false,"manifest_digest":preview["manifest_digest"],"file_ids":preview["files"].as_array().unwrap().iter().map(|f|f["id"].clone()).collect::<Vec<_>>()}))).await;
        let deadline=tokio::time::Instant::now()+Duration::from_secs(30);loop{let jobs=api(&third,"/jobs",None).await;let job=jobs.as_array().unwrap().iter().find(|j|j["id"]==copy["id"]).unwrap();if job["state"]=="completed"{break;}assert_ne!(job["state"],"failed","{job}");assert!(tokio::time::Instant::now()<deadline);tokio::time::sleep(Duration::from_millis(100)).await;}
        let published:Vec<_>=fs::read_dir(&downloads).unwrap().map(|e|e.unwrap().path()).filter(|p|!p.file_name().unwrap().to_string_lossy().starts_with('.')).collect();assert_eq!(published.len(),1);assert_eq!(plans::hash(&published[0].join(payload.file_name().unwrap())).unwrap(),original);assert_eq!(plans::hash(&payload).unwrap(),original);expect(&third,"unorganized",1).await;
        local_core::request(&restored,&re,"stop").await.unwrap();
        json!({"schema":db::SCHEMA_VERSION,"separate_core_http":true,"generated_source_files":1,"binding":"manual fixture, not matching accuracy evidence","managed_root_conflict_rollback":true,"organization_fact_transitions":["unorganized","pending","organized","pending","unorganized"],"pinned_snapshot_after_move_restart_restore":true,"move_and_reviewed_undo_hashes":true,"backup_via_http_restore_via_local_library":true,"restored_core_root_constraint":true,"acquisition_to_independent_destination":true,"download_does_not_organize_source":true,"source_hash":original,"installed_ui":false})
    }).await;
    let _=local_core::request(&state,&exe,"stop").await;let _=local_core::request(&out.join("restored"),&exe,"stop").await;
    let mut report=result.expect("Acceptance failed; stop attempted");report["core_sha256"]=json!(plans::hash(&exe).unwrap().1);fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

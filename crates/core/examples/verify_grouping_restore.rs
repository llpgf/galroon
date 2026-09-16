//! Installed HTTP merge/split private-data preservation; generated metadata only.
use galroon_core::{backup,db,local_core::{self,Session},plans};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,time::Duration};
async fn request(s:&Session,path:&str,body:Option<Value>)->(u16,Value) {
    let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();let url=format!("{}/api{path}",s.url);
    let req=if let Some(body)=body {c.post(url).json(&body)} else {c.get(url)};
    let r=req.bearer_auth(&s.token).send().await.unwrap();let status=r.status().as_u16();let text=r.text().await.unwrap();
    (status,serde_json::from_str(&text).unwrap_or(json!({"text":text})))
}
async fn api(s:&Session,path:&str,body:Option<Value>)->Value {let (status,v)=request(s,path,body).await;assert!((200..300).contains(&status),"{path}: {status}: {v}");v}
async fn le(s:&Session,revision:i64,action:Value)->Value {api(s,"/lists/l",Some(json!({"revision":revision,"request_id":format!("list-{revision}"),"edit":action}))).await}
fn choices(preview:&Value,follow:&str)->Value {
    let choices=|key:&str|{let ids:std::collections::BTreeSet<_>=preview[key].as_array().unwrap().iter().map(|v|v["id"].as_str().unwrap()).collect();ids.into_iter().map(|id|json!({"id":id,"follow":follow})).collect::<Vec<_>>()};
    json!({"digest":preview["digest"],"tags":choices("tags"),"entries":choices("entries")})
}
#[tokio::main] async fn main() {
    let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();let mut cases=vec![];
    for follow in ["original","new","both"] {
        let folder=out.join(follow);fs::create_dir(&folder).unwrap();let state=folder.join("state");let restored=folder.join("restored");let generated=folder.join("generated-empty-source");fs::create_dir(&generated).unwrap();
        {let d=db::open(&state.join("library.sqlite")).unwrap();let c=d.lock().unwrap();
            c.execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Generated empty source')",[generated.to_str().unwrap()]).unwrap();
            c.execute_batch("INSERT INTO works(id,title,original_title,notes) VALUES('a','Generated A','Generated A','Private work A'),('b','Generated B','Generated B','Private work B'); INSERT INTO releases(id,label) VALUES('ra','Generated edition A'),('rb','Generated edition B'); INSERT INTO work_releases VALUES('a','ra'),('b','rb'); INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('x','r','empty','Generated resource','archive','a'); INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES('x','a','ra','main');").unwrap();
            c.execute("INSERT INTO source_watch(root_id,path,enabled,status) SELECT id,path,0,'disabled' FROM roots",[]).unwrap();
        }
        let s=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let ss=state.clone();let rr=restored.clone();let ee=exe.clone();let ff=folder.clone();
        let outcome=tokio::spawn(async move {
            let tag=api(&s,"/custom-tags",Some(json!({"name":"Private grouping","work_ids":["a"]}))).await;let tid=tag["id"].clone();
            le(&s,0,json!({"action":"create","name":"Private variants"})).await;
            le(&s,1,json!({"action":"add","members":[{"work_key":"local:a","title":"unused"},{"work_key":"local:b","title":"unused"}]})).await;
            let list=api(&s,"/lists/l",None).await;let a=list["entries"][0]["id"].clone();let b=list["entries"][1]["id"].clone();
            le(&s,2,json!({"action":"annotate","entry_id":a,"notes":"Private A 日本語","preferred_release_id":"ra"})).await;
            le(&s,3,json!({"action":"annotate","entry_id":b,"notes":"Private B 中文","preferred_release_id":"rb"})).await;
            api(&s,"/works/a/merge",Some(json!({"revision":1,"target_id":"b","target_revision":1}))).await;
            let merged=api(&s,"/lists/l",None).await;assert_eq!(merged["total"],1);assert_eq!(merged["entries"][0]["work_key"],"local:b");assert_eq!(merged["entries"][0]["notes"],"Private A 日本語");
            let variants=merged["entries"][0]["merge_variants"].as_array().unwrap();assert_eq!(variants.len(),2);assert_eq!(variants[0]["notes"],"Private A 日本語");assert_eq!(variants[1]["notes"],"Private B 中文");assert_eq!(variants[0]["preferred_release_id"],"ra");assert_eq!(variants[1]["preferred_release_id"],"rb");
            let saved=api(&s,"/backups",Some(json!({"destination":ff}))).await;let bp=PathBuf::from(saved["path"].as_str().unwrap());let manifest=backup::inspect(&bp).unwrap();
            local_core::request(&ss,&ee,"stop").await.unwrap();backup::restore_new(&bp,&rr,Some(&manifest.sha256)).unwrap();
            let restored_session=local_core::connect_or_start(rr.clone(),ee.clone()).await.unwrap();assert_eq!(api(&restored_session,"/lists/l",None).await,merged);
            let baseline_works=api(&restored_session,"/works",None).await;let work_revision=baseline_works.as_array().unwrap().iter().find(|w|w["id"]=="b").unwrap()["revision"].as_i64().unwrap();
            let old=api(&restored_session,"/works/b/split-preview",None).await;assert_eq!(old["entries"].as_array().unwrap().len(),1);assert_eq!(old["tags"].as_array().unwrap().len(),1);
            let entry=merged["entries"][0]["id"].clone();
            le(&restored_session,5,json!({"action":"annotate","entry_id":entry,"notes":"Reviewed split 日本語","preferred_release_id":"ra"})).await;
            let split_body=|p:&Value|json!({"revision":work_revision,"resources":["x"],"title":"Generated split","private":choices(p,follow)});
            let rejected=request(&restored_session,"/works/b/split",Some(split_body(&old))).await;assert_eq!(rejected.0,400);assert!(rejected.1.to_string().contains("Private classifications changed"),"{rejected:?}");
            let works=api(&restored_session,"/works",None).await;assert_eq!(works,baseline_works);
            let current=api(&restored_session,"/works/b/split-preview",None).await;let split=api(&restored_session,"/works/b/split",Some(split_body(&current))).await;let target=split["id"].as_str().unwrap();
            let after=api(&restored_session,"/lists/l",None).await;let entries=after["entries"].as_array().unwrap();assert_eq!(entries.len(),if follow=="both"{2}else{1});
            for entry in entries {assert_eq!(entry["notes"],"Reviewed split 日本語");assert_eq!(entry["preferred_release_id"],"ra");assert_eq!(entry["merge_variants"],merged["entries"][0]["merge_variants"]);}
            let expected_keys=match follow {"original"=>vec!["local:b".to_owned()],"new"=>vec![format!("local:{target}")],_=>vec!["local:b".to_owned(),format!("local:{target}")]};assert_eq!(entries.iter().map(|v|v["work_key"].as_str().unwrap().to_owned()).collect::<Vec<_>>(),expected_keys);
            let tags=api(&restored_session,"/custom-tags",None).await;let members=tags.as_array().unwrap().iter().find(|v|v["id"]==tid).unwrap()["work_ids"].as_array().unwrap().clone();assert_eq!(members.len(),if follow=="both"{2}else{1});assert_eq!(members.contains(&json!("b")),follow!="new");assert_eq!(members.contains(&json!(target)),follow!="original");
            local_core::request(&rr,&ee,"stop").await.unwrap();let restarted=local_core::connect_or_start(rr.clone(),ee.clone()).await.unwrap();assert_eq!(api(&restarted,"/lists/l",None).await,after);local_core::request(&rr,&ee,"stop").await.unwrap();
            {let c=rusqlite::Connection::open_with_flags(rr.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT work_id FROM resources WHERE id='x'",[],|r|r.get::<_,String>(0)).unwrap(),target);for (id,note) in [("a","Private work A"),("b","Private work B")] {assert_eq!(c.query_row("SELECT notes FROM works WHERE id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),note);}assert_eq!(c.query_row("SELECT count(*) FROM files",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM operations",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
            fs::write(ff.join("merged-list.json"),serde_json::to_vec_pretty(&merged).unwrap()).unwrap();fs::write(ff.join("split-list.json"),serde_json::to_vec_pretty(&after).unwrap()).unwrap();
            json!({"follow":follow,"merge_two_notes_and_preferences_retained":true,"exact_list_after_restore":true,"stale_split_preview_rejected_without_new_work":true,"reviewed_inheritance":true,"restart_exact":true,"files":0,"file_operations":0,"backup_sha256":manifest.sha256})
        }).await;
        let _=local_core::request(&state,&exe,"stop").await;let _=local_core::request(&restored,&exe,"stop").await;cases.push(outcome.expect("Grouping failed; stop attempted"));
    }
    let report=json!({"schema":db::SCHEMA_VERSION,"core_sha256":plans::hash(&exe).unwrap().1,"separate_installed_core_http":true,"cases":cases,"native_ui":false,"full_mvp":false});fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

use galroon_core::local_core::{self,Session};
use serde_json::{json,Value};
use std::{path::PathBuf,time::Duration,fs};
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{
 let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();
 let url=format!("{}/api{path}",s.url);let request=if let Some(v)=body{client.post(url).json(&v)}else{client.get(url)};
 let response=request.bearer_auth(&s.token).send().await.unwrap();assert!(response.status().is_success(),"API {path}: {}",response.status());response.json().await.unwrap()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh fixture output required"));
 let exe=PathBuf::from(args.get(2).expect("Core executable required")).canonicalize().unwrap();
 let count:usize=args.get(3).map(|v|v.parse().expect("Work count")).unwrap_or(55);assert!((55..=100000).contains(&count));
 let populated=args.get(4).is_some_and(|s|s=="populated");let organizing=args.get(5).is_some_and(|s|s=="organizing");
 assert!(!output.exists(),"Use a fresh fixture");fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();
 if populated{tx.execute("INSERT INTO roots(id,path,label) VALUES('fixture-root',?1,'Generated offline root')",[output.join("absent-source").to_string_lossy().as_ref()]).unwrap();}
 for n in 0..count{let id=format!("w{n:03}");tx.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?1,?1)",[&id]).unwrap();
 if populated{
 let vn=format!("v{}",n+1);tx.execute("UPDATE works SET vndb_id=?2 WHERE id=?1",rusqlite::params![id,vn]).unwrap();
 let cache=json!({"fetched_at":galroon_core::db::now(),"more":false,"vn":{"released":"2020-01","developers":[{"id":"p1","name":"Fixture Studio"}],"staff":[],"va":[],"tags":[{"id":"g1","name":"Fixture tag","spoiler":0,"lie":false}]},"characters":[]});
 tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![format!("exploration.v4.work.{vn}.1"),cache.to_string()]).unwrap();
 tx.execute("INSERT INTO releases(id,label,languages,platforms) VALUES(?1,'Fixture edition','[\"ja\"]','[\"Windows\"]')",[&id]).unwrap();
 tx.execute("INSERT INTO work_releases(work_id,release_id) VALUES(?1,?1)",[&id]).unwrap();
 tx.execute("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES(?1,'fixture-root',?1,?1,'archive')",[&id]).unwrap();
 tx.execute("INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES(?1,?1,?1,'main')",[&id]).unwrap();
 tx.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES(?1,'fixture-root',?1,?1,1,'1','zip','fixture','missing')",[&id]).unwrap();
 tx.execute("INSERT INTO resource_files(resource_id,file_id) VALUES(?1,?1)",[&id]).unwrap();
 }}
 tx.commit().unwrap();}

 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.pid,std::process::id());
 let task_state=state.clone();let task_exe=exe.clone();
 let outcome=tokio::spawn(async move{
  let status=json!({"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}});
  let rule=if populated{json!({"kind":"group","join":"all","children":[status,{"kind":"field","predicate":{"field":"source_tags","mode":"any","values":["g1"]}},{"kind":"edition","join":"all","conditions":[{"field":"confirmed_languages","mode":"any","values":["ja"]},{"field":"availability","mode":"any","values":["missing"]}]}]})}else{status};
  let rule=if organizing{json!({"kind":"group","join":"all","children":[rule,{"kind":"field","predicate":{"field":"organizing_state","mode":"any","values":["unorganized"]}}]})}else{rule};
  let created=api(&first,"/smart-lists/fixture",Some(json!({"revision":0,"request_id":"create","edit":{"action":"save","name":"Backlog fixture","description":"","pinned":false,"definition":{"schema_version":1,"scope":"collection","sort":"title","descending":false,"rule":rule}}}))).await;assert_eq!(created["revision"],1);
  let compute_started=std::time::Instant::now();
  let (computed,concurrent_read_ms)=tokio::join!(
   api(&first,"/smart-lists/fixture/compute",Some(json!({"revision":1,"request_id":"compute"}))),
   async{tokio::time::sleep(Duration::from_millis(200)).await;let start=std::time::Instant::now();api(&first,"/smart-lists/fixture/results",None).await;start.elapsed().as_millis()}
  );
  assert!(concurrent_read_ms<=300,"Result read during manual compute exceeded300ms");
  let snapshot=computed["snapshot"].as_str().unwrap().to_string();
  let compute_ms=compute_started.elapsed().as_millis();
  let page=api(&first,&format!("/smart-lists/fixture/results?snapshot={snapshot}"),None).await;assert_eq!(page["total"],count);assert_eq!(page["entries"].as_array().unwrap().len(),50);assert_eq!(page["next_after"],49);
  let mut cursor=None;let mut keys=std::collections::BTreeSet::new();let mut page_ms=Vec::new();
  loop{let path=format!("/smart-lists/fixture/results?snapshot={snapshot}{}",cursor.map(|v|format!("&after={v}")).unwrap_or_default());
   let start=std::time::Instant::now();let page=api(&first,&path,None).await;page_ms.push(start.elapsed().as_millis());
   for entry in page["entries"].as_array().unwrap(){assert!(keys.insert(entry["work_key"].as_str().unwrap().to_owned()),"Duplicate across pages");}
   cursor=page["next_after"].as_i64();if cursor.is_none(){break;}
  }
  assert_eq!(keys.len(),count);page_ms.sort();let page_p95_ms=page_ms[(page_ms.len()*95).div_ceil(100)-1];
  let work=api(&first,"/works",None).await.as_array().unwrap().iter().find(|w|w["id"]=="w054").unwrap().clone();
  api(&first,"/works/w054",Some(json!({"revision":work["revision"],"status":"playing"}))).await;
  let update_started=std::time::Instant::now();
  let until=tokio::time::Instant::now()+Duration::from_secs(30);
  loop{let current=api(&first,"/smart-lists/fixture/results",None).await;if current["total"]==count-1{break;}assert!(tokio::time::Instant::now()<until,"Automatic recompute timed out");tokio::time::sleep(Duration::from_millis(200)).await;}
  let automatic_update_ms=update_started.elapsed().as_millis();
  let tail=api(&first,&format!("/smart-lists/fixture/results?snapshot={snapshot}&after=49"),None).await;assert_eq!(tail["total"],count);assert_eq!(tail["entries"].as_array().unwrap().len(),(count-50).min(50));assert_eq!(tail["newer_available"],true);
  let body=json!({"snapshot":snapshot,"expected_total":count,"name":"Frozen backlog","request_id":"conversion"});
  let conversion_started=std::time::Instant::now();
  let converted=api(&first,"/smart-lists/fixture/to-manual",Some(body.clone())).await;assert_eq!(converted["count"],count);
  let conversion_ms=conversion_started.elapsed().as_millis();
  let manual=converted["id"].as_str().unwrap().to_string();
  let library=first.library_id.clone();let instance=first.instance_id.clone();
  local_core::request(&task_state,&task_exe,"stop").await.unwrap();tokio::time::sleep(Duration::from_millis(300)).await;
  let restarted=local_core::connect_or_start(task_state,task_exe).await.unwrap();assert_ne!(restarted.instance_id,instance);assert_eq!(restarted.library_id,library);
  let saved=api(&restarted,&format!("/lists/{manual}"),None).await;assert_eq!(saved["total"],count);
  assert_eq!(api(&restarted,"/smart-lists/fixture/to-manual",Some(body)).await,converted);
  let old=api(&restarted,&format!("/smart-lists/fixture/results?snapshot={snapshot}&after=49"),None).await;assert_eq!(old["entries"].as_array().unwrap().len(),(count-50).min(50));
  assert_eq!(api(&restarted,"/smart-lists/fixture/results",None).await["total"],count-1);
  json!({"separate_process":true,"http_definition_and_compute":true,"automatic_recompute":true,"pinned_snapshot_after_change_and_restart":true,"full_conversion_count":count,"idempotent_conversion_after_restart":true,"library_identity_retained":true,"schema":galroon_core::db::SCHEMA_VERSION,"generated_works":count,"compute_ms":compute_ms,"concurrent_manual_read_ms":concurrent_read_ms,"automatic_update_ms":automatic_update_ms,"conversion_ms":conversion_ms,"page_p95_ms":page_p95_ms,"pages_measured":page_ms.len(),"page_target_met":page_p95_ms<=300,"source_files":0,"populated_metadata_and_resource_records":populated,"organizing_state_in_rule":organizing})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;
 let report=outcome.expect("Verification failed; Core stop attempted");stopped.expect("Final Core stop failed");
 fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

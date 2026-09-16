//! Full generated collection traversal against a separate Windows Core executable.
use galroon_core::{db,local_core::{self,Session},plans};
use serde_json::{json,Value};
use std::{collections::BTreeSet,fs,path::{Path,PathBuf},time::{Duration,Instant}};
use std::os::windows::process::CommandExt;
type R<T>=Result<T,String>;
async fn request(client:&reqwest::Client,s:&Session,path:&str,body:Option<Value>)->R<(Value,u128,usize)>{
    let start=Instant::now();let url=format!("{}/api{path}",s.url);let req=if let Some(body)=body{client.post(url).json(&body)}else{client.get(url)};
    let response=req.bearer_auth(&s.token).send().await.map_err(|e|e.to_string())?;let status=response.status();let bytes=response.bytes().await.map_err(|e|e.to_string())?;
    let value:Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;if !status.is_success(){return Err(format!("HTTP {status}: {value}"));}Ok((value,start.elapsed().as_millis(),bytes.len()))
}
fn memory(pid:u32)->Value{
    let result=std::process::Command::new("powershell.exe").creation_flags(0x08000000).args(["-NoProfile","-NonInteractive","-Command",&format!("$taskProcess=Get-Process -Id {pid}; [ordered]@{{working_set_bytes=$taskProcess.WorkingSet64;peak_working_set_bytes=$taskProcess.PeakWorkingSet64;private_bytes=$taskProcess.PrivateMemorySize64}} | ConvertTo-Json -Compress")]).output().unwrap();
    assert!(result.status.success());serde_json::from_slice(&result.stdout).unwrap()
}
fn log_count(state:&Path,code:&str)->usize{fs::read_to_string(state.join("logs/core.jsonl")).unwrap_or_default().lines().filter(|line|line.contains(&format!("\"code\":\"{code}\""))).count()}
async fn stop(state:&Path,exe:&Path)->R<()>{
    local_core::request(state,exe,"stop").await?;
    let lock=fs::OpenOptions::new().read(true).write(true).open(state.join("core.lock")).map_err(|e|e.to_string())?;let until=Instant::now()+Duration::from_secs(15);
    loop{if fs2::FileExt::try_lock_exclusive(&lock).is_ok(){return Ok(());}if Instant::now()>until{return Err("Core lock did not release after stop".into());}tokio::time::sleep(Duration::from_millis(30)).await;}
}
async fn run(client:&reqwest::Client,s:&Session,state:&Path,count:usize)->R<Value>{
    // Startup observations legitimately invalidate cursors. Measure a stable fixture only after observing this write.
    let observed_at=Instant::now();let read=rusqlite::Connection::open_with_flags(state.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
    loop{let raw:String=read.query_row("SELECT root_digest FROM smart_catalog_state",[],|r|r.get(0)).map_err(|e|e.to_string())?;if serde_json::from_str::<Value>(&raw).ok().is_some_and(|v|v==json!({"generated":true})){break;}if observed_at.elapsed()>Duration::from_secs(10){return Err("Initial source observation did not complete".into());}tokio::time::sleep(Duration::from_millis(30)).await;}
    drop(read);let root_observation_ms=observed_at.elapsed().as_millis();
    let baseline=memory(s.pid);let (first,cold_ms,first_bytes)=request(client,s,"/collection",None).await?;
    if first["total"]!=count||first["filtered"]!=count{return Err("Incomplete global count".into());}
    let mut page=first.clone();let mut times=Vec::new();let mut ids=BTreeSet::new();let mut pages=0;let mut max_bytes=first_bytes;
    loop{
        let items=page["items"].as_array().ok_or("Missing work page")?;if items.len()>60{return Err("Unbounded page".into());}
        for item in items{let id=item["id"].as_str().ok_or("Missing work ID")?;if !ids.insert(id.to_owned()){return Err("Duplicate work across pages".into());}}
        pages+=1;let Some(next)=page["next"].as_str() else{break};
        let query=reqwest::Url::parse_with_params(&format!("{}/api/collection",s.url),&[("before",next)]).unwrap();
        let (value,elapsed,bytes)=request(client,s,&format!("/collection?{}",query.query().unwrap()),None).await?;times.push(elapsed);max_bytes=max_bytes.max(bytes);page=value;
    }
    if ids.len()!=count||pages!=(count+59)/60{return Err("Traversal lost works".into());}
    times.sort();let p95=times.get(times.len()*95/100).copied().unwrap_or(0);if p95>300{return Err(format!("Warm page p95 exceeded 300 ms: {p95}"));}
    let query=json!({"sort":"title","locale":"en"});let params=reqwest::Url::parse_with_params(&format!("{}/api/collection",s.url),&[("query",query.to_string())]).unwrap();let path=format!("/collection?{}",params.query().unwrap());
    let starts=log_count(state,"collection.view.started");let finishes=log_count(state,"collection.view.completed");
    let background_client=client.clone();let background_session=s.clone();let computing=tokio::spawn(async move{request(&background_client,&background_session,&path,None).await});
    let until=Instant::now()+Duration::from_secs(10);while log_count(state,"collection.view.started")==starts{if Instant::now()>until{return Err("Collection evaluation did not start".into());}tokio::time::sleep(Duration::from_millis(5)).await;}
    let began_active=log_count(state,"collection.view.completed")==finishes;
    let (_,concurrent_ms,_)=request(client,s,"/dashboard",None).await?;
    let overlapped=began_active&&log_count(state,"collection.view.completed")==finishes;
    let (sorted,sort_ms,_)=computing.await.map_err(|e|e.to_string())??;
    if count>=10000&&(!overlapped||concurrent_ms>300){return Err(format!("Concurrent read not responsive during evaluation: overlap={overlapped}, ms={concurrent_ms}"));}
    if sorted["items"][0]["id"]!="collection-000000"||sorted["items"][2]["id"]!="collection-000002"{return Err("Natural title sort changed".into());}
    let selection=json!({"query":{},"view_token":first["view_token"],"ids":["collection-000000",format!("collection-{:06}",count-1)]});
    let (selected,selection_ms,_)=request(client,s,"/collection/selection",Some(selection)).await?;if selected["members"].as_array().map(Vec::len)!=Some(2){return Err("Selection lost a work outside current page".into());}
    if count>10000{let response=client.post(format!("{}/api/collection/selection",s.url)).bearer_auth(&s.token).json(&json!({"query":{},"view_token":first["view_token"],"ids":null})).send().await.map_err(|e|e.to_string())?;if response.status()!=reqwest::StatusCode::BAD_REQUEST{return Err("Oversized selection was not rejected".into());}}
    Ok(json!({"works":count,"physical_source_files":0,"populated_resource_file_edition_rows":count,"pages":pages,"unique_works":ids.len(),"startup_root_observation_ms":root_observation_ms,"cold_query_after_root_observation_ms":cold_ms,"warm_page_p95_ms":p95,"warm_page_max_ms":times.last(),"max_page_bytes":max_bytes,"natural_sort_ms":sort_ms,"concurrent_dashboard_ms":concurrent_ms,"concurrent_read_observed_during_evaluation":overlapped,"cross_page_selection_ms":selection_ms,"baseline_memory":baseline,"after_queries_memory":memory(s.pid),"native_ui":false}))
}
#[tokio::main]async fn main(){
    let args=std::env::args().collect::<Vec<_>>();let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();let count:usize=args.get(3).map(|x|x.parse().unwrap()).unwrap_or(100000);assert!((61..=100000).contains(&count));assert!(!out.exists());fs::create_dir_all(out.join("state")).unwrap();let out=out.canonicalize().unwrap();let state=out.join("state");let root=out.join("generated-source");fs::create_dir(&root).unwrap();
    {let db=db::open(&state.join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();
        tx.execute("INSERT INTO roots(id,path,label) VALUES('generated',?1,'Generated metadata source')",[root.to_string_lossy().to_string()]).unwrap();tx.execute_batch("INSERT INTO releases(id,label,languages) VALUES('shared','Generated shared edition','[\"ja\",\"en\"]'); INSERT INTO custom_tags(id,name,name_key) VALUES('personal','Generated personal tag','generated personal tag');").unwrap();
        for i in 0..count{let id=format!("collection-{i:06}");let resource=format!("resource-{i}");let file=format!("file-{i}");
            tx.execute("INSERT INTO works(id,title,original_title,description,developer,released,tags,aliases,status,favorite) VALUES(?1,?2,?3,?4,'Generated Studio',?5,?6,?7,?8,?9)",rusqlite::params![id,format!("Generated volume {i}"),format!("原題 {i}"),"Generated description. ".repeat(100),format!("{}-04-28",2000+i%20),if i%2==0{"[\"Drama\"]"}else{"[\"Comedy\"]"},json!([format!("Alias {i}")]).to_string(),if i%2==0{"backlog"}else{"completed"},i%3==0]).unwrap();
            tx.execute("INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES(?1,'generated',?2,?2,'archive',?3)",rusqlite::params![resource,format!("generated-{i}.zip"),id]).unwrap();
            tx.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES(?1,'generated',?2,?3,1,'1','zip','generated-fixture','present')",rusqlite::params![file,root.join(format!("generated-{i}.zip")).to_string_lossy().to_string(),format!("generated-{i}.zip")]).unwrap();
            tx.execute("INSERT INTO resource_files VALUES(?1,?2)",rusqlite::params![resource,file]).unwrap();tx.execute("INSERT INTO work_releases VALUES(?1,'shared')",[&id]).unwrap();tx.execute("INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES(?1,?2,'shared','main')",rusqlite::params![resource,id]).unwrap();if i%5==0{tx.execute("INSERT INTO custom_tag_works VALUES('personal',?1)",[&id]).unwrap();}
        }tx.commit().unwrap();
    }
    let s=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();println!("Generated {count} works; separate Core pid {}",s.pid);
    let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap();let result=run(&client,&s,&state,count).await;
    let stopped=stop(&state,&exe).await;fs::write(out.join("stop.json"),json!({"acknowledged_and_lock_released":stopped.is_ok()}).to_string()).unwrap();
    let mut report=match result{Ok(v)=>v,Err(error)=>json!({"passed":false,"error":error})};report["core_sha256"]=json!(plans::hash(&exe).unwrap().1);report["schema"]=json!(db::SCHEMA_VERSION);report["core_stop_acknowledged"]=json!(stopped.is_ok());
    if report.get("error").is_none(){report["passed"]=json!(stopped.is_ok());}
    fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");assert_eq!(report["passed"],true);
}

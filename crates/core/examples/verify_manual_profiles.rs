use galroon_core::{local_core::{self,Session},relation_corrections::{Correction,Key,Link},relation_correction_store::{self,Edit}};
use serde_json::{json,Value};use std::{fs,path::PathBuf,time::Duration};
async fn get(s:&Session,path:&str)->Value{reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap().get(format!("{}/api{path}",s.url)).bearer_auth(&s.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap()}
fn links(spoiler:u8)->Vec<Correction>{vec![Key::Staff{id:"s1".into(),aid:Some(8),role:"scenario".into(),note:"".into()},Key::Character{id:"c1".into()},Key::Company{id:"p1".into()}].into_iter().enumerate().map(|(i,key)|Correction{id:format!("add-{i}"),replaces:None,link:Some(Link{key,name:if spoiler==0{"Generated local credit".into()}else{"Generated secret credit".into()},character_name:None,spoiler})}).collect()}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 let raw=json!({"id":"v1","title":"Generated provider work","staff":[{"id":"s1","aid":1,"name":"First alias","role":"scenario"},{"id":"s1","aid":2,"name":"Second alias","role":"scenario"}],"va":[],"developers":[{"id":"p1","name":"Studio"}]});
 let mut snapshots=vec![];let edit=Edit{revision:0,request_id:"rebind".into(),corrections:vec![Correction{id:"alias".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:Some(1),role:"scenario".into(),note:"".into()}),link:Some(Link{key:Key::Staff{id:"s1".into(),aid:Some(3),role:"scenario".into(),note:"".into()},name:"Rebound alias".into(),character_name:None,spoiler:0})}]};
 {
  let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();{
   let mut c=db.lock().unwrap();relation_correction_store::change(&mut c,"v1",&edit).ok().unwrap();
   for(work,spoiler)in [("v2",0),("v3",2)]{relation_correction_store::change(&mut c,work,&Edit{revision:0,request_id:format!("add-{work}"),corrections:links(spoiler)}).ok().unwrap();}
   for(kind,id)in [("person","s1"),("character","c1"),("company","p1")]{let page=json!({(kind):{"id":id,"name":"Generated profile","vns":[{"id":"v1","spoiler":0}]},"works":[raw],"more":false,"page":1,"fetched_at":galroon_core::db::now()});snapshots.push((format!("exploration.v3.{kind}.{id}.1"),page));}
   for(id,title)in [("v1","Generated provider work"),("v2","Generated manual-only work"),("v3","Generated secret work")]{snapshots.push((format!("exploration.v4.work.{id}.1"),json!({"vn":if id=="v1"{raw.clone()}else{json!({"id":id,"title":title,"staff":[],"va":[],"developers":[],"relations":[]})},"characters":[],"more":false,"fetched_at":galroon_core::db::now()})));}
   for(key,value)in &snapshots{c.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![key,value.to_string()]).unwrap();}
  }
  galroon_core::access::setup(&db,"Generated-profile-validation-864!",false).unwrap();
  let backup=galroon_core::backup::export(&db,&output).unwrap();let restored=galroon_core::backup::restore_new(&backup,&output.join("restored"),None).unwrap();let db2=galroon_core::db::open(&restored.join("library.sqlite")).unwrap();let mut c=db2.lock().unwrap();
  assert_eq!(galroon_core::relation_correction_index::current_revision(&c,None).unwrap(),3);assert_eq!(relation_correction_store::read(&c,"v1").ok().unwrap().1,edit.corrections);
  assert_eq!(relation_correction_store::change(&mut c,"v1",&edit).ok().unwrap()["revision"],1);assert_eq!(galroon_core::relation_correction_index::current_revision(&c,None).unwrap(),3);
  assert_eq!(relation_correction_store::history(&c,"v1",None).ok().unwrap()["items"].as_array().unwrap().len(),1);
  let missing=galroon_core::manual_profile::page(&c,"company","p1",None,None,false).unwrap();assert_eq!(missing["partial"],true);assert_eq!(missing["missing_work_ids"],json!(["v2"]));assert_eq!(missing["works"],json!([]));
  // Backup intentionally drops provider cache. Rehydrating public metadata restores presentation.
  c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v2.1',?1)",[json!({"vn":{"id":"v2","title":"Generated manual-only work"}}).to_string()]).unwrap();assert_eq!(galroon_core::manual_profile::page(&c,"company","p1",None,None,false).unwrap()["works"][0]["id"],"v2");
  relation_correction_store::change(&mut c,"v2",&Edit{revision:1,request_id:"restored-undo".into(),corrections:vec![]}).ok().unwrap();assert_eq!(galroon_core::manual_profile::page(&c,"company","p1",None,None,false).unwrap()["works"],json!([]));
  assert_eq!(relation_correction_store::read(&db.lock().unwrap(),"v2").ok().unwrap().1.len(),3);
 }
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();
 let result=tokio::spawn(async move{
  for(kind,id,endpoint)in [("person","s1","people"),("character","c1","characters"),("company","p1","companies")]{
   let page=get(&first,&format!("/{endpoint}/{id}?revision=3")).await;assert_eq!(page["relationship_revision"],3);assert_eq!(page["works"][0]["profile_membership"],true);
   if kind=="person"{assert_eq!(page["works"][0]["staff"][0]["aid"],2);assert_eq!(page["works"][0]["staff"][1]["aid"],3);}
   for spoilers in [false,true]{let manual=get(&first,&format!("/entities/{id}/manual-works?spoilers={spoilers}&revision=3")).await;assert_eq!(manual["partial"],false);assert_eq!(manual.to_string().contains("Generated secret work"),spoilers);assert!(manual.to_string().contains("Generated manual-only work"));}
  }
  for spoilers in [false,true]{
   let rebound=get(&first,&format!("/vns/v1/exploration?spoilers={spoilers}")).await;assert_eq!(rebound["vn"]["staff"][0]["aid"],2);assert_eq!(rebound["vn"]["staff"][1]["aid"],3);
   let visible=get(&first,&format!("/vns/v2/exploration?spoilers={spoilers}")).await;assert_eq!(visible["vn"]["staff"][0]["aid"],8);assert_eq!(visible["vn"]["developers"][0]["id"],"p1");assert_eq!(visible["characters"][0]["id"],"c1");
   let secret=get(&first,&format!("/vns/v3/exploration?spoilers={spoilers}")).await;assert_eq!(secret.to_string().contains("Generated secret credit"),spoilers);if !spoilers{assert_eq!(secret["vn"]["staff"],json!([]));assert_eq!(secret["vn"]["developers"],json!([]));assert_eq!(secret["characters"],json!([]));}
  }
  let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(20)).build().unwrap();let response=client.post(format!("{}/api/access/login",first.url)).header("Origin",&first.url).json(&json!({"password":"Generated-profile-validation-864!","name":"generated-web"})).send().await.unwrap().error_for_status().unwrap();let cookie=response.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
  for(path,method,expected)in [("/entities/s1/manual-works","GET",200),("/entities/s1/manual-works","POST",403),("/people/s1","GET",200),("/vns/v3/exploration","GET",200)]{let req=if method=="POST"{client.post(format!("{}/api{path}",first.url))}else{client.get(format!("{}/api{path}",first.url))};let response=req.header("cookie",&cookie).send().await.unwrap();assert_eq!(response.status().as_u16(),expected);if expected==200{assert!(!response.text().await.unwrap().contains("Generated secret credit"));}}
  let response=client.post(format!("{}/api/vns/v1/relationship-decisions",first.url)).bearer_auth(&first.token).json(&json!({"revision":0,"request_id":"native-broad","hidden":{"people":["s1"]}})).send().await.unwrap();assert!(response.status().is_success());
  assert_eq!(client.get(format!("{}/api/people/s1?revision=3",first.url)).bearer_auth(&first.token).send().await.unwrap().status().as_u16(),400);
  assert_eq!(get(&first,"/people/s1?revision=4").await["works"],json!([]));
  local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs.clone(),re).await.unwrap();assert_eq!(first.library_id,second.library_id);assert_eq!(get(&second,"/people/s1?revision=4").await["works"],json!([]));assert_eq!(get(&second,"/entities/p1/manual-works?revision=4").await["works"][0]["id"],"v2");
  let c=rusqlite::Connection::open_with_flags(rs.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();for(key,value)in snapshots{let saved:String=c.query_row("SELECT value FROM settings WHERE key=?1",[key],|r|r.get(0)).unwrap();assert_eq!(serde_json::from_str::<Value>(&saved).unwrap(),value);}assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  json!({"separate_core":true,"profile_kinds":3,"staff_alias_rebind":true,"forward_alias_and_spoiler_edges":true,"spoiler_membership":true,"web_read_and_write_rejection":true,"broad_hide_revision_and_restart":true,"raw_caches_unchanged":6,"backup_restore_receipt_history_index_and_undo":true,"restore_without_provider_cache_is_explicitly_partial":true,"source_files":0,"collection_works":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let mut report=result.expect("Validation failed; stop attempted");stopped.unwrap();report["core_sha256"]=json!(galroon_core::plans::hash(&exe).unwrap().1);fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

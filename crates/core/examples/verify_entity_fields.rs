use galroon_core::local_core::{self,Session};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,time::Duration};
async fn api(s:&Session,path:&str,body:Option<Value>)->Value{
 let client=reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();let url=format!("{}/api{path}",s.url);
 let req=if let Some(body)=body{client.post(url).json(&body)}else{client.get(url)};
 let response=req.bearer_auth(&s.token).send().await.unwrap();assert!(response.status().is_success(),"{path}: {}",response.status());response.json().await.unwrap()
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();
 let raw=json!({"fetched_at":galroon_core::db::now(),"vn":{"id":"v1","title":"Fixture","developers":[{"id":"p1","name":"Source company"}],"staff":[{"id":"s1","aid":7,"name":"Actual credit","role":"scenario"}],"va":[],"relations":[]},"characters":[{"id":"c1","name":"Source character","vns":[{"id":"v1","spoiler":0}]}],"more":false});
 let mut caches=vec![("exploration.v4.work.v1.1".to_string(),raw.clone())];
 for(kind,id)in [("person","s1"),("character","c1"),("company","p1")]{caches.push((format!("exploration.v3.{kind}.{id}.1"),json!({"fetched_at":galroon_core::db::now(),(kind):{"id":id,"name":format!("Source {kind}"),"description":"Source introduction","vns":[{"id":"v1"}]},"works":[raw["vn"].clone()],"page":1,"more":false})));}
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Fixture','Fixture','v1')",[]).unwrap();for(key,value)in &caches{c.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![key,value.to_string()]).unwrap();}}
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();
 let result=tokio::spawn(async move{
 let entities=[("person","s1","people"),("character","c1","characters"),("company","p1","companies")];let mut receipts=vec![];
 for(kind,id,endpoint)in entities{
  let body=json!({"revision":0,"request_id":format!("edit-{id}"),"fields":{"name":format!("Personal {kind}"),"description":""}});
  let saved=api(&first,&format!("/entities/{id}/field-decisions"),Some(body.clone())).await;assert_eq!(saved["revision"],1);receipts.push((id,body,saved));
  let profile=api(&first,&format!("/{endpoint}/{id}"),None).await;assert_eq!(profile[kind]["name"],format!("Personal {kind}"));assert_eq!(profile[kind]["description"],"");assert!(profile[kind]["local_fields"].as_array().unwrap().contains(&json!("description")));
 }
 let projected=api(&first,"/works/w/exploration",None).await;assert_eq!(projected["vn"]["developers"][0]["name"],"Personal company");assert_eq!(projected["characters"][0]["name"],"Personal character");assert_eq!(projected["vn"]["staff"][0]["name"],"Actual credit");assert_eq!(projected["vn"]["staff"][0]["aid"],7);assert_eq!(projected["vn"]["staff"][0]["display_name"],"Personal person");
 local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs.clone(),re).await.unwrap();assert_eq!(first.library_id,second.library_id);
 for(id,body,saved)in receipts{assert_eq!(api(&second,&format!("/entities/{id}/field-decisions"),Some(body.clone())).await,saved);assert_eq!(api(&second,&format!("/entities/{id}/field-decisions"),None).await["fields"],body["fields"]);}
 for(kind,id,endpoint)in entities{
  let profile=api(&second,&format!("/{endpoint}/{id}"),None).await;assert_eq!(profile[kind]["name"],format!("Personal {kind}"));assert_eq!(profile[kind]["description"],"");
  api(&second,&format!("/entities/{id}/field-decisions"),Some(json!({"revision":1,"request_id":format!("reset-{id}"),"fields":{"name":null,"description":null}}))).await;
  let restored=api(&second,&format!("/{endpoint}/{id}"),None).await;assert_eq!(restored[kind]["name"],format!("Source {kind}"));assert_eq!(restored[kind]["description"],"Source introduction");assert!(restored[kind].get("local_fields").is_none());
  let history=api(&second,&format!("/entities/{id}/field-history"),None).await;assert_eq!(history["items"].as_array().unwrap().len(),2);assert_eq!(history["items"][0]["before"]["name"],format!("Personal {kind}"));
 }
 assert_eq!(api(&second,"/works/w/exploration",None).await,raw);
 let c=rusqlite::Connection::open_with_flags(rs.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();for(key,expected)in caches{let value:String=c.query_row("SELECT value FROM settings WHERE key=?1",[key],|r|r.get(0)).unwrap();assert_eq!(serde_json::from_str::<Value>(&value).unwrap(),expected);}
 json!({"separate_core":true,"entities":3,"restart_and_receipt_replay":true,"empty_introduction":true,"reset_source":true,"history_per_entity":2,"credit_alias_preserved":true,"reference_names_projected":true,"raw_caches_unchanged":4,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;
 let stopped=local_core::request(&state,&exe,"stop").await;let report=result.expect("Validation failed; stop attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

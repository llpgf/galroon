//! Windows acceptance (US-4.1): reads the installed desktop's Core through its control channel and
//! prints matching state and folder → VNDB identities. Never prints the bearer credential.
//! Usage: acceptance_probe <GALROON_DATA dir> <installed galroon-core.exe> [add <folder> | remap-preview <root id> <folder> | remap <root id> <folder> <digest> | summary]
//! `add` does only what the Sources form's Save does; scanning and matching must then happen by themselves.
use galroon_core::local_core::{self,Session};
use serde_json::{Value,json};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let state=std::path::Path::new(&args[1]);let exe=std::path::Path::new(&args[2]);
 let session:Session=serde_json::from_value(local_core::request(state,exe,"session").await.expect("Core is not running for this data folder")).unwrap();
 let client=reqwest::Client::new();
 let get=|path:&str|{let request=client.get(format!("{}/api{path}",session.url)).bearer_auth(&session.token);async move{request.send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap()}};
 if args.get(3).map(String::as_str)==Some("add"){let path=args.get(4).expect("Source folder required");let r:Value=client.post(format!("{}/api/roots",session.url)).bearer_auth(&session.token).json(&json!({"path":path,"label":"acceptance"})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();println!("{}",json!({"added":r}));return;}
 let post=|path:String,body:Value|{let request=client.post(format!("{}/api{path}",session.url)).bearer_auth(&session.token).json(&body);async move{let r=request.send().await.unwrap();let status=r.status();let v=r.json::<Value>().await.unwrap_or(Value::Null);json!({"status":status.as_u16(),"body":v})}};
 match args.get(3).map(String::as_str){
  Some("remap-preview")=>{println!("{}",post(format!("/roots/{}/remap/preview",args[4]),json!({"path":args[5]})).await);return;}
  Some("remap")=>{println!("{}",post(format!("/roots/{}/remap",args[4]),json!({"path":args[5],"digest":args[6]})).await);return;}
  _=>{}
 }
 let (matching,resources,works,roots)=(get("/matching").await,get("/resources").await,get("/works").await,get("/roots").await);
 let by_id:std::collections::HashMap<&str,&Value>=works.as_array().unwrap().iter().map(|w|(w["id"].as_str().unwrap(),w)).collect();
 let folders:Vec<Value>=resources.as_array().unwrap().iter().filter(|r|r["files"].as_i64().unwrap_or(0)>0).map(|r|{let w=r["work_id"].as_str().and_then(|id|by_id.get(id));json!({"path":r["path"],"vndb":w.and_then(|w|w["vndb_id"].as_str()),"cover":w.map(|w|!w["cover"].as_str().unwrap_or("").is_empty())})}).collect();
 if args.get(3).map(String::as_str)==Some("summary"){
  let all=resources.as_array().unwrap();let count=|f:&dyn Fn(&Value)->bool|all.iter().filter(|r|f(r)).count();
  let jobs=get("/jobs").await;
  println!("{}",json!({"pid":session.pid,"roots":roots,"job":matching["job"],"works":works.as_array().unwrap().len(),"resources":all.len(),"with_files":count(&|r|r["files"].as_i64().unwrap_or(0)>0),"matched":count(&|r|r["work_id"].is_string()),"missing_files":all.iter().map(|r|r["missing"].as_i64().unwrap_or(0)).sum::<i64>(),"unverified_files":all.iter().map(|r|r["unverified"].as_i64().unwrap_or(0)).sum::<i64>(),"files":all.iter().map(|r|r["files"].as_i64().unwrap_or(0)).sum::<i64>(),"jobs":jobs.as_array().map(|j|j.iter().take(6).map(|j|json!([j["kind"],j["state"],j["processed"],j["errors"],j["message"]])).collect::<Vec<_>>())}));return;}
 println!("{}",json!({"pid":session.pid,"roots":roots,"job":matching["job"],"works":works.as_array().unwrap().len(),"resources":folders}));
}

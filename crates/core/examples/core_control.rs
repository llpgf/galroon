//! Acceptance harness: never prints the control-channel bearer credential.
use galroon_core::local_core::{self,Session};
use serde_json::{Value,json};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let state=std::path::Path::new(&args[1]);let exe=std::path::Path::new(&args[2]);let action=args.get(3).map(String::as_str).unwrap_or("status");
 if action=="start"{assert!(state.join("library.sqlite").is_file(),"Existing fixture catalog required");let session=local_core::connect_or_start(state.to_path_buf(),exe.canonicalize().unwrap()).await.unwrap();println!("{}",json!({"url":session.url,"pid":session.pid,"library_id":session.library_id}));return;}
 if action=="stop"{println!("{}",local_core::request(state,exe,"stop").await.unwrap());return;}
 let session:Session=serde_json::from_value(local_core::request(state,exe,"session").await.unwrap()).unwrap();let client=reqwest::Client::new();
 if action=="scan"{let source=std::path::Path::new(args.get(4).expect("Source directory required")).canonicalize().unwrap();let root:Value=client.post(format!("{}/api/roots",session.url)).bearer_auth(&session.token).json(&json!({"path":source,"label":"Native lifecycle acceptance"})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();let job:Value=client.post(format!("{}/api/scans",session.url)).bearer_auth(&session.token).json(&json!({"root_id":root["id"],"scope":"","exclude":[]})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();println!("{}",json!({"pid":session.pid,"instance_id":session.instance_id,"job_id":job["id"]}));return;}
 let jobs:Value=client.get(format!("{}/api/jobs",session.url)).bearer_auth(&session.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();println!("{}",json!({"pid":session.pid,"instance_id":session.instance_id,"jobs":jobs}));
}

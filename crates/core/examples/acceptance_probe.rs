//! Windows acceptance (US-4.1): reads the installed desktop's Core through its control channel and
//! prints matching state and folder → VNDB identities. Never prints the bearer credential.
//! Usage: acceptance_probe <GALROON_DATA dir> <installed galroon-core.exe>
use galroon_core::local_core::{self,Session};
use serde_json::{Value,json};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let state=std::path::Path::new(&args[1]);let exe=std::path::Path::new(&args[2]);
 let session:Session=serde_json::from_value(local_core::request(state,exe,"session").await.expect("Core is not running for this data folder")).unwrap();
 let client=reqwest::Client::new();
 let get=|path:&str|{let request=client.get(format!("{}/api{path}",session.url)).bearer_auth(&session.token);async move{request.send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap()}};
 let (matching,resources,works,roots)=(get("/matching").await,get("/resources").await,get("/works").await,get("/roots").await);
 let by_id:std::collections::HashMap<&str,&Value>=works.as_array().unwrap().iter().map(|w|(w["id"].as_str().unwrap(),w)).collect();
 let folders:Vec<Value>=resources.as_array().unwrap().iter().filter(|r|r["files"].as_i64().unwrap_or(0)>0).map(|r|{let w=r["work_id"].as_str().and_then(|id|by_id.get(id));json!({"path":r["path"],"vndb":w.and_then(|w|w["vndb_id"].as_str()),"cover":w.map(|w|!w["cover"].as_str().unwrap_or("").is_empty())})}).collect();
 println!("{}",json!({"pid":session.pid,"roots":roots,"job":matching["job"],"works":works.as_array().unwrap().len(),"resources":folders}));
}

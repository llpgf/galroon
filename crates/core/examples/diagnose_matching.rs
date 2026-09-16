//! Read-only regression of recorded unmatched names; optional bounded live VNDB queries.
use serde_json::{Value,json};
#[tokio::main]async fn main(){
 let args:Vec<String>=std::env::args().collect();let rows:Vec<Value>=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();assert!(rows.len()<=12);
 let c=rusqlite::Connection::open_with_flags(&args[2],rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();let mut report=vec![];
 for row in rows{
  let mut hints=vec![row["relative_path"].as_str().unwrap().to_string()];hints.extend(row["members"].as_array().unwrap().iter().take(31).map(|v|v.as_str().unwrap().to_string()));
  let input=galroon_core::matching::prepare(row["title"].as_str().unwrap(),&hints).unwrap();
  let response=if args.get(4).is_some_and(|v|v=="live"){galroon_core::matching::search(input.clone()).await.unwrap()}else{let mut v=row["evidence"].clone();v["results"]=json!(galroon_core::matching::rank(&input,v["results"].as_array().unwrap().clone()));v["titles"]=json!(input.titles);v};
  let result=galroon_core::automatch::explain(&c,row["id"].as_str().unwrap(),&response).unwrap();
  println!("{}",json!({"title":row["title"],"titles":result["titles"],"vndb_id":result["results"][0]["id"],"match":result["results"][0]["match"],"review_reason":result["decision"]["reason"]}));
  report.push(json!({"id":row["id"],"title":row["title"],"result":result}));std::fs::write(&args[3],serde_json::to_vec_pretty(&report).unwrap()).unwrap();
 }
}

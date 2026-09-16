//! Generated catalog-only preview benchmark; no physical game files or apply.
use galroon_core::{db,membership};
use serde_json::json;
use std::{path::PathBuf,time::Instant};
fn main(){
 let args=std::env::args().collect::<Vec<_>>();let count:usize=args.get(3).map(|s|s.parse().unwrap()).unwrap_or(6539);assert!((1..=100000).contains(&count));let state=PathBuf::from(&args[1]);std::fs::create_dir_all(&state).unwrap();let db=db::open(&state.join("library.sqlite")).unwrap();
 {let c=db.lock().unwrap();c.execute_batch(&"INSERT OR IGNORE INTO roots(id,path,label) VALUES('r','generated-catalog-only','Scale fixture');INSERT OR IGNORE INTO resources(id,root_id,relative_path,title,kind) VALUES('source','r','Game','Generated scale source','directory');WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<6538) INSERT OR IGNORE INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) SELECT printf('f%05d',i),'r',printf('generated/Game/%05d.bin',i),printf('Game/%05d.bin',i),1,'generated','bin','generated' FROM n;INSERT OR IGNORE INTO resource_files(resource_id,file_id) SELECT 'source',id FROM files;".replace("i<6538",&format!("i<{}",count-1))).unwrap();}
 let selection=membership::Selection{revision:1,file_ids:(0..count).rev().map(|i|format!("f{i:05}")).collect(),target_id:None,target_revision:None,title:"Selected scale files".into()};
 let mut times=vec![];let mut preview=serde_json::Value::Null;
 for _ in 0..3 {drop(std::mem::take(&mut preview));let start=Instant::now();preview=membership::preview(&db,"source",&selection).unwrap();times.push(start.elapsed().as_millis());}
 assert_eq!(preview["selected"].as_array().unwrap().len(),count);assert_eq!(preview["source"]["files"].as_array().unwrap().len(),count);assert_eq!(preview["selected"][0]["id"],"f00000");
 let output=PathBuf::from(&args[2]);serde_json::to_writer_pretty(std::fs::File::create(&output).unwrap(),&preview).unwrap();println!("{}",json!({"members":count,"selected":count,"selection_json_bytes":serde_json::to_vec(&selection).unwrap().len(),"pretty_preview_bytes":std::fs::metadata(&output).unwrap().len(),"preview_ms":times,"digest":preview["digest"],"physical_files":0,"scope":"In-process release Core catalog-only preview; not HTTP/RSS/cold boot acceptance"}));
}

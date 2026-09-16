//! Generated readonly-Web fixture; prints URL only, never an owner bearer token.
use galroon_core::{access,db,local_core,plans};
use serde_json::json;
use std::{fs,path::PathBuf};
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();assert!(!out.exists());fs::create_dir_all(out.join("state")).unwrap();let out=out.canonicalize().unwrap();let state=out.join("state");
 let d=db::open(&state.join("library.sqlite")).unwrap();access::setup(&d,"Generated-Web-Acceptance-002!",false).unwrap();
 {let c=d.lock().unwrap();
  let title="Generated collection — 日本語と繁體中文 / A very long title for narrow-screen acceptance";
  c.execute("INSERT INTO works(id,title,original_title,vndb_id,description,notes) VALUES('w',?1,'生成作品・原題','v1','Generated introduction for full installed Web acceptance. No real work is represented.','Private owner work note')",[title]).unwrap();
  let raw=json!({"fetched_at":db::now(),"vn":{"id":"v1","title":title,"description":"Generated introduction","developers":[{"id":"p1","name":"Generated studio"}],"staff":[{"id":"s1","aid":7,"name":"Generated credit alias","role":"scenario"}],"va":[],"relations":[]},"characters":[{"id":"c1","name":"Generated character","description":"Generated character introduction","vns":[{"id":"v1","spoiler":0}]}],"more":false});
  c.execute("INSERT INTO settings VALUES('exploration.v4.work.v1.1',?1)",[raw.to_string()]).unwrap();
  for(kind,id)in [("person","s1"),("character","c1"),("company","p1")]{c.execute("INSERT INTO settings VALUES(?1,?2)",rusqlite::params![format!("exploration.v3.{kind}.{id}.1"),json!({"fetched_at":db::now(),(kind):{"id":id,"name":format!("Generated {kind}"),"description":"Generated profile introduction","vns":[{"id":"v1"}]},"works":[raw["vn"].clone()],"page":1,"more":false}).to_string()]).unwrap();}
  c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Private tag for generated collection','private tag for generated collection');INSERT INTO custom_tag_works VALUES('t','w');INSERT INTO custom_tag_entities VALUES('t','s1');INSERT INTO curated_lists(id,name,description) VALUES('l','Generated reading list — 日本語','Long description for a narrow-screen collection list');INSERT INTO list_entries(id,list_id,work_key,title,position,notes,added) VALUES('e','l','local:w','Saved generated title',0,'Private list entry note',1);").unwrap();
 }drop(d);
 let session=local_core::connect_or_start(state,exe.clone()).await.unwrap();let report=json!({"url":session.url,"pid":session.pid,"core_sha256":plans::hash(&exe).unwrap().1,"schema":db::SCHEMA_VERSION,"fixture_only":true,"source_files":0,"installed_web_root":exe.parent().unwrap().join("web"),"ui_acceptance_passed":false});fs::write(out.join("fixture.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

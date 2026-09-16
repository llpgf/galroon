//! Reviewed private classification inheritance; caller owns the grouping transaction.
use rusqlite::{Connection,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
type R<T>=Result<T,String>;
#[derive(Deserialize,Serialize)]pub struct Choice{pub id:String,pub follow:String}
#[derive(Deserialize,Serialize)]pub struct Selection{pub digest:String,pub tags:Vec<Choice>,pub entries:Vec<Choice>}
pub fn preview(c:&Connection,source:&str)->R<Value>{
 let tags={let mut q=c.prepare("WITH RECURSIVE ancestors(id) AS (SELECT ?1 UNION SELECT w.id FROM works w JOIN ancestors a ON w.merged_into=a.id) SELECT t.id,t.name,t.revision,t.deleted,m.work_id FROM custom_tags t JOIN custom_tag_works m ON m.tag_id=t.id WHERE m.work_id IN (SELECT id FROM ancestors) ORDER BY t.id,m.work_id").map_err(|e|e.to_string())?;let rows=q.query_map([source],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?,"deleted":r.get::<_,bool>(3)?,"work_id":r.get::<_,String>(4)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;rows};
 let entries={let mut q=c.prepare("SELECT e.id,e.list_id,l.name,l.revision,l.deleted,e.work_key,e.title,e.position,e.notes,e.preferred_release_id,e.added FROM list_entries e JOIN curated_lists l ON l.id=e.list_id ORDER BY e.list_id,e.position,e.id").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"list_id":r.get::<_,String>(1)?,"name":r.get::<_,String>(2)?,"revision":r.get::<_,i64>(3)?,"deleted":r.get::<_,bool>(4)?,"work_key":r.get::<_,String>(5)?,"title":r.get::<_,String>(6)?,"position":r.get::<_,i64>(7)?,"notes":r.get::<_,String>(8)?,"preferred_release_id":r.get::<_,Option<String>>(9)?,"added":r.get::<_,i64>(10)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;rows};
 let key=crate::lists::identity(c,&format!("local:{source}")).map_err(|e|e.0)?;
 let mut selected=Vec::new();for mut row in entries{if crate::lists::identity(c,row["work_key"].as_str().unwrap()).map_err(|e|e.0)?==key{
 let mut q=c.prepare("SELECT original_json FROM list_entry_variants WHERE entry_id=?1 ORDER BY id").map_err(|e|e.to_string())?;
 let variants=q.query_map([row["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;row["variants"]=json!(variants);selected.push(row);
 }}
 let mut result=json!({"source":source,"tags":tags,"entries":selected});let digest=hex::encode(Sha256::digest(result.to_string().as_bytes()));result["digest"]=json!(digest);Ok(result)
}
pub fn apply(c:&Connection,source:&str,target:&str,selection:Option<&Selection>)->R<()> {
 let snapshot=preview(c,source)?;
 let Some(selection)=selection else{if snapshot["tags"].as_array().unwrap().is_empty()&&snapshot["entries"].as_array().unwrap().is_empty(){return Ok(());}return Err("Review tag and list inheritance before splitting".into());};
 if snapshot["digest"].as_str()!=Some(&selection.digest){return Err("Private classifications changed. Reload the split preview.".into());}
 for (field,choices) in [("tags",&selection.tags),("entries",&selection.entries)]{
  let expected=snapshot[field].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<std::collections::BTreeSet<_>>();
  let actual=choices.iter().map(|v|v.id.as_str()).collect::<std::collections::BTreeSet<_>>();
  if expected!=actual||actual.len()!=choices.len()||choices.iter().any(|v|!matches!(v.follow.as_str(),"original"|"new"|"both")){return Err("Choose inheritance for each tag and list entry".into());}
 }
 for choice in &selection.tags{if choice.follow=="original"{continue;}
  c.execute("INSERT OR IGNORE INTO custom_tag_works(tag_id,work_id) VALUES(?1,?2)",params![choice.id,target]).map_err(|e|e.to_string())?;
  if choice.follow=="new"{c.execute("WITH RECURSIVE ancestors(id) AS (SELECT ?2 UNION SELECT w.id FROM works w JOIN ancestors a ON w.merged_into=a.id) DELETE FROM custom_tag_works WHERE tag_id=?1 AND work_id IN (SELECT id FROM ancestors)",params![choice.id,source]).map_err(|e|e.to_string())?;}
  c.execute("UPDATE custom_tags SET revision=revision+1 WHERE id=?1",[&choice.id]).map_err(|e|e.to_string())?;
 }
 let mut changed=std::collections::BTreeSet::new();
 for choice in &selection.entries{if choice.follow=="original"{continue;}let row=snapshot["entries"].as_array().unwrap().iter().find(|v|v["id"].as_str()==Some(&choice.id)).unwrap();let list=row["list_id"].as_str().unwrap();changed.insert(list.to_owned());
  if choice.follow=="new"{c.execute("UPDATE list_entries SET work_key=?2 WHERE id=?1",params![choice.id,format!("local:{target}")]).map_err(|e|e.to_string())?;}else{
   let new_id=crate::db::id();let position:i64=c.query_row("SELECT position FROM list_entries WHERE id=?1",[&choice.id],|r|r.get(0)).map_err(|e|e.to_string())?;
   c.execute("UPDATE list_entries SET position=position+1 WHERE list_id=?1 AND position>?2",params![list,position]).map_err(|e|e.to_string())?;
   c.execute("INSERT INTO list_entries(id,list_id,work_key,title,position,notes,preferred_release_id,added) SELECT ?2,list_id,?3,title,position+1,notes,preferred_release_id,added FROM list_entries WHERE id=?1",params![choice.id,new_id,format!("local:{target}")]).map_err(|e|e.to_string())?;
   for variant in row["variants"].as_array().unwrap(){c.execute("INSERT INTO list_entry_variants(id,entry_id,original_json) VALUES(?1,?2,?3)",params![crate::db::id(),new_id,variant.as_str().unwrap()]).map_err(|e|e.to_string())?;}
  }
 }
 for list in changed{c.execute("UPDATE curated_lists SET revision=revision+1 WHERE id=?1",[list]).map_err(|e|e.to_string())?;}Ok(())
}

#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->(tempfile::TempDir,crate::db::Db){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('a','A','A','v1'); INSERT INTO works(id,title,original_title,merged_into) VALUES('old','Old','Old','a'); INSERT INTO roots(id,path,label) VALUES('root','fixture','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('r','root','dummy','Dummy','archive','a'); INSERT INTO resource_bindings(resource_id,work_id,role) VALUES('r','a','unknown'); INSERT INTO custom_tags(id,name,name_key) VALUES('t','Tag','tag'); INSERT INTO custom_tag_works VALUES('t','old'); INSERT INTO curated_lists(id,name) VALUES('l','List'); INSERT INTO list_entries(id,list_id,work_key,title,position,notes,preferred_release_id,added) VALUES('e','l','vndb:v1','A',0,'Private note','saved-release',7),('other','l','vndb:v9','Other',1,'',NULL,8); INSERT INTO list_entry_variants VALUES('variant','e','{\"notes\":\"Older note\"}');").unwrap();(t,db)}
 fn choice(db:&crate::db::Db,follow:&str)->Selection{let p=preview(&db.lock().unwrap(),"a").unwrap();Selection{digest:p["digest"].as_str().unwrap().into(),tags:vec![Choice{id:"t".into(),follow:follow.into()}],entries:vec![Choice{id:"e".into(),follow:follow.into()}]}}
 #[test]fn split_choices_preserve_private_data_and_order(){for follow in ["original","new","both"]{
  let (_t,db)=fixture();let selection=choice(&db,follow);let result=crate::grouping::split_reviewed(&db,"a",1,&["r".into()],"New",Some(&selection)).unwrap();let target=result["id"].as_str().unwrap();let c=db.lock().unwrap();
  let tag_count:i64=c.query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get(0)).unwrap();assert_eq!(tag_count,if follow=="both"{2}else{1});
  let original:i64=c.query_row("SELECT count(*) FROM custom_tag_works WHERE work_id='old'",[],|r|r.get(0)).unwrap();assert_eq!(original,if follow=="new"{0}else{1});
  let key:String=c.query_row("SELECT work_key FROM list_entries WHERE id='e'",[],|r|r.get(0)).unwrap();assert_eq!(key,if follow=="new"{format!("local:{target}")}else{"vndb:v1".into()});
  let notes:i64=c.query_row("SELECT count(*) FROM list_entries WHERE notes='Private note' AND preferred_release_id='saved-release' AND added=7",[],|r|r.get(0)).unwrap();assert_eq!(notes,if follow=="both"{2}else{1});
  let pos:i64=c.query_row("SELECT position FROM list_entries WHERE id='other'",[],|r|r.get(0)).unwrap();assert_eq!(pos,if follow=="both"{2}else{1});
  let variants:i64=c.query_row("SELECT count(*) FROM list_entry_variants",[],|r|r.get(0)).unwrap();assert_eq!(variants,if follow=="both"{2}else{1});
 }}
 #[test]fn missing_stale_or_failed_private_choice_rolls_back_entire_split(){
  let (_t,db)=fixture();assert!(crate::grouping::split(&db,"a",1,&["r".into()],"New").is_err());let old=choice(&db,"new");db.lock().unwrap().execute("UPDATE list_entries SET notes='Changed' WHERE id='e'",[]).unwrap();assert!(crate::grouping::split_reviewed(&db,"a",1,&["r".into()],"New",Some(&old)).is_err());let current=choice(&db,"both");db.lock().unwrap().execute_batch("CREATE TRIGGER fail_copy BEFORE INSERT ON list_entry_variants BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();assert!(crate::grouping::split_reviewed(&db,"a",1,&["r".into()],"New",Some(&current)).is_err());let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(c.query_row("SELECT revision FROM works WHERE id='a'",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT work_id FROM resources WHERE id='r'",[],|r|r.get::<_,String>(0)).unwrap(),"a");assert_eq!(c.query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
}

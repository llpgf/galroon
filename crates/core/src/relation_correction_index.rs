//! Indexed candidate works affected by manual corrections; projection determines final membership.
use rusqlite::{Connection,params};use serde_json::{Value,json};
use crate::relation_corrections::{Correction,Key};
fn targets(key:&Key)->Vec<(&'static str,&str)>{match key{Key::Company{id}=>vec![("company",id)],Key::Character{id}=>vec![("character",id)],Key::Staff{id,..}=>vec![("person",id)],Key::Voice{id,character,..}=>vec![("person",id),("character",character)],Key::Work{id,..}=>vec![("work",id)]}}
pub fn replace(c:&Connection,work:&str,corrections:&[Correction])->rusqlite::Result<()>{
 c.execute("DELETE FROM relation_correction_targets WHERE vndb_id=?1",[work])?;
 for change in corrections{for key in change.replaces.iter().chain(change.link.iter().map(|v|&v.key)){for(kind,id)in targets(key){c.execute("INSERT OR IGNORE INTO relation_correction_targets(kind,entity_id,vndb_id) VALUES(?1,?2,?3)",params![kind,id,work])?;}}}Ok(())
}
pub fn migrate(c:&Connection,rebuild:bool)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS relation_correction_targets(kind TEXT NOT NULL,entity_id TEXT NOT NULL,vndb_id TEXT NOT NULL,PRIMARY KEY(kind,entity_id,vndb_id)); CREATE INDEX IF NOT EXISTS relation_correction_targets_work ON relation_correction_targets(vndb_id); CREATE TABLE IF NOT EXISTS relation_correction_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),revision INTEGER NOT NULL); INSERT OR IGNORE INTO relation_correction_state VALUES(1,0);")?;
 if rebuild{c.execute("DELETE FROM relation_correction_targets",[])?;let rows=c.prepare("SELECT vndb_id,corrections_json FROM relation_corrections")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for(work,raw)in rows{let corrections:Vec<Correction>=serde_json::from_str(&raw).map_err(|e|rusqlite::Error::FromSqlConversionFailure(1,rusqlite::types::Type::Text,Box::new(e)))?;replace(c,&work,&corrections)?;}}
 Ok(())
}
/// Return candidate IDs only, never claim these are complete or visible profile memberships.
pub fn page(c:&Connection,kind:&str,id:&str,after:Option<&str>,revision:Option<i64>)->Result<Value,String>{
 let prefix=match kind{"person"=>'s',"character"=>'c',"company"=>'p',"work"=>'v',_=>return Err("Invalid relationship target kind".into())};
 if !crate::exploration::valid_id(id,prefix)||after.is_some_and(|v|!crate::exploration::valid_id(v,'v'))||after.is_some()&&revision.is_none(){return Err("Invalid relationship page cursor".into());}
 let current=current_revision(c,revision)?;
 let mut q=c.prepare_cached("SELECT vndb_id FROM relation_correction_targets WHERE kind=?1 AND entity_id=?2 AND vndb_id>?3 ORDER BY vndb_id LIMIT 51").map_err(|e|e.to_string())?;
 let rows=q.query_map(params![kind,id,after.unwrap_or("")],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
 let more=rows.len()>50;let items:Vec<_>=rows.into_iter().take(50).collect();let next=if more{items.last().cloned()}else{None};Ok(json!({"candidate_work_ids":items,"next":next,"revision":current}))
}
#[cfg(test)]mod tests{
 use super::*;use crate::relation_corrections::Link;use crate::relation_correction_store::{Edit,change};
 fn input(revision:i64,request:String)->Edit{Edit{revision,request_id:request,corrections:vec![Correction{id:"voice".into(),replaces:Some(Key::Voice{id:"s1".into(),character:"c1".into(),alias:1,note:"".into()}),link:Some(Link{key:Key::Voice{id:"s2".into(),character:"c2".into(),alias:2,note:"".into()},name:"Alias".into(),character_name:Some("Character".into()),spoiler:2})}]}}
 #[test]fn all_endpoints_indexed_undo_clears_and_replay_does_not_advance(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let a=input(0,"add".into());change(&mut c,"v1",&a).ok().unwrap();change(&mut c,"v1",&a).ok().unwrap();for(kind,id)in [("person","s1"),("person","s2"),("character","c1"),("character","c2")]{let p=page(&c,kind,id,None,None).unwrap();assert_eq!(p["revision"],1);assert_eq!(p["candidate_work_ids"],json!(["v1"]));}change(&mut c,"v1",&Edit{revision:1,request_id:"undo".into(),corrections:vec![]}).ok().unwrap();assert_eq!(page(&c,"person","s1",None,None).unwrap()["candidate_work_ids"],json!([]));}
 #[test]fn stable_bounded_pages_reject_mutation_between_requests(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();for i in 1..=55{change(&mut c,&format!("v{i}"),&input(0,format!("r{i}"))).ok().unwrap();}let first=page(&c,"person","s2",None,None).unwrap();assert_eq!(first["candidate_work_ids"].as_array().unwrap().len(),50);let last=first["next"].as_str().unwrap();let second=page(&c,"person","s2",Some(last),Some(55)).unwrap();assert_eq!(second["candidate_work_ids"].as_array().unwrap().len(),5);assert!(second["next"].is_null());change(&mut c,"v56",&input(0,"r56".into())).ok().unwrap();assert!(page(&c,"person","s2",Some(last),Some(55)).is_err());assert!(page(&c,"person","s2",Some(last),None).is_err());}
 #[test]fn schema36_backfill_recovers_index_from_authoritative_decisions(){let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();{let mut c=db.lock().unwrap();change(&mut c,"v1",&input(0,"fixture".into())).ok().unwrap();c.execute_batch("DROP TABLE relation_correction_targets; DROP TABLE relation_correction_state; PRAGMA user_version=36;").unwrap();}drop(db);let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(page(&c,"person","s2",None,None).unwrap()["candidate_work_ids"],json!(["v1"]));assert_eq!(crate::relation_correction_store::read(&c,"v1").ok().unwrap().0,1);}
}

pub fn current_revision(c:&Connection,expected:Option<i64>)->Result<i64,String>{
 let current:i64=c.query_row("SELECT revision FROM relation_correction_state WHERE singleton=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if expected.is_some_and(|value|value!=current){return Err("Relationship corrections changed. Restart pagination.".into());}Ok(current)
}

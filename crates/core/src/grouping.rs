use crate::db::{Db,id,now};
use rusqlite::params;
use serde_json::{json,Value};
type R<T>=Result<T,String>;
// Grouping changes only catalog bindings. Source files are never touched.
pub fn merge(db:&Db,source:&str,target:&str,source_revision:i64,target_revision:i64)->R<Value>{
    if source==target{return Err("Choose a different destination work".into());}
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    for(w,revision)in [(source,source_revision),(target,target_revision)]{let current:i64=tx.query_row("SELECT revision FROM works WHERE id=?1 AND merged_into IS NULL",[w],|r|r.get(0)).map_err(|_|"Work is unavailable")?;if current!=revision{return Err("A work changed. Reload the grouping preview.".into());}}
    let resources:Vec<String>={let mut s=tx.prepare("SELECT resource_id FROM resource_bindings WHERE work_id=?1").map_err(|e|e.to_string())?;let rows=s.query_map([source],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;rows};
    tx.execute("INSERT OR IGNORE INTO work_releases(work_id,release_id) SELECT ?2,release_id FROM work_releases WHERE work_id=?1",params![source,target]).map_err(|e|e.to_string())?;
    for member in &resources{transfer(&tx,member,source,target)?;}
    crate::lists::merge_work(&tx,source,target).map_err(|e|e.0)?;
    tx.execute("UPDATE works SET merged_into=?2,revision=revision+1 WHERE id=?1",params![source,target]).map_err(|e|e.to_string())?;
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[target]).map_err(|e|e.to_string())?;
    let event=id();tx.execute("INSERT INTO grouping_history(id,kind,source,target,resources,created) VALUES(?1,'merge',?2,?3,?4,?5)",params![event,source,target,json!(resources).to_string(),now()]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(json!({"id":target,"history_id":event}))
}
pub fn split(db:&Db,source:&str,revision:i64,members:&[String],title:&str)->R<Value>{
    split_reviewed(db,source,revision,members,title,None)
}
pub fn split_reviewed(db:&Db,source:&str,revision:i64,members:&[String],title:&str,private:Option<&crate::split_private::Selection>)->R<Value>{
    if title.trim().is_empty()||members.is_empty(){return Err("Choose resources and enter a title for the new work".into());}
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    let current:i64=tx.query_row("SELECT revision FROM works WHERE id=?1 AND merged_into IS NULL",[source],|r|r.get(0)).map_err(|e|e.to_string())?;if current!=revision{return Err("The work changed. Reload the grouping preview.".into());}
    let distinct:std::collections::HashSet<_>=members.iter().collect();if distinct.len()!=members.len(){return Err("Duplicate resource selection".into());}
    for member in members{let matches:i64=tx.query_row("SELECT count(*) FROM resource_bindings WHERE resource_id=?1 AND work_id=?2",params![member,source],|r|r.get(0)).map_err(|e|e.to_string())?;if matches!=1{return Err("Resource bindings changed. Reload the grouping preview.".into());}}
    let target=id();let baseline=json!({"title":title,"original_title":title});tx.execute("INSERT INTO works(id,title,original_title,source_json) VALUES(?1,?2,?2,?3)",params![target,title,baseline.to_string()]).map_err(|e|e.to_string())?;
    tx.execute("INSERT OR IGNORE INTO work_releases(work_id,release_id) SELECT ?2,release_id FROM work_releases WHERE work_id=?1",params![source,target]).map_err(|e|e.to_string())?;
    crate::split_private::apply(&tx,source,&target,private)?;
    for member in members{transfer(&tx,member,source,&target)?;}
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[source]).map_err(|e|e.to_string())?;
    let event=id();tx.execute("INSERT INTO grouping_history(id,kind,source,target,resources,created) VALUES(?1,'split',?2,?3,?4,?5)",params![event,source,target,json!(members).to_string(),now()]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(json!({"id":target,"history_id":event}))
}
fn transfer(c:&rusqlite::Connection,rid:&str,source:&str,target:&str)->R<()>{
 let mut bindings=crate::editions::bindings(c,rid)?;let old=bindings.iter().find(|b|b.work_id==source).cloned().ok_or("Source binding is unavailable")?;bindings.retain(|b|b.work_id!=source);if !bindings.iter().any(|b|b.work_id==target){bindings.push(crate::editions::Binding{work_id:target.into(),..old});}
 let (revision,primary):(i64,Option<String>)=c.query_row("SELECT revision,work_id FROM resources WHERE id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;let primary=if primary.as_deref()==Some(source){Some(target.into())}else{primary};crate::editions::set_bindings_in(c,rid,crate::editions::EditBindings{revision,primary_work_id:primary,bindings})?;Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn grouping_changes_bindings_without_losing_source_metadata(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("db")).unwrap();{let c=db.lock().unwrap();c.execute_batch("INSERT INTO roots(id,path,label) VALUES('r','fixture','fixture');INSERT INTO works(id,title,original_title) VALUES('a','Alpha','Alpha'),('b','Beta','Beta');INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('x','r','one','One','archive','a'),('y','r','two','Two','archive','a'); INSERT INTO resource_bindings(resource_id,work_id,role) VALUES('x','a','unknown'),('y','a','unknown');").unwrap();}
 let split=split(&db,"a",1,&["x".into()],"Separate work").unwrap();let target=split["id"].as_str().unwrap();
 let revision=db.lock().unwrap().query_row("SELECT revision FROM works WHERE id=?1",[target],|r|r.get(0)).unwrap();merge(&db,target,"b",revision,1).unwrap();let c=db.lock().unwrap();let binding:String=c.query_row("SELECT work_id FROM resources WHERE id='x'",[],|r|r.get(0)).unwrap();assert_eq!(binding,"b");let title:String=c.query_row("SELECT title FROM works WHERE id=?1",[target],|r|r.get(0)).unwrap();assert_eq!(title,"Separate work");drop(c);assert!(merge(&db,"a","b",1,2).is_err());assert!(super::split(&db,"a",2,&["x".into()],"Wrong membership").is_err());
 }
}


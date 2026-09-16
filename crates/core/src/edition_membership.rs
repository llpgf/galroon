//! Bounded membership checks and atomic deltas. Off-page membership stays in Core.
use crate::{ApiError,App,db::Db,editions::EditionInput};
use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
use rusqlite::{Connection,params};
use serde::Deserialize;
use serde_json::{Value,json};
use std::collections::BTreeSet;
type R<T>=Result<T,ApiError>;
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
pub struct MembershipQuery {pub ids:String,pub revision:i64,pub collection_revision:i64,pub library_id:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
pub struct MembershipEdit {pub edition:EditionInput,pub add_work_ids:Vec<String>,pub remove_work_ids:Vec<String>,pub collection_revision:i64,pub library_id:String}
fn pin(c:&Connection,id:&str,revision:i64,catalog:i64,library:&str)->R<()> {
 if crate::context::library(c)?.id!=library || c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get::<_,i64>(0))?!=catalog {return Err(ApiError("Collection changed. Close and reopen the edition editor before saving.".into()));}
 if c.query_row("SELECT revision FROM releases WHERE id=?1",[id],|r|r.get::<_,i64>(0))?!=revision {return Err(ApiError("Edition changed. Close and reopen the edition editor before saving.".into()));}Ok(())
}
fn distinct(ids:&[String])->R<BTreeSet<String>> {
 if ids.iter().any(|id|id.is_empty()||id.len()>200) {return Err(ApiError("Invalid work identity".into()));}
 let set=ids.iter().cloned().collect::<BTreeSet<_>>();if set.len()!=ids.len(){return Err(ApiError("Choose distinct works".into()));}Ok(set)
}
pub(crate) fn membership(c:&Connection,id:&str,q:MembershipQuery)->R<Value> {
 if q.ids.len()>16000{return Err(ApiError("Work lookup is limited to 60 identities".into()));}
 let ids:Vec<String>=serde_json::from_str(&q.ids).map_err(|_|ApiError("Invalid work identities".into()))?;
 if ids.len()>60{return Err(ApiError("Work lookup is limited to 60 identities".into()));}distinct(&ids)?;
 pin(c,id,q.revision,q.collection_revision,&q.library_id)?;
 let mut query=c.prepare("SELECT EXISTS(SELECT 1 FROM work_releases WHERE work_id=?1 AND release_id=?2) FROM works WHERE id=?1 AND merged_into IS NULL")?;
 let mut members=vec![];for wid in &ids {let selected:bool=query.query_row(params![wid,id],|r|r.get(0)).map_err(|_|ApiError("Work is unavailable. Close and reopen the edition editor.".into()))?;if selected{members.push(wid);}}
 Ok(json!({"ids":ids,"members":members,"revision":q.revision,"collection_revision":q.collection_revision,"library_id":q.library_id}))
}
pub fn edit(db:&Db,id:&str,mut v:MembershipEdit)->R<Value> {
 if !v.edition.work_ids.is_empty(){return Err(ApiError("Membership edits use additions and removals, not replacement work IDs".into()));}
 let added=distinct(&v.add_work_ids)?;let removed=distinct(&v.remove_work_ids)?;
 if !added.is_disjoint(&removed){return Err(ApiError("A work cannot be both added and removed".into()));}
 let c=db.lock().map_err(|e|ApiError(e.to_string()))?;let mut checkpoint=crate::checkpoint::LargeEdition::new(&c,Some(id),added.len())?;let tx=c.unchecked_transaction()?;
 pin(&tx,id,v.edition.revision.ok_or_else(||ApiError("Edition revision is required".into()))?,v.collection_revision,&v.library_id)?;
 let mut existing={let mut q=tx.prepare("SELECT work_id FROM work_releases WHERE release_id=?1")?;let rows=q.query_map([id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<BTreeSet<_>>>()?;rows};
 for wid in &removed{if !existing.remove(wid){return Err(ApiError("Edition membership changed. Close and reopen the editor.".into()));}}
 for wid in &added{let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE id=?1 AND merged_into IS NULL)",[wid],|r|r.get(0))?;if !active||!existing.insert(wid.clone()){return Err(ApiError("Work is unavailable or already belongs to this edition".into()));}}
 let active_count:i64=tx.query_row("SELECT count(*) FROM work_releases wr JOIN works w ON w.id=wr.work_id WHERE wr.release_id=?1 AND w.merged_into IS NULL",[id],|r|r.get(0))?;
 let mut active_removed=0;for wid in &removed {if tx.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE id=?1 AND merged_into IS NULL)",[wid],|r|r.get::<_,bool>(0))?{active_removed+=1;}}
 if active_count-active_removed+added.len() as i64<=0{return Err(ApiError("Choose at least one active work for this edition".into()));}
 v.edition.work_ids=existing.into_iter().collect();
 let result=crate::editions::save_in(&tx,Some(id),v.edition,true)?;tx.commit()?;checkpoint.committed();Ok(result)
}
pub async fn read(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<MembershipQuery>)->crate::Result<Value>{crate::auth(&a,&h)?;crate::work_catalog::read_snapshot(a,move|c|membership(c,&id,q)).await}
pub async fn write(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<MembershipEdit>)->crate::Result<Value>{crate::auth(&a,&h)?;let value=tokio::task::spawn_blocking(move||edit(&a.db,&id,v)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(value))}

#[cfg(test)]mod tests {
 use super::*;
 fn fixture()->(tempfile::TempDir,Db,String){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','Alpha','Alpha'),('b','Beta','Beta'),('c','Gamma','Gamma');INSERT INTO releases(id,label) VALUES('e','Shared');INSERT INTO work_releases(work_id,release_id) VALUES('a','e'),('b','e');").unwrap();(t,db,"e".into())}
 fn request(db:&Db)->MembershipEdit {let c=db.lock().unwrap();MembershipEdit{edition:serde_json::from_value(json!({"label":"Updated","work_ids":[],"revision":1})).unwrap(),add_work_ids:vec![],remove_work_ids:vec![],collection_revision:c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0)).unwrap(),library_id:crate::context::library(&c).unwrap().id}}
 fn check(db:&Db,ids:Value)->R<Value>{let v=request(db);membership(&db.lock().unwrap(),"e",MembershipQuery{ids:ids.to_string(),revision:1,collection_revision:v.collection_revision,library_id:v.library_id})}
 #[test]fn membership_is_bounded_exact_and_fail_closed(){let(_t,db,_)=fixture();let page=check(&db,json!(["a","c"])).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(page["members"],json!(["a"]));assert_eq!(page["ids"],json!(["a","c"]));assert!(check(&db,json!(["absent"])).is_err());assert!(check(&db,json!(["a","a"])).is_err());assert!(check(&db,json!((0..61).map(|n|format!("w{n}")).collect::<Vec<_>>())).is_err());assert!(check(&db,json!([])).is_ok());let mut stale=request(&db);stale.library_id="elsewhere".into();assert!(edit(&db,"e",stale).is_err());}
 #[test]fn deltas_keep_off_page_members_and_advance_union_once(){let(_t,db,e)=fixture();let mut update=request(&db);update.add_work_ids=vec!["c".into()];update.remove_work_ids=vec!["a".into()];let stale=request(&db);edit(&db,&e,update).unwrap_or_else(|e|panic!("{}",e.0));assert!(edit(&db,&e,stale).is_err());let c=db.lock().unwrap();assert_eq!(crate::editions::one(&c,&e).unwrap().unwrap()["work_ids"],json!(["b","c"]));for id in ["a","b","c"]{assert_eq!(c.query_row("SELECT revision FROM works WHERE id=?1",[id],|r|r.get::<_,i64>(0)).unwrap(),2);}assert_eq!(c.query_row("SELECT label FROM releases WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"Updated");}
 #[test]fn guards_and_merge_conflicts_roll_back_metadata_and_membership(){let(_t,db,e)=fixture();db.lock().unwrap().execute_batch("INSERT INTO work_preferences(work_id,preferred_release_id) VALUES('a','e');").unwrap();let mut remove=request(&db);remove.remove_work_ids=vec!["a".into()];assert!(edit(&db,&e,remove).is_err());db.lock().unwrap().execute_batch("DELETE FROM work_preferences;INSERT INTO roots(id,path,label) VALUES('r','generated','Generated');INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('x','r','sample','Sample','file');INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES('x','a','e','main');").unwrap();let mut remove=request(&db);remove.remove_work_ids=vec!["a".into()];assert!(edit(&db,&e,remove).is_err());let mut all=request(&db);all.remove_work_ids=vec!["a".into(),"b".into()];assert!(edit(&db,&e,all).is_err());let mut duplicate=request(&db);duplicate.add_work_ids=vec!["b".into()];assert!(edit(&db,&e,duplicate).is_err());assert_eq!(crate::editions::one(&db.lock().unwrap(),&e).unwrap().unwrap()["label"],"Shared");let before_merge=request(&db);crate::grouping::merge(&db,"b","c",1,1).unwrap();assert!(edit(&db,&e,before_merge).is_err());let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT revision FROM releases WHERE id='e'",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
}

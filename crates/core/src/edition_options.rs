//! Bounded edition choices for one active work. No shared membership expansion.
use axum::{extract::{Path,Query,State},http::HeaderMap,Json};
use serde::{Deserialize,Serialize};use serde_json::{json,Value};use rusqlite::params;
use crate::{App,ApiError};
#[derive(Default,Deserialize)]pub struct Options{#[serde(default)]query:String,before:Option<String>}
#[derive(Serialize,Deserialize)]struct Cursor{library:String,revision:i64,work:String,query:String,offset:i64}
pub async fn read(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<Options>)->crate::Result<Value>{crate::auth(&a,&h)?;Ok(Json(page(&a.db.lock().unwrap(),&id,&q)?))}
fn page(c:&rusqlite::Connection,work:&str,q:&Options)->Result<Value,ApiError>{
 if q.query.len()>512{return Err(ApiError("Edition search is too long".into()));}
 let active:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE id=?1 AND merged_into IS NULL)",[work],|r|r.get(0))?;if !active{return Err(ApiError("Work not found or merged".into()));}
 let library=crate::context::library(c)?.id;let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
 let offset=if let Some(raw)=&q.before{if raw.len()>4096{return Err(ApiError("Invalid edition cursor".into()));}let cursor:Cursor=serde_json::from_str(raw).map_err(|_|ApiError("Invalid edition cursor".into()))?;if cursor.library!=library||cursor.revision!=revision||cursor.work!=work||cursor.query!=q.query||cursor.offset<0{return Err(ApiError("Edition choices changed. Restart the search.".into()));}cursor.offset}else{0};
 let filter=" FROM work_releases wr JOIN releases r ON r.id=wr.release_id WHERE wr.work_id=?1 AND instr(lower(r.label),lower(?2))>0";
 let total:i64=c.query_row(&format!("SELECT count(*){filter}"),params![work,q.query],|r|r.get(0))?;
 let mut statement=c.prepare(&format!("SELECT r.id,r.label,r.revision{filter} ORDER BY r.label,r.id LIMIT 61 OFFSET ?3"))?;
 let mut items=statement.query_map(params![work,q.query,offset],|r|Ok(json!({"id":r.get::<_,String>(0)?,"label":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=items.len()>60;items.truncate(60);let next=if more{Some(serde_json::to_string(&Cursor{library:library.clone(),revision,work:work.to_owned(),query:q.query.clone(),offset:offset+60}).map_err(|e|ApiError(e.to_string()))?)}else{None};
 Ok(json!({"items":items,"next":next,"total":total,"revision":revision,"library_id":library,"work_id":work}))
}
#[derive(Deserialize)]pub struct LabelQuery{ids:String,revision:Option<i64>,library_id:Option<String>}
pub async fn labels(State(a):State<App>,h:HeaderMap,Query(q):Query<LabelQuery>)->crate::Result<Value>{crate::auth(&a,&h)?;Ok(Json(label_page(&a.db.lock().unwrap(),&q)?))}
fn label_page(c:&rusqlite::Connection,q:&LabelQuery)->Result<Value,ApiError>{
 if q.ids.len()>16384{return Err(ApiError("Edition lookup is too large".into()));}let ids:Vec<String>=serde_json::from_str(&q.ids).map_err(|_|ApiError("Invalid edition IDs".into()))?;
 if ids.len()>60||ids.iter().collect::<std::collections::HashSet<_>>().len()!=ids.len(){return Err(ApiError("Select at most 60 distinct editions".into()));}
 let library=crate::context::library(c)?.id;let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
 if q.revision.is_some_and(|r|r!=revision)||q.library_id.as_ref().is_some_and(|v|v!=&library){return Err(ApiError("Edition labels changed. Retry before reviewing.".into()));}
 let mut items=Vec::new();let mut missing=Vec::new();let mut statement=c.prepare("SELECT id,label FROM releases WHERE id=?1")?;
 for id in ids{use rusqlite::OptionalExtension;let item=statement.query_row([&id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"label":r.get::<_,String>(1)?}))).optional()?;if let Some(item)=item{items.push(item);}else{missing.push(id);}}
 Ok(json!({"items":items,"missing":missing,"revision":revision,"library_id":library}))
}
#[cfg(test)]mod tests{use super::*;
#[test]fn pages_are_scoped_complete_searchable_and_revision_guarded(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','A','A'),('b','B','B');").unwrap();for i in 0..125{let id=format!("r{i:03}");c.execute("INSERT INTO releases(id,label) VALUES(?1,?2)",params![id,format!("Edition {i:03}")]).unwrap();c.execute("INSERT INTO work_releases(work_id,release_id) VALUES('a',?1),('b',?1)",[id]).unwrap();}c.execute_batch("INSERT INTO releases(id,label) VALUES('other','Other');INSERT INTO work_releases(work_id,release_id) VALUES('b','other');").unwrap();
 let mut q=Options::default();let mut all=Vec::new();loop{let result=page(&c,"a",&q).ok().unwrap();assert_eq!(result["total"],125);let items=result["items"].as_array().unwrap();assert!(items.len()<=60);for item in items{assert!(item.get("work_ids").is_none());all.push(item["id"].as_str().unwrap().to_owned());}q.before=result["next"].as_str().map(str::to_owned);if q.before.is_none(){break;}}assert_eq!(all.len(),125);assert_eq!(all.iter().collect::<std::collections::HashSet<_>>().len(),125);
 let first=page(&c,"a",&Options::default()).ok().unwrap();let cursor=first["next"].as_str().unwrap().to_owned();assert!(page(&c,"b",&Options{query:String::new(),before:Some(cursor.clone())}).is_err());assert!(page(&c,"a",&Options{query:"124".into(),before:Some(cursor.clone())}).is_err());let filtered=page(&c,"a",&Options{query:"124".into(),before:None}).ok().unwrap();assert_eq!(filtered["items"][0]["id"],"r124");assert_eq!(filtered["total"],1);
 c.execute("UPDATE releases SET label='Changed' WHERE id='r000'",[]).unwrap();assert!(page(&c,"a",&Options{query:String::new(),before:Some(cursor)}).is_err());c.execute("UPDATE works SET merged_into='b' WHERE id='a'",[]).unwrap();assert!(page(&c,"a",&Options::default()).is_err());}
}
#[cfg(test)]mod label_tests{use super::*;#[test]fn labels_are_exact_bounded_and_revision_pinned(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO releases(id,label) VALUES('a','First'),('b','Second');").unwrap();let q=LabelQuery{ids:"[\"b\",\"absent\",\"a\"]".into(),revision:None,library_id:None};let result=label_page(&c,&q).ok().unwrap();assert_eq!(result["items"],json!([{"id":"b","label":"Second"},{"id":"a","label":"First"}]));assert_eq!(result["missing"],json!(["absent"]));for ids in [json!(["a","a"]).to_string(),json!((0..61).map(|i|i.to_string()).collect::<Vec<_>>()).to_string()]{assert!(label_page(&c,&LabelQuery{ids,revision:None,library_id:None}).is_err());}let pinned=LabelQuery{ids:"[\"a\"]".into(),revision:result["revision"].as_i64(),library_id:result["library_id"].as_str().map(str::to_owned)};c.execute("UPDATE releases SET label='Renamed' WHERE id='a'",[]).unwrap();assert!(label_page(&c,&pinned).is_err());}}

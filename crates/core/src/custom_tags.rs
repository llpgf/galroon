use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
use serde::Deserialize;
use serde_json::{json,Value};
use rusqlite::params;
use unicode_normalization::UnicodeNormalization;
use crate::{App,ApiError,Result};

fn name_key(name:&str)->String { name.trim().nfc().collect::<String>().to_lowercase().nfc().collect() }
// Called inside the catalog migration transaction, after its protection snapshot.
pub fn migrate_names(c:&rusqlite::Connection)->rusqlite::Result<()> {
 let rows={let mut q=c.prepare("SELECT id,name FROM custom_tags")?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
 // Temporary keys cannot be user names (control characters are rejected).
 for (id,_) in &rows {c.execute("UPDATE custom_tags SET name_key=?2 WHERE id=?1",params![id,format!("\0{id}")])?;}
 for (id,name) in &rows {c.execute("UPDATE custom_tags SET name_key=?2 WHERE id=?1",params![id,name_key(name)])?;}
 Ok(())
}

pub fn migrate(c:&rusqlite::Connection)->rusqlite::Result<()> {
 c.execute_batch("CREATE TABLE IF NOT EXISTS custom_tags(id TEXT PRIMARY KEY,name TEXT NOT NULL,name_key TEXT NOT NULL UNIQUE);
 CREATE TABLE IF NOT EXISTS custom_tag_works(tag_id TEXT NOT NULL REFERENCES custom_tags(id),work_id TEXT NOT NULL REFERENCES works(id),PRIMARY KEY(tag_id,work_id));")?;
 let has:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('custom_tags') WHERE name='revision')",[],|r|r.get(0))?;if !has{c.execute_batch("ALTER TABLE custom_tags ADD COLUMN revision INTEGER NOT NULL DEFAULT 1; ALTER TABLE custom_tags ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;")?;}
 c.execute_batch("CREATE TABLE IF NOT EXISTS tag_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL); CREATE TABLE IF NOT EXISTS tag_undo(tag_id TEXT NOT NULL,revision INTEGER NOT NULL,before_ids TEXT NOT NULL,after_ids TEXT NOT NULL,topology TEXT NOT NULL,PRIMARY KEY(tag_id,revision));")
}

#[derive(Deserialize)]pub struct ListQuery{#[serde(default)]include_deleted:bool,#[serde(default)]summary:bool}
pub async fn list(State(a):State<App>,h:HeaderMap,Query(q):Query<ListQuery>)->Result<Value>{
 if q.include_deleted&&crate::access::authenticate(&a,&h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}
 crate::auth(&a,&h)?;
 let c=a.db.lock().unwrap();
 Ok(Json(list_data(&c,q.include_deleted,q.summary)?))
}
fn list_data(c:&rusqlite::Connection,include_deleted:bool,summary:bool)->std::result::Result<Value,ApiError>{
 let mut statement=c.prepare("SELECT id,name,revision,deleted FROM custom_tags WHERE deleted=0 OR ?1 ORDER BY name_key")?;
 let rows=statement.query_map([include_deleted],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,bool>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let dependencies=crate::smart_lists::dependency_counts(&c)?;
 let impact_revision:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
 let has_merges:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE merged_into IS NOT NULL)",[],|r|r.get(0))?;
 let merged_counts=if summary&&has_merges{
  let mut q=c.prepare("WITH RECURSIVE resolved(id,target) AS (SELECT id,id FROM works WHERE merged_into IS NULL UNION SELECT w.id,r.target FROM works w JOIN resolved r ON w.merged_into=r.id) SELECT t.tag_id,count(DISTINCT r.target) FROM custom_tag_works t JOIN resolved r ON r.id=t.work_id GROUP BY t.tag_id")?;
  let counts=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<rusqlite::Result<std::collections::HashMap<_,_>>>()?;counts
 }else{std::collections::HashMap::new()};
 let mut result=Vec::new();
 for(id,name,revision,deleted)in rows {
  // Resolve merged identities without rewriting the user's original assignments.
  let mut members=c.prepare("WITH RECURSIVE members(id) AS (SELECT work_id FROM custom_tag_works WHERE tag_id=?1 UNION SELECT w.merged_into FROM works w JOIN members m ON w.id=m.id WHERE w.merged_into IS NOT NULL) SELECT DISTINCT w.id FROM works w JOIN members m ON w.id=m.id WHERE w.merged_into IS NULL ORDER BY w.id")?;
  let (work_count,work_ids)=if summary {
   let count:i64=if !has_merges{c.query_row("SELECT count(*) FROM custom_tag_works WHERE tag_id=?1",[&id],|r|r.get(0))?}else{merged_counts.get(&id).copied().unwrap_or(0)};
   (count,None)
  }else{let ids=members.query_map([&id],|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;(ids.len() as i64,Some(ids))};
  let undo_available=!deleted&&c.query_row("SELECT EXISTS(SELECT 1 FROM tag_undo WHERE tag_id=?1 AND revision=?2)",params![id,revision],|r|r.get::<_,bool>(0))?;
  let entity_count:i64=c.prepare_cached("SELECT count(*) FROM custom_tag_entities WHERE tag_id=?1")?.query_row([&id],|r|r.get(0))?;
  let mut item=json!({"work_count":work_count,"impact_revision":impact_revision,"smart_list_count":dependencies.get(&id).copied().unwrap_or(0),"entity_count":entity_count,"undo_available":undo_available,"id":id,"name":name,"work_ids":work_ids,"revision":revision,"deleted":deleted});
  if summary{item.as_object_mut().unwrap().remove("work_ids");}result.push(item);
 }
 Ok(json!(result))
}

#[derive(Deserialize)]pub struct MembershipQuery{ids:String,revision:i64,impact_revision:i64}
pub async fn membership(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<MembershipQuery>)->Result<Value>{
 if crate::access::authenticate(&a,&h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}
 let ids:Vec<String>=serde_json::from_str(&q.ids).map_err(|_|ApiError("Invalid work IDs".into()))?;
 let c=a.db.lock().unwrap();Ok(Json(membership_data(&c,&id,&ids,q.revision,q.impact_revision)?))
}
fn membership_data(c:&rusqlite::Connection,id:&str,ids:&[String],revision:i64,impact_revision:i64)->std::result::Result<Value,ApiError>{
 if ids.len()>60||ids.iter().collect::<std::collections::HashSet<_>>().len()!=ids.len(){return Err(ApiError("Select at most 60 distinct work IDs".into()));}
 let actual:i64=c.query_row("SELECT revision FROM custom_tags WHERE id=?1",[id],|r|r.get(0))?;
 let impact:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
 if actual!=revision||impact!=impact_revision{return Err(ApiError("Tag or collection changed. Refresh tags before continuing.".into()));}
 // Walk backwards from the requested active identities, including legacy merged assignments.
 let encoded=serde_json::to_string(ids).map_err(|e|ApiError(e.to_string()))?;
 let mut query=c.prepare("WITH RECURSIVE ancestors(requested,id) AS (SELECT w.id,w.id FROM json_each(?2) j JOIN works w ON w.id=j.value WHERE w.merged_into IS NULL UNION SELECT a.requested,w.id FROM works w JOIN ancestors a ON w.merged_into=a.id) SELECT DISTINCT a.requested FROM ancestors a JOIN custom_tag_works t ON t.work_id=a.id AND t.tag_id=?1")?;
 let found=query.query_map(params![id,encoded],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
 let members=ids.iter().filter(|wid|found.contains(*wid)).collect::<Vec<_>>();
 Ok(json!({"ids":ids,"members":members,"revision":revision,"impact_revision":impact_revision}))
}

#[derive(Deserialize)]pub struct Create {name:String,#[serde(default)]work_ids:Vec<String>}
fn create_tag(db:&crate::db::Db,input:Create)->std::result::Result<Value,ApiError>{
 let name=input.name.trim();
 if name.is_empty()||name.chars().count()>80||name.chars().any(char::is_control){return Err(ApiError("Use a tag name of 1–80 characters without control characters.".into()));}
 if input.work_ids.len()>10000{return Err(ApiError("Too many selected works.".into()));}
 let key=name_key(name);
 let mut connection=db.lock().unwrap();let c=connection.transaction()?;
 if c.query_row("SELECT EXISTS(SELECT 1 FROM custom_tags WHERE name_key=?1)",[&key],|r|r.get::<_,bool>(0))?{return Err(ApiError("A custom tag with this name already exists.".into()));}
 let id=crate::db::id();
 c.execute("INSERT INTO custom_tags(id,name,name_key) VALUES(?1,?2,?3)",params![id,name,key])?;
 for work_id in &input.work_ids {
  if !c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE id=?1 AND merged_into IS NULL)",[work_id],|r|r.get::<_,bool>(0))?{return Err(ApiError("A selected work changed. Reload the collection and try again.".into()));}
  c.execute("INSERT OR IGNORE INTO custom_tag_works(tag_id,work_id) VALUES(?1,?2)",params![id,work_id])?;
 }
 c.commit()?;Ok(json!({"id":id,"name":name,"work_ids":input.work_ids}))
}
pub async fn create(State(a):State<App>,h:HeaderMap,Json(input):Json<Create>)->Result<Value>{
 crate::auth(&a,&h)?;Ok(Json(create_tag(&a.db,input)?))
}
#[derive(Deserialize,serde::Serialize)]pub struct Edit{#[serde(default,skip_serializing_if="Option::is_none")]impact_revision:Option<i64>,revision:i64,request_id:String,action:String,#[serde(default,skip_serializing_if="Option::is_none")]name:Option<String>,#[serde(default)]work_ids:Vec<String>}
fn raw_members(c:&rusqlite::Connection,id:&str)->rusqlite::Result<Vec<String>>{let mut q=c.prepare("SELECT work_id FROM custom_tag_works WHERE tag_id=?1 ORDER BY work_id")?;let rows=q.query_map([id],|r|r.get(0))?.collect();rows}
fn topology(c:&rusqlite::Connection,ids:&[String])->rusqlite::Result<Vec<(String,Option<String>)>>{ids.iter().map(|id|c.query_row("SELECT id,merged_into FROM works WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))).collect()}
fn change(c:&rusqlite::Connection,id:&str,input:&Edit)->std::result::Result<Value,ApiError>{
 use rusqlite::OptionalExtension;use sha2::{Digest,Sha256};
 if input.request_id.len()>128||input.request_id.is_empty(){return Err(ApiError("Request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((old,result))=c.query_row("SELECT digest,result FROM tag_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=digest{return Err(ApiError("Request ID already used for another edit".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let(revision,deleted):(i64,bool)=c.query_row("SELECT revision,deleted FROM custom_tags WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;if revision!=input.revision{return Err(ApiError("Tag changed. Reload before editing.".into()));}
 if input.action=="delete"{if let Some(expected)=input.impact_revision{let current:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;if current!=expected{return Err(ApiError("Tag impact changed. Reload and review deletion again.".into()));}}}
 if input.work_ids.len()>10000{return Err(ApiError("Too many selected works".into()));}
 let before=if matches!(input.action.as_str(),"add"|"remove"){Some(raw_members(c,id)?)}else{None};
 let batch_summary=if before.is_some(){
 let mut q=c.prepare("WITH RECURSIVE members(id) AS (SELECT work_id FROM custom_tag_works WHERE tag_id=?1 UNION SELECT w.merged_into FROM works w JOIN members m ON w.id=m.id WHERE w.merged_into IS NOT NULL) SELECT DISTINCT w.id FROM works w JOIN members m ON w.id=m.id WHERE w.merged_into IS NULL")?;
 let members=q.query_map([id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
 let selected=input.work_ids.iter().collect::<std::collections::HashSet<_>>();let changed=selected.iter().filter(|wid|members.contains(wid.as_str())==(input.action=="remove")).count();
 Some(json!({"processed":selected.len(),"changed":changed,"unchanged":selected.len()-changed,"failed":0}))
 }else{None};
 match input.action.as_str(){
  "undo" if !deleted && input.work_ids.is_empty()=>{
   let (old,after,identity):(String,String,String)=c.query_row("SELECT before_ids,after_ids,topology FROM tag_undo WHERE tag_id=?1 AND revision=?2",params![id,revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
   let decode=|s:&str|serde_json::from_str::<Vec<String>>(s).map_err(|e|ApiError(e.to_string()));let old=decode(&old)?;let after=decode(&after)?;
   if raw_members(c,id)?!=after{return Err(ApiError("Tag links changed. Undo is unavailable.".into()));}
   let saved:Vec<(String,Option<String>)>=serde_json::from_str(&identity).map_err(|e|ApiError(e.to_string()))?;
   if topology(c,&saved.iter().map(|v|v.0.clone()).collect::<Vec<_>>())?!=saved{return Err(ApiError("Work identities changed. Review tag assignments instead.".into()));}
   c.execute("DELETE FROM custom_tag_works WHERE tag_id=?1",[id])?;for wid in old{c.execute("INSERT INTO custom_tag_works(tag_id,work_id) VALUES(?1,?2)",params![id,wid])?;}
  },
  "rename" if !deleted && input.work_ids.is_empty()=>{
   let name=input.name.as_deref().unwrap_or("").trim();
   if name.is_empty()||name.chars().count()>80||name.chars().any(char::is_control){return Err(ApiError("Use a tag name of 1–80 characters without control characters.".into()));}
   let key=name_key(name);
   if c.query_row("SELECT EXISTS(SELECT 1 FROM custom_tags WHERE name_key=?1 AND id<>?2)",params![key,id],|r|r.get::<_,bool>(0))?{return Err(ApiError("A custom tag with this name already exists.".into()));}
   c.execute("UPDATE custom_tags SET name=?2,name_key=?3 WHERE id=?1",params![id,name,key])?;
  },
  "add"|"remove" if !deleted=>{if input.work_ids.is_empty(){return Err(ApiError("Select works first".into()));}for wid in &input.work_ids{if !c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE id=?1 AND merged_into IS NULL)",[wid],|r|r.get::<_,bool>(0))?{return Err(ApiError("A selected work changed. Reload first.".into()));}if input.action=="add"{c.execute("INSERT OR IGNORE INTO custom_tag_works(tag_id,work_id) VALUES(?1,?2)",params![id,wid])?;}else{c.execute("WITH RECURSIVE ancestors(id) AS (SELECT ?2 UNION SELECT w.id FROM works w JOIN ancestors a ON w.merged_into=a.id) DELETE FROM custom_tag_works WHERE tag_id=?1 AND work_id IN (SELECT id FROM ancestors)",params![id,wid])?;}}},
  "delete"|"restore" if input.work_ids.is_empty()=>{c.execute("UPDATE custom_tags SET deleted=?2 WHERE id=?1",params![id,input.action=="delete"])?;},
  _=>return Err(ApiError("Tag action unavailable".into()))
 }
 if let Some(before)=before{let after=raw_members(c,id)?;let mut ids=before.clone();ids.extend(after.iter().cloned());ids.extend(input.work_ids.iter().cloned());ids.sort();ids.dedup();let identity=topology(c,&ids)?;
 c.execute("INSERT INTO tag_undo(tag_id,revision,before_ids,after_ids,topology) VALUES(?1,?2,?3,?4,?5)",params![id,revision+1,json!(before).to_string(),json!(after).to_string(),json!(identity).to_string()])?;}
 c.execute("UPDATE custom_tags SET revision=revision+1 WHERE id=?1",[id])?;let mut result=json!({"id":id,"revision":revision+1});if let Some(summary)=batch_summary{result["batch"]=summary;}c.execute("INSERT INTO tag_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub async fn edit(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Edit>)->Result<Value>{crate::auth(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let v=change(&tx,&id,&input)?;tx.commit()?;Ok(Json(v))}

#[cfg(test)]mod tests {
#[test]fn existing_collection_reopen_adds_parent_index_without_changing_membership(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
 {let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('old','Old','Old'),('current','Current','Current'); UPDATE works SET merged_into='current' WHERE id='old'; INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag'); INSERT INTO custom_tag_works(tag_id,work_id) VALUES('tag','old'); DROP INDEX works_merged_parent;").unwrap();}drop(db);
 let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT work_id FROM custom_tag_works WHERE tag_id='tag'",[],|r|r.get::<_,String>(0)).unwrap(),"old");
 let plan=c.query_row("EXPLAIN QUERY PLAN SELECT id FROM works WHERE merged_into='current'",[],|r|r.get::<_,String>(3)).unwrap();assert!(plan.contains("works_merged_parent"),"{plan}");
 let summary=super::list_data(&c,true,true).ok().unwrap();assert_eq!(summary[0]["work_count"],1);
 let page=super::membership_data(&c,"tag",&["current".into()],1,summary[0]["impact_revision"].as_i64().unwrap()).ok().unwrap();assert_eq!(page["members"],serde_json::json!(["current"]));
 assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
}

#[test]fn summary_and_bounded_membership_preserve_merged_identity_and_revision(){
 let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
 c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('old','Old','Old'),('new','New','New'),('other','Other','Other'); UPDATE works SET merged_into='new' WHERE id='old'; INSERT INTO custom_tags(id,name,name_key) VALUES('t','Tag','tag'); INSERT INTO custom_tag_works(tag_id,work_id) VALUES('t','old'),('t','new');").unwrap();
 let summary=super::list_data(&c,true,true).ok().unwrap();assert_eq!(summary[0]["work_count"],1);assert!(summary[0].get("work_ids").is_none());
 let legacy=super::list_data(&c,true,false).ok().unwrap();assert_eq!(legacy[0]["work_ids"],serde_json::json!(["new"]));
 let impact=summary[0]["impact_revision"].as_i64().unwrap();let ids=vec!["new".to_owned(),"other".to_owned(),"old".to_owned()];
 let page=super::membership_data(&c,"t",&ids,1,impact).ok().unwrap();assert_eq!(page["members"],serde_json::json!(["new"]));
 assert!(super::membership_data(&c,"t",&ids,2,impact).is_err());assert!(super::membership_data(&c,"t",&ids,1,impact+1).is_err());
 assert!(super::membership_data(&c,"t",&vec!["new".into();61],1,impact).is_err());assert!(super::membership_data(&c,"t",&vec!["new".into();2],1,impact).is_err());
}
#[test]fn dense_summary_omits_all_member_ids(){
 let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
 c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Tag','tag'); WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO works(id,title,original_title) SELECT 'w'||x,'Work','Work' FROM n; INSERT INTO custom_tag_works(tag_id,work_id) SELECT 't',id FROM works;").unwrap();
 let data=super::list_data(&c,true,true).ok().unwrap();assert_eq!(data[0]["work_count"],10000);assert!(serde_json::to_vec(&data).unwrap().len()<400);
}

 use super::*;
 #[test]fn deletion_dependency_counts_and_stale_review_are_safe(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute("INSERT INTO custom_tags(id,name,name_key) VALUES('t','T','t')",[]).unwrap();
 let predicate=|field|json!({"kind":"field","predicate":{"field":field,"mode":"any","values":["t"]}});
 let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"group","join":"all","children":[predicate("personal_tags"),predicate("related_personal_tags")]}});
 for(id,deleted)in [("active",false),("deleted",true)]{c.execute("INSERT INTO smart_lists(id,name,description,revision,definition,deleted) VALUES(?1,?1,'',1,?2,?3)",params![id,definition.to_string(),deleted]).unwrap();}
 assert_eq!(crate::smart_lists::dependency_counts(&c).ok().unwrap().get("t"),Some(&1));
 let old=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap();c.execute("UPDATE smart_lists SET deleted=0 WHERE id='deleted'",[]).unwrap();
 let mut input=Edit{impact_revision:Some(old),revision:1,request_id:"delete".into(),action:"delete".into(),name:None,work_ids:vec![]};
 {let tx=c.transaction().unwrap();assert!(change(&tx,"t",&input).is_err());}
 assert_eq!(c.query_row("SELECT deleted FROM custom_tags WHERE id='t'",[],|r|r.get::<_,bool>(0)).unwrap(),false);
 input.impact_revision=Some(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap());let tx=c.transaction().unwrap();let first=change(&tx,"t",&input).ok().unwrap();assert_eq!(change(&tx,"t",&input).ok().unwrap(),first);tx.commit().unwrap();assert!(crate::smart_lists::read_one(&c,"active").ok().unwrap()["needs_repair"]==true);
 }
 #[test]fn undo_restores_mixed_memberships_after_reopen_and_rejects_identity_changes(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','A','A'),('b','B','B');").unwrap();let tag=create_tag(&db,Create{name:"Mixed".into(),work_ids:vec!["a".into()]}).ok().unwrap();let id=tag["id"].as_str().unwrap().to_owned();
 let apply=|db:&crate::db::Db,revision,req:&str,action:&str,ids:Vec<&str>|{let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let r=change(&tx,&id,&Edit{impact_revision:None,revision,request_id:req.into(),action:action.into(),name:None,work_ids:ids.into_iter().map(str::to_owned).collect()});if r.is_ok(){tx.commit().unwrap();}r};
 let added=apply(&db,1,"add","add",vec!["a","b","b"]).ok().unwrap();assert_eq!(added["batch"],json!({"processed":2,"changed":1,"unchanged":1,"failed":0}));drop(db);let db=crate::db::open(&path).unwrap();let first=apply(&db,2,"undo","undo",vec![]).ok().unwrap();assert_eq!(apply(&db,2,"undo","undo",vec![]).ok().unwrap(),first);assert_eq!(raw_members(&db.lock().unwrap(),&id).unwrap(),vec!["a"]);assert!(apply(&db,3,"again","undo",vec![]).is_err());
 apply(&db,3,"remove","remove",vec!["a","b"]).ok().unwrap();let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restore"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();apply(&restored,4,"restored-undo","undo",vec![]).ok().unwrap();assert_eq!(raw_members(&restored.lock().unwrap(),&id).unwrap(),vec!["a"]);
 db.lock().unwrap().execute("UPDATE works SET merged_into='b' WHERE id='a'",[]).unwrap();assert!(apply(&db,4,"changed","undo",vec![]).is_err());assert!(raw_members(&db.lock().unwrap(),&id).unwrap().is_empty());
 }
 #[test]fn nfc_upgrade_preserves_deleted_tag_identity_and_memberships(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();{
 let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','W','W'); INSERT INTO custom_tags(id,name,name_key,revision,deleted) VALUES('t','Ａ','a',7,1); INSERT INTO custom_tag_works VALUES('t','w'); PRAGMA user_version=18;").unwrap();}drop(db);
 let db=crate::db::open(&path).unwrap();{
 let c=db.lock().unwrap();let row=c.query_row("SELECT name,name_key,revision,deleted FROM custom_tags WHERE id='t'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,bool>(3)?))).unwrap();assert_eq!(row,("Ａ".into(),"ａ".into(),7,true));assert_eq!(c.query_row("SELECT work_id FROM custom_tag_works WHERE tag_id='t'",[],|r|r.get::<_,String>(0)).unwrap(),"w");}
 assert!(create_tag(&db,Create{name:"A".into(),work_ids:vec![]}).is_ok());assert!(create_tag(&db,Create{name:"Café".into(),work_ids:vec![]}).is_ok());assert!(create_tag(&db,Create{name:"CAFE\u{301}".into(),work_ids:vec![]}).is_err());assert_ne!(name_key("A B"),name_key("A  B"));
 assert_eq!(std::fs::read_dir(t.path().join("migration-backups")).unwrap().count(),1);
 }
 #[test]fn rename_preserves_links_and_receipts_and_rejects_conflicts(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','W','W');").unwrap();
 let tag=create_tag(&db,Create{name:"Old".into(),work_ids:vec!["w".into()]}).ok().unwrap();create_tag(&db,Create{name:"Taken".into(),work_ids:vec![]}).ok().unwrap();let id=tag["id"].as_str().unwrap();
 let apply=|revision,req:&str,name:&str|{let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let r=change(&tx,id,&Edit{impact_revision:None,revision,request_id:req.into(),action:"rename".into(),name:Some(name.into()),work_ids:vec![]});if r.is_ok(){tx.commit().unwrap();}r};
 assert!(apply(1,"bad"," TAKEN ").is_err());assert!(apply(1,"empty"," ").is_err());let first=apply(1,"rename","New").ok().unwrap();assert_eq!(apply(1,"rename","New").ok().unwrap(),first);assert!(apply(1,"stale","Other").is_err());assert!(apply(2,"rename","Other").is_err());
 let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT name FROM custom_tags WHERE id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),"New");assert_eq!(c.query_row("SELECT work_id FROM custom_tag_works WHERE tag_id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),"w");assert_eq!(c.query_row("SELECT count(*) FROM tag_requests",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn batch_conflicts_replays_and_deleted_members_survive_restore(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','A','A'),('b','B','B');").unwrap();let tag=create_tag(&db,Create{name:"Test".into(),work_ids:vec![]}).ok().unwrap();let id=tag["id"].as_str().unwrap();let apply=|revision,request:&str,action:&str,ids:Vec<&str>|{let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let r=change(&tx,id,&Edit{impact_revision:None,name:None,revision,request_id:request.into(),action:action.into(),work_ids:ids.into_iter().map(str::to_owned).collect()});if r.is_ok(){tx.commit().unwrap();}r};
  assert!(apply(1,"invalid","add",vec!["a","missing"]).is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  let first=apply(1,"add","add",vec!["a","b"]).ok().unwrap();assert_eq!(first["batch"],json!({"processed":2,"changed":2,"unchanged":0,"failed":0}));assert_eq!(apply(1,"add","add",vec!["a","b"]).ok().unwrap(),first);assert!(apply(1,"other","remove",vec!["a"]).is_err());assert!(apply(2,"add","remove",vec!["a"]).is_err());apply(2,"delete","delete",vec![]).ok().unwrap();assert!(apply(3,"hidden","add",vec!["a"]).is_err());apply(3,"restore","restore",vec![]).ok().unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  let backup=crate::backup::export(&db,t.path()).unwrap();let dest=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&dest.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();assert_eq!(c.query_row("SELECT revision||':'||deleted FROM custom_tags",[],|r|r.get::<_,String>(0)).unwrap(),"4:0");assert_eq!(c.query_row("SELECT count(*) FROM tag_requests",[],|r|r.get::<_,i64>(0)).unwrap(),3);
 }
 #[test]fn removal_clears_merged_ancestor_memberships(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','A','A'),('b','B','B');").unwrap();let tag=create_tag(&db,Create{name:"Test".into(),work_ids:vec!["a".into()]}).ok().unwrap();let mut c=db.lock().unwrap();c.execute("UPDATE works SET merged_into='b' WHERE id='a'",[]).unwrap();let tx=c.transaction().unwrap();change(&tx,tag["id"].as_str().unwrap(),&Edit{impact_revision:None,name:None,revision:1,request_id:"remove".into(),action:"remove".into(),work_ids:vec!["b".into()]}).ok().unwrap();assert_eq!(tx.query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),0);tx.commit().unwrap();}
 #[test]fn custom_tags_are_atomic_persistent_and_separate_from_provider_metadata(){
  let dir=tempfile::tempdir().unwrap();let path=dir.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
  db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,tags) VALUES('w','Work','Work','[\"Drama\"]')",[]).unwrap();
  let input=|name:&str,ids:Vec<&str>|Create{name:name.into(),work_ids:ids.into_iter().map(str::to_owned).collect()};
  assert!(create_tag(&db,input(" ",vec![])).is_err());
  assert!(create_tag(&db,input("Failed",vec!["w","missing"])).is_err());
  assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM custom_tags",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  assert!(create_tag(&db,input(" My favourites ",vec!["w","w"])).is_ok());
  assert!(create_tag(&db,input("my FAVOURITES",vec![])).is_err());
  assert!(create_tag(&db,input("Empty tag",vec![])).is_ok());
  drop(db);let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();
  assert_eq!(c.query_row("SELECT count(*) FROM custom_tags",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  assert_eq!(c.query_row("SELECT count(*) FROM custom_tag_works",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  assert_eq!(c.query_row("SELECT tags FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"[\"Drama\"]");
 }
}



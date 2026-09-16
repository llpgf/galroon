use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use crate::ApiError;
type R<T>=std::result::Result<T,ApiError>;
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS curated_lists(id TEXT PRIMARY KEY,name TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',pinned INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0,revision INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS list_entries(id TEXT PRIMARY KEY,list_id TEXT NOT NULL REFERENCES curated_lists(id),work_key TEXT NOT NULL,title TEXT NOT NULL,position INTEGER NOT NULL,notes TEXT NOT NULL DEFAULT '',preferred_release_id TEXT,added INTEGER NOT NULL,UNIQUE(list_id,work_key));
CREATE TABLE IF NOT EXISTS list_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL); CREATE TABLE IF NOT EXISTS list_entry_variants(id TEXT PRIMARY KEY,entry_id TEXT NOT NULL,original_json TEXT NOT NULL); CREATE INDEX IF NOT EXISTS list_entries_position ON list_entries(list_id,position,id); CREATE INDEX IF NOT EXISTS list_variants_entry ON list_entry_variants(entry_id);")}
#[derive(Deserialize,Serialize)]pub struct Member{pub work_key:String,pub title:String}
#[derive(Deserialize,Serialize)]#[serde(tag="action",rename_all="snake_case")]
pub enum Action{Create{name:String},Duplicate{name:String},Update{name:String,description:String,pinned:bool},Add{members:Vec<Member>},Remove{entry_id:String},Move{entry_id:String,position:usize},Annotate{entry_id:String,notes:String,preferred_release_id:Option<String>},Delete,Restore}
#[derive(Deserialize,Serialize)]pub struct Edit{pub request_id:String,pub revision:i64,pub edit:Action}
fn validate_text(s:&str,max:usize)->R<()>{if s.chars().count()>max||s.chars().any(|c|c.is_control()&&c!='\n'&&c!='\t'){return Err(ApiError("Invalid list text".into()));}Ok(())}
fn name(s:&str)->R<&str>{let s=s.trim();validate_text(s,80)?;if s.is_empty(){return Err(ApiError("List name required".into()));}Ok(s)}
pub(crate) fn identity(c:&Connection,key:&str)->R<String>{
 let Some(wid)=key.strip_prefix("local:") else{return Ok(key.to_owned());};let mut id=wid.to_owned();let mut seen=std::collections::HashSet::new();
 loop{if !seen.insert(id.clone()){return Err(ApiError("Cyclic work identity; repair before editing this list".into()));}
 let row:Option<(Option<String>,Option<String>)>=c.prepare_cached("SELECT merged_into,vndb_id FROM works WHERE id=?1")?.query_row([&id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 match row{Some((Some(next),_))=>id=next,Some((None,Some(vndb))) if !vndb.is_empty()=>return Ok(format!("vndb:{vndb}")),_=>return Ok(format!("local:{id}"))}
 }
}
pub fn merge_work(c:&Connection,source:&str,target:&str)->R<()>{
 let source_key=identity(c,&format!("local:{source}"))?;let target_key=identity(c,&format!("local:{target}"))?;
 let rows={let mut q=c.prepare("SELECT id,list_id,work_key,title,position,notes,preferred_release_id,added FROM list_entries ORDER BY list_id,position,id")?;let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"list_id":r.get::<_,String>(1)?,"work_key":r.get::<_,String>(2)?,"title":r.get::<_,String>(3)?,"position":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?,"preferred_release_id":r.get::<_,Option<String>>(6)?,"added":r.get::<_,i64>(7)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
 let mut groups=std::collections::BTreeMap::<String,Vec<Value>>::new();for row in rows{let key=identity(c,row["work_key"].as_str().unwrap())?;if key==source_key||key==target_key{groups.entry(row["list_id"].as_str().unwrap().into()).or_default().push(row);}}
 for (list,entries) in groups{let keep=entries[0]["id"].as_str().unwrap();for row in &entries{
  let eid=row["id"].as_str().unwrap();c.execute("INSERT INTO list_entry_variants(id,entry_id,original_json) VALUES(?1,?2,?3)",params![crate::db::id(),keep,row.to_string()])?;
  if eid!=keep{c.execute("UPDATE list_entry_variants SET entry_id=?2 WHERE entry_id=?1",params![eid,keep])?;c.execute("DELETE FROM list_entries WHERE id=?1",[eid])?;}
 }
 c.execute("UPDATE list_entries SET work_key=?2 WHERE id=?1",params![keep,format!("local:{target}")])?;order(c,&ordered(c,&list)?)?;c.execute("UPDATE curated_lists SET revision=revision+1 WHERE id=?1",[list])?;
 }Ok(())
}
fn ordered(c:&Connection,id:&str)->rusqlite::Result<Vec<String>>{let mut q=c.prepare("SELECT id FROM list_entries WHERE list_id=?1 ORDER BY position,id")?;let rows=q.query_map([id],|r|r.get(0))?.collect();rows}
fn order(c:&Connection,ids:&[String])->rusqlite::Result<()>{for (n,id) in ids.iter().enumerate(){c.execute("UPDATE list_entries SET position=?2 WHERE id=?1",params![id,n as i64])?;}Ok(())}
pub fn change(c:&Connection,id:&str,input:&Edit)->R<Value>{
 use sha2::{Digest,Sha256};
 if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((old,result))=c.query_row("SELECT digest,result FROM list_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=digest{return Err(ApiError("Request ID already used".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let current:Option<(i64,bool)>=c.query_row("SELECT revision,deleted FROM curated_lists WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 if matches!(input.edit,Action::Create{..}){if current.is_some()||input.revision!=0{return Err(ApiError("List already exists".into()));}}else{let Some((revision,deleted))=current else{return Err(ApiError("List not found".into()));};if revision!=input.revision{return Err(ApiError("List changed. Reload first.".into()));}if deleted&&!matches!(input.edit,Action::Restore){return Err(ApiError("Restore this list before editing".into()));}}
 let mut result=json!({"id":id,"revision":input.revision+1});
 match &input.edit{
 Action::Duplicate{name:n}=>{
 let new_id=crate::db::id();
 c.execute("INSERT INTO curated_lists(id,name,description) SELECT ?2,?3,description FROM curated_lists WHERE id=?1",params![id,new_id,name(n)?])?;
 // Copy all persisted entries, independent of the page loaded by the client.
 for entry in ordered(c,id)?{let new_entry=crate::db::id();c.execute("INSERT INTO list_entries(id,list_id,work_key,title,position,notes,preferred_release_id,added) SELECT ?2,?3,work_key,title,position,notes,preferred_release_id,added FROM list_entries WHERE id=?1",params![entry,new_entry,new_id])?;let variants={let mut q=c.prepare("SELECT original_json FROM list_entry_variants WHERE entry_id=?1")?;let rows=q.query_map([&entry],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};for variant in variants{c.execute("INSERT INTO list_entry_variants(id,entry_id,original_json) VALUES(?1,?2,?3)",params![crate::db::id(),new_entry,variant])?;}}
 result=json!({"id":new_id,"revision":1,"source_id":id,"source_revision":input.revision});
 },
 Action::Create{name:n}=>{c.execute("INSERT INTO curated_lists(id,name) VALUES(?1,?2)",params![id,name(n)?])?;},
 Action::Update{name:n,description,pinned}=>{validate_text(description,10000)?;c.execute("UPDATE curated_lists SET name=?2,description=?3,pinned=?4 WHERE id=?1",params![id,name(n)?,description,pinned])?;},
 Action::Add{members}=>{
 if members.is_empty()||members.len()>10000{return Err(ApiError("Select 1–10000 works".into()));}let mut added=0;let mut skipped=0;let mut position=ordered(c,id)?.len();
 let existing={let mut q=c.prepare("SELECT work_key FROM list_entries WHERE list_id=?1")?;let rows=q.query_map([id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
 let mut known=existing.iter().map(|key|identity(c,key)).collect::<R<std::collections::HashSet<_>>>()?;
 for member in members{
 let title=if let Some(wid)=member.work_key.strip_prefix("local:"){c.query_row("SELECT title FROM works WHERE id=?1 AND merged_into IS NULL",[wid],|r|r.get::<_,String>(0)).optional()?.ok_or_else(||ApiError("Work identity changed".into()))?}else if let Some(vid)=member.work_key.strip_prefix("vndb:v"){if vid.is_empty()||vid.len()>12||!vid.bytes().all(|b|b.is_ascii_digit())||vid.starts_with('0'){return Err(ApiError("Invalid VNDB work identity".into()));}validate_text(&member.title,1000)?;if member.title.trim().is_empty(){return Err(ApiError("Work title required".into()));}member.title.trim().to_owned()}else{return Err(ApiError("Invalid work reference".into()));};
 let canonical=identity(c,&member.work_key)?;if !known.insert(canonical){skipped+=1;continue;}
 let n=c.execute("INSERT OR IGNORE INTO list_entries(id,list_id,work_key,title,position,added) VALUES(?1,?2,?3,?4,?5,?6)",params![crate::db::id(),id,member.work_key,title,position as i64,crate::db::now()])?;if n==1{added+=1;position+=1;}else{skipped+=1;}
 }result["added"]=json!(added);result["skipped"]=json!(skipped);
 },
 Action::Remove{entry_id}=>{if c.execute("DELETE FROM list_entries WHERE id=?1 AND list_id=?2",params![entry_id,id])?!=1{return Err(ApiError("Entry changed".into()));}c.execute("DELETE FROM list_entry_variants WHERE entry_id=?1",[entry_id])?;order(c,&ordered(c,id)?)?;},
 Action::Move{entry_id,position}=>{let mut ids=ordered(c,id)?;if *position==0||*position>ids.len(){return Err(ApiError("Position is outside the list".into()));}let old=ids.iter().position(|v|v==entry_id).ok_or_else(||ApiError("Entry changed".into()))?;let moved=ids.remove(old);ids.insert(position-1,moved);order(c,&ids)?;},
 Action::Annotate{entry_id,notes,preferred_release_id}=>{validate_text(notes,10000)?;let key:String=c.query_row("SELECT work_key FROM list_entries WHERE id=?1 AND list_id=?2",params![entry_id,id],|r|r.get(0))?;
 if let Some(release)=preferred_release_id{let wid=key.strip_prefix("local:").ok_or_else(||ApiError("Save a local edition before selecting a preference".into()))?;if !c.query_row("SELECT EXISTS(SELECT 1 FROM work_releases WHERE work_id=?1 AND release_id=?2)",params![wid,release],|r|r.get::<_,bool>(0))?{return Err(ApiError("Edition does not belong to this work".into()));}}
 c.execute("UPDATE list_entries SET notes=?2,preferred_release_id=?3 WHERE id=?1",params![entry_id,notes,preferred_release_id])?;
 },
 Action::Delete|Action::Restore=>{c.execute("UPDATE curated_lists SET deleted=?2 WHERE id=?1",params![id,matches!(input.edit,Action::Delete)])?;}
 }
 if !matches!(input.edit,Action::Create{..}|Action::Duplicate{..}){c.execute("UPDATE curated_lists SET revision=revision+1 WHERE id=?1",[id])?;}
 c.execute("INSERT INTO list_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub async fn edit(axum::extract::State(a):axum::extract::State<crate::App>,h:axum::http::HeaderMap,axum::extract::Path(id):axum::extract::Path<String>,axum::Json(input):axum::Json<Edit>)->crate::Result<Value>{crate::auth(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=change(&tx,&id,&input)?;tx.commit()?;Ok(axum::Json(result))}


#[derive(Deserialize,Default)]pub struct Page{pub revision:Option<i64>,pub after:Option<i64>}
pub fn read_page(c:&Connection,id:&str,q:&Page)->R<Value>{
 let mut list=c.query_row("SELECT name,description,pinned,deleted,revision FROM curated_lists WHERE id=?1",[id],|r|Ok(json!({"id":id,"name":r.get::<_,String>(0)?,"description":r.get::<_,String>(1)?,"pinned":r.get::<_,bool>(2)?,"deleted":r.get::<_,bool>(3)?,"revision":r.get::<_,i64>(4)?})))?;
 let revision=list["revision"].as_i64().unwrap();if q.revision.is_some_and(|v|v!=revision)||q.after.is_some()&&q.revision.is_none(){return Err(ApiError("List changed. Reload from the first page.".into()));}if q.after.is_some_and(|v|v<0){return Err(ApiError("Invalid list cursor".into()));}
 let mut stmt=c.prepare("SELECT id,work_key,title,position,notes,preferred_release_id,added FROM list_entries WHERE list_id=?1 AND position>?2 ORDER BY position,id LIMIT 51")?;
 let mut entries=stmt.query_map(params![id,q.after.unwrap_or(-1)],|r|Ok(json!({"id":r.get::<_,String>(0)?,"work_key":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,"position":r.get::<_,i64>(3)?+1,"notes":r.get::<_,String>(4)?,"preferred_release_id":r.get::<_,Option<String>>(5)?,"added":r.get::<_,i64>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=entries.len()>50;entries.truncate(50);let next=if more{entries.last().and_then(|e|e["position"].as_i64()).map(|v|v-1)}else{None};
 for entry in &mut entries{
  let mut variants=c.prepare("SELECT original_json FROM list_entry_variants WHERE entry_id=?1 ORDER BY rowid")?;
  let saved=variants.query_map([entry["id"].as_str().unwrap()],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;entry["merge_variants"]=json!(saved.iter().map(|s|serde_json::from_str::<Value>(s).unwrap_or(Value::Null)).collect::<Vec<_>>());
  let key=entry["work_key"].as_str().unwrap();let local=if let Some(wid)=key.strip_prefix("local:"){Some(wid.to_owned())}else{None};
  let resolved=if let Some(wid)=local{c.query_row("SELECT id,title,merged_into FROM works WHERE id=?1",[wid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?))).optional()?}else{None};
  entry["reference_state"]=json!(if key.starts_with("vndb:"){"external"}else if resolved.as_ref().is_some_and(|v|v.2.is_some()){"merged"}else if resolved.is_some(){"local"}else{"missing"});
  if let Some((wid,title,merged))=resolved{entry["local_work_id"]=json!(wid);entry["current_title"]=json!(title);entry["merged_into"]=json!(merged);}
 }
 list["entries"]=json!(entries);list["next_after"]=json!(next);list["total"]=json!(c.query_row("SELECT count(*) FROM list_entries WHERE list_id=?1",[id],|r|r.get::<_,i64>(0))?);Ok(list)
}
#[derive(Deserialize,Default)]pub struct IndexQuery{#[serde(default)]pub include_deleted:bool,pub after:Option<String>}
pub async fn index(axum::extract::State(a):axum::extract::State<crate::App>,h:axum::http::HeaderMap,axum::extract::Query(q):axum::extract::Query<IndexQuery>)->crate::Result<Value>{
 let who=crate::access::authenticate(&a,&h)?;if q.include_deleted&&who.role=="web"{return Err(ApiError("Desktop access required".into()));}let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let mut stmt=c.prepare("SELECT id,name,description,pinned,deleted,revision,(SELECT count(*) FROM list_entries WHERE list_id=curated_lists.id) FROM curated_lists WHERE (deleted=0 OR ?1) AND id>?2 ORDER BY id LIMIT 51")?;
 let mut rows=stmt.query_map(params![q.include_deleted,q.after.unwrap_or_default()],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"description":r.get::<_,String>(2)?,"pinned":r.get::<_,bool>(3)?,"deleted":r.get::<_,bool>(4)?,"revision":r.get::<_,i64>(5)?,"count":r.get::<_,i64>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;let more=rows.len()>50;rows.truncate(50);for row in &mut rows{let mut covers=c.prepare("SELECT work_key FROM list_entries WHERE list_id=?1 ORDER BY position,id LIMIT 4")?;let keys=covers.query_map([row["id"].as_str().unwrap()],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;row["cover_keys"]=json!(keys);}let next=if more{rows.last().map(|v|v["id"].clone())}else{None};Ok(axum::Json(json!({"lists":rows,"next_after":next})))
}
pub async fn read(axum::extract::State(a):axum::extract::State<crate::App>,h:axum::http::HeaderMap,axum::extract::Path(id):axum::extract::Path<String>,axum::extract::Query(q):axum::extract::Query<Page>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let result=read_page(&c,&id,&q)?;if result["deleted"]==true&&who.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(axum::Json(result))}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn list_read_indexes_upgrade_idempotently_and_preserve_history_order(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
 c.execute_batch("DROP INDEX list_entries_position; DROP INDEX list_variants_entry; INSERT INTO curated_lists(id,name) VALUES('l','L'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('b','l','vndb:v2','B',1,1),('a','l','vndb:v1','A',0,1); INSERT INTO list_entry_variants VALUES('z','a','{\"notes\":\"first\"}'),('a','a','{\"notes\":\"second\"}');").unwrap();
 let before=read_page(&c,"l",&Page::default()).ok().unwrap();migrate(&c).unwrap();migrate(&c).unwrap();assert_eq!(read_page(&c,"l",&Page::default()).ok().unwrap(),before);
 assert_eq!(before["entries"][0]["merge_variants"][0]["notes"],"first");
 for(sql,index)in [("EXPLAIN QUERY PLAN SELECT id FROM list_entries WHERE list_id='l' AND position>0 ORDER BY position,id LIMIT 51","list_entries_position"),("EXPLAIN QUERY PLAN SELECT original_json FROM list_entry_variants WHERE entry_id='a' ORDER BY rowid","list_variants_entry")]{let mut q=c.prepare(sql).unwrap();let plans=q.query_map([],|r|r.get::<_,String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap().join(" ");assert!(plans.contains(index),"{plans}");assert!(!plans.contains("TEMP B-TREE"),"{plans}");}
 }
 #[test]fn removing_entry_is_list_scoped_and_preserves_work(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','W','W'); INSERT INTO curated_lists(id,name) VALUES('a','A'),('b','B'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('e','a','local:w','W',0,1),('f','a','vndb:v1','V',1,1); INSERT INTO list_entry_variants VALUES('v','e','{}');").unwrap();
 let input=Edit{request_id:"remove".into(),revision:1,edit:Action::Remove{entry_id:"e".into()}};{let tx=c.transaction().unwrap();assert!(change(&tx,"b",&input).is_err());}
 let result={let tx=c.transaction().unwrap();let result=change(&tx,"a",&input).ok().unwrap();tx.commit().unwrap();result};assert_eq!(change(&c,"a",&input).ok().unwrap(),result);assert_eq!(ordered(&c,"a").unwrap(),vec!["f"]);assert_eq!(c.query_row("SELECT position FROM list_entries WHERE id='f'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM list_entry_variants",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn work_merge_keeps_earliest_entry_and_all_private_variants_atomically(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('a','A','A','v1'),('b','B','B','v2'); INSERT INTO curated_lists(id,name) VALUES('l','L'); INSERT INTO list_entries(id,list_id,work_key,title,position,notes,preferred_release_id,added) VALUES('early','l','vndb:v1','A',0,'First note','ra',1),('later','l','local:b','B',1,'Second note','rb',2);").unwrap();crate::grouping::merge(&db,"a","b",1,1).unwrap();{
 let c=db.lock().unwrap();assert_eq!(ordered(&c,"l").unwrap(),vec!["early"]);let page=read_page(&c,"l",&Page::default()).ok().unwrap();assert_eq!(page["revision"],2);assert_eq!(page["entries"][0]["work_key"],"local:b");let variants=page["entries"][0]["merge_variants"].as_array().unwrap();assert_eq!(variants.len(),2);assert_eq!(variants[0]["notes"],"First note");assert_eq!(variants[1]["preferred_release_id"],"rb");}
 let backup=crate::backup::export(&db,t.path()).unwrap();let dest=crate::backup::restore_new(&backup,&t.path().join("restore"),None).unwrap();let restored=crate::db::open(&dest.join("library.sqlite")).unwrap();assert_eq!(restored.lock().unwrap().query_row("SELECT count(*) FROM list_entry_variants",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 let other=tempfile::tempdir().unwrap();let db=crate::db::open(&other.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('a','A','A'),('b','B','B'); INSERT INTO curated_lists(id,name) VALUES('l','L'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('e','l','local:a','A',0,1); CREATE TRIGGER fail_variants BEFORE INSERT ON list_entry_variants BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();assert!(crate::grouping::merge(&db,"a","b",1,1).is_err());let c=db.lock().unwrap();assert!(c.query_row("SELECT merged_into FROM works WHERE id='a'",[],|r|r.get::<_,Option<String>>(0)).unwrap().is_none());assert_eq!(c.query_row("SELECT work_key FROM list_entries WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"local:a");
 }
 #[test]fn adding_uses_provider_identity_and_merge_chains_without_name_matching(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute_batch("INSERT INTO curated_lists(id,name) VALUES('l','L'); INSERT INTO works(id,title,original_title,vndb_id,merged_into) VALUES('a','Same','Same','v17',NULL),('b','Same','Same',NULL,NULL),('old','Old','Old',NULL,'a'); INSERT INTO list_entries(id,list_id,work_key,title,position,notes,added) VALUES('e','l','local:old','Saved',0,'Do not lose',1);").unwrap();
 let input=Edit{request_id:"dedup".into(),revision:1,edit:Action::Add{members:vec![Member{work_key:"vndb:v17".into(),title:"Alias".into()},Member{work_key:"local:a".into(),title:"Ignored".into()},Member{work_key:"local:b".into(),title:"Ignored".into()}]}};
 let tx=c.transaction().unwrap();let result=change(&tx,"l",&input).ok().unwrap();assert_eq!(result["added"],1);assert_eq!(result["skipped"],2);tx.commit().unwrap();assert_eq!(ordered(&c,"l").unwrap().len(),2);assert_eq!(c.query_row("SELECT notes FROM list_entries WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"Do not lose");
 c.execute_batch("INSERT INTO curated_lists(id,name) VALUES('reverse','Reverse'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('external','reverse','vndb:v17','External',0,1);").unwrap();
 let tx=c.transaction().unwrap();let reverse=change(&tx,"reverse",&Edit{request_id:"reverse".into(),revision:1,edit:Action::Add{members:vec![Member{work_key:"local:a".into(),title:"Same".into()}]}}).ok().unwrap();assert_eq!(reverse["added"],0);assert_eq!(reverse["skipped"],1);tx.commit().unwrap();
 c.execute("UPDATE works SET merged_into='old' WHERE id='a'",[]).unwrap();assert!(identity(&c,"local:a").is_err());
 }
 #[test]fn duplicate_copies_all_pages_and_private_fields_once_without_editing_source(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute("INSERT INTO curated_lists(id,name,description,pinned,revision) VALUES('l','Original','Description',1,9)",[]).unwrap();for n in 0..55{c.execute("INSERT INTO list_entries(id,list_id,work_key,title,position,notes,preferred_release_id,added) VALUES(?1,'l',?2,'Saved',?3,'Private note','retained-edition',7)",params![format!("e{n}"),format!("local:w{n}"),n]).unwrap();}
 let input=Edit{request_id:"copy".into(),revision:9,edit:Action::Duplicate{name:"Copy".into()}};
 let result={let tx=c.transaction().unwrap();let result=change(&tx,"l",&input).ok().unwrap();tx.commit().unwrap();result};let copy=result["id"].as_str().unwrap();let replay=change(&c,"l",&input).ok().unwrap();assert_eq!(replay,result);
 assert_eq!(c.query_row("SELECT count(*) FROM curated_lists",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(c.query_row("SELECT revision FROM curated_lists WHERE id='l'",[],|r|r.get::<_,i64>(0)).unwrap(),9);assert_eq!(ordered(&c,copy).unwrap().len(),55);
 let count=c.query_row("SELECT count(*) FROM list_entries WHERE list_id=?1 AND notes='Private note' AND preferred_release_id='retained-edition' AND added=7",[copy],|r|r.get::<_,i64>(0)).unwrap();assert_eq!(count,55);assert_eq!(c.query_row("SELECT description||':'||pinned||':'||revision FROM curated_lists WHERE id=?1",[copy],|r|r.get::<_,String>(0)).unwrap(),"Description:0:1");
 c.execute("DELETE FROM list_entries WHERE list_id=?1",[copy]).unwrap();assert_eq!(ordered(&c,"l").unwrap().len(),55);let stale=Edit{request_id:"stale".into(),revision:8,edit:Action::Duplicate{name:"Bad".into()}};assert!(change(&c,"l",&stale).is_err());assert_eq!(c.query_row("SELECT count(*) FROM curated_lists",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn pages_are_bounded_and_revision_pinned_with_missing_references_retained(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute("INSERT INTO curated_lists(id,name) VALUES('l','Many')",[]).unwrap();for n in 0..55{c.execute("INSERT INTO list_entries(id,list_id,work_key,title,position,notes,added) VALUES(?1,'l',?2,'Saved title',?3,'Saved note',1)",params![format!("e{n}"),format!("local:missing{n}"),n]).unwrap();}
 let first=read_page(&c,"l",&Page::default()).ok().unwrap();assert_eq!(first["entries"].as_array().unwrap().len(),50);assert_eq!(first["next_after"],49);assert_eq!(first["entries"][0]["reference_state"],"missing");assert_eq!(first["entries"][0]["notes"],"Saved note");assert_eq!(first["total"],55);
 let second=read_page(&c,"l",&Page{revision:Some(1),after:Some(49)}).ok().unwrap();assert_eq!(second["entries"].as_array().unwrap().len(),5);assert_eq!(second["entries"][0]["position"],51);assert!(second["next_after"].is_null());assert!(read_page(&c,"l",&Page{revision:None,after:Some(49)}).is_err());c.execute("UPDATE curated_lists SET revision=2 WHERE id='l'",[]).unwrap();assert!(read_page(&c,"l",&Page{revision:Some(1),after:Some(49)}).is_err());
 }
 #[test]fn manual_list_roundtrip_is_atomic_ordered_private_and_replayable(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Local','Local');").unwrap();
 let apply=|db:&crate::db::Db,revision,req:&str,edit|{let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let r=change(&tx,"l",&Edit{request_id:req.into(),revision,edit});if r.is_ok(){tx.commit().unwrap();}r};
 apply(&db,0,"create",Action::Create{name:"Queue".into()}).ok().unwrap();
 let member=|key:&str|Member{work_key:key.into(),title:"External".into()};
 assert!(apply(&db,1,"invalid",Action::Add{members:vec![member("local:w"),member("bad")] }).is_err());assert!(ordered(&db.lock().unwrap(),"l").unwrap().is_empty());
 let add=||Action::Add{members:vec![member("local:w"),member("vndb:v17"),member("local:w")]};let first=apply(&db,1,"add",add()).ok().unwrap();assert_eq!(first["added"],2);assert_eq!(first["skipped"],1);assert_eq!(apply(&db,1,"add",add()).ok().unwrap(),first);assert!(apply(&db,1,"stale",Action::Delete).is_err());assert!(apply(&db,2,"add",Action::Delete).is_err());
 let ids=ordered(&db.lock().unwrap(),"l").unwrap();apply(&db,2,"move",Action::Move{entry_id:ids[1].clone(),position:1}).ok().unwrap();apply(&db,3,"note",Action::Annotate{entry_id:ids[1].clone(),notes:"Keep offline".into(),preferred_release_id:None}).ok().unwrap();assert!(apply(&db,4,"wrong-edition",Action::Annotate{entry_id:ids[1].clone(),notes:"Overwrite".into(),preferred_release_id:Some("r".into())}).is_err());
 apply(&db,4,"delete",Action::Delete).ok().unwrap();drop(db);let db=crate::db::open(&path).unwrap();assert_eq!(ordered(&db.lock().unwrap(),"l").unwrap(),vec![ids[1].clone(),ids[0].clone()]);apply(&db,5,"restore",Action::Restore).ok().unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 let backup=crate::backup::export(&db,t.path()).unwrap();let dest=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&dest.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();assert_eq!(c.query_row("SELECT notes FROM list_entries WHERE id=?1",[&ids[1]],|r|r.get::<_,String>(0)).unwrap(),"Keep offline");assert_eq!(c.query_row("SELECT revision FROM curated_lists WHERE id='l'",[],|r|r.get::<_,i64>(0)).unwrap(),6);assert_eq!(ordered(&c,"l").unwrap(),vec![ids[1].clone(),ids[0].clone()]);
 }
}

#[cfg(test)]mod capacity_tests{
 use super::*;
 #[test]fn sqlite_full_list_write_rolls_back_and_same_request_recovers(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
  let input=Edit{request_id:"capacity-add".into(),revision:1,edit:Action::Add{members:(1..=100).map(|i|Member{work_key:format!("vndb:v{i}"),title:"Generated capacity title ".repeat(35)}).collect()}};
  {let mut c=db.lock().unwrap();c.execute("INSERT INTO curated_lists(id,name) VALUES('capacity','Generated capacity')",[]).unwrap();let before=read_page(&c,"capacity",&Page::default()).ok().unwrap();
   let pages:i64=c.query_row("PRAGMA page_count",[],|r|r.get(0)).unwrap();c.pragma_update(None,"max_page_count",pages).unwrap();
   let error={let tx=c.transaction().unwrap();let result=change(&tx,"capacity",&input);assert!(result.is_err(),"Page budget must cause an actual SQLite capacity failure");result.err().unwrap().0};assert!(error.contains("full"),"Expected SQLITE_FULL: {error}");
   assert_eq!(read_page(&c,"capacity",&Page::default()).ok().unwrap(),before);assert_eq!(c.query_row("SELECT count(*) FROM list_requests WHERE id='capacity-add'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
   c.pragma_update(None,"max_page_count",pages+1024).unwrap();let tx=c.transaction().unwrap();let saved=change(&tx,"capacity",&input).ok().unwrap();tx.commit().unwrap();assert_eq!(saved["added"],100);assert_eq!(saved["revision"],2);assert_eq!(change(&c,"capacity",&input).ok().unwrap(),saved);
  }
  drop(db);let reopened=crate::db::open(&path).unwrap();let c=reopened.lock().unwrap();let page=read_page(&c,"capacity",&Page::default()).ok().unwrap();assert_eq!(page["total"],100);assert_eq!(page["revision"],2);assert_eq!(c.query_row("SELECT count(*) FROM list_requests WHERE id='capacity-add'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
}

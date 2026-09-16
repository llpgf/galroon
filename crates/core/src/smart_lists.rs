use crate::{ApiError,smart_rules::{Rule,Field}};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use axum::{extract::{Path,State,Query},http::HeaderMap,Json};
type R<T>=Result<T,ApiError>;
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS smart_lists(id TEXT PRIMARY KEY,name TEXT NOT NULL,description TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 0,revision INTEGER NOT NULL,deleted INTEGER NOT NULL DEFAULT 0,definition TEXT NOT NULL); CREATE TABLE IF NOT EXISTS smart_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL);")}
#[derive(Serialize,Deserialize,Clone)]#[serde(rename_all="snake_case")]pub enum Scope{Collection,StoredReferences}
#[derive(Serialize,Deserialize,Clone)]#[serde(rename_all="snake_case")]pub enum Sort{Title,Year}
#[derive(Serialize,Deserialize,Clone)]#[serde(deny_unknown_fields)]pub struct Definition{pub schema_version:u8,pub scope:Scope,pub sort:Sort,#[serde(default)]pub descending:bool,pub rule:Rule}
#[derive(Serialize,Deserialize)]#[serde(tag="action",rename_all="snake_case",deny_unknown_fields)]pub enum Action{Save{name:String,description:String,pinned:bool,definition:Definition},Delete,Restore}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct Edit{pub revision:i64,pub request_id:String,pub edit:Action}
fn dependencies(rule:&Rule,ids:&mut std::collections::BTreeSet<String>){match rule{Rule::Group{children,..}=>for child in children{dependencies(child,ids)},Rule::Field{predicate} if matches!(predicate.field,Field::PersonalTags|Field::RelatedPersonalTags)=>ids.extend(predicate.values.iter().cloned()),_=>{}}}
pub(crate) fn dependency_counts(c:&Connection)->R<std::collections::BTreeMap<String,usize>>{
 let mut counts=std::collections::BTreeMap::new();let mut q=c.prepare("SELECT definition FROM smart_lists WHERE deleted=0")?;
 for row in q.query_map([],|r|r.get::<_,String>(0))?{let definition:Definition=serde_json::from_str(&row?).map_err(|e|ApiError(e.to_string()))?;let mut ids=std::collections::BTreeSet::new();dependencies(&definition.rule,&mut ids);for id in ids{*counts.entry(id).or_insert(0)+=1;}}
 Ok(counts)
}
pub fn broken_dependencies(c:&Connection,definition:&Definition)->R<Vec<String>>{let mut ids=std::collections::BTreeSet::new();dependencies(&definition.rule,&mut ids);let mut broken=Vec::new();for id in ids{let active:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM custom_tags WHERE id=?1 AND deleted=0)",[&id],|r|r.get(0))?;if !active{broken.push(id);}}Ok(broken)}
fn text(value:&str,max:usize)->R<()>{if value.len()>max||value.chars().any(|c|c.is_control()&&c!='\n'&&c!='\t'){return Err(ApiError("Invalid list text".into()));}Ok(())}
pub fn change(c:&Connection,id:&str,input:&Edit)->R<Value>{
 if id.is_empty()||id.len()>128||input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("List and request IDs required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((saved,result))=c.query_row("SELECT digest,result FROM smart_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if saved!=digest{return Err(ApiError("Request ID already used".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let current:Option<(i64,bool)>=c.query_row("SELECT revision,deleted FROM smart_lists WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 match current{Some((revision,_)) if revision!=input.revision=>return Err(ApiError("Smart list changed. Reload before editing.".into())),None if input.revision!=0||!matches!(input.edit,Action::Save{..})=>return Err(ApiError("Smart list not found".into())),_=>{}}
 if current.is_some_and(|(_,deleted)|deleted)&&!matches!(input.edit,Action::Restore){return Err(ApiError("Restore the smart list before editing".into()));}
 match &input.edit{
 Action::Save{name,description,pinned,definition}=>{
 text(name,320)?;text(description,10000)?;if name.trim().is_empty(){return Err(ApiError("List name required".into()));}if definition.schema_version!=1{return Err(ApiError("Unsupported rule version".into()));}crate::smart_rules::validate(&definition.rule).map_err(ApiError)?;
 if !broken_dependencies(c,definition)?.is_empty(){return Err(ApiError("Repair missing or deleted tag conditions before saving".into()));}
 c.execute("INSERT INTO smart_lists(id,name,description,pinned,revision,definition) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,pinned=excluded.pinned,revision=excluded.revision,definition=excluded.definition",params![id,name.trim(),description,pinned,input.revision+1,serde_json::to_string(definition).map_err(|e|ApiError(e.to_string()))?])?;
 },
 Action::Delete=>{c.execute("UPDATE smart_lists SET deleted=1,revision=revision+1 WHERE id=?1",[id])?;},
 Action::Restore=>{c.execute("UPDATE smart_lists SET deleted=0,revision=revision+1 WHERE id=?1",[id])?;}
 }
 let result=json!({"id":id,"revision":input.revision+1});c.execute("INSERT INTO smart_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub fn read_one(c:&Connection,id:&str)->R<Value>{let mut row=c.query_row("SELECT name,description,pinned,revision,deleted,definition FROM smart_lists WHERE id=?1",[id],|r|Ok(json!({"id":id,"kind":"smart","name":r.get::<_,String>(0)?,"description":r.get::<_,String>(1)?,"pinned":r.get::<_,bool>(2)?,"revision":r.get::<_,i64>(3)?,"deleted":r.get::<_,bool>(4)?,"definition":r.get::<_,String>(5)?})))?;let definition:Definition=serde_json::from_str(row["definition"].as_str().unwrap()).map_err(|e|ApiError(e.to_string()))?;let broken=broken_dependencies(c,&definition)?;row["needs_repair"]=json!(!broken.is_empty());row["broken_tag_ids"]=json!(broken);row["definition"]=serde_json::to_value(definition).map_err(|e|ApiError(e.to_string()))?;Ok(row)}
pub async fn edit(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Edit>)->crate::Result<Value>{crate::auth(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=change(&tx,&id,&input)?;tx.commit()?;Ok(Json(result))}
pub async fn read(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let row=read_one(&c,&id)?;if who.role=="web"&&row["deleted"]==true{return Err(ApiError("Desktop access required".into()));}Ok(Json(row))}
#[derive(Deserialize,Default)]pub struct Index{#[serde(default)]pub include_deleted:bool,pub after:Option<String>}
pub async fn index(State(a):State<crate::App>,h:HeaderMap,Query(q):Query<Index>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"&&q.include_deleted{return Err(ApiError("Desktop access required".into()));}let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let mut statement=c.prepare("SELECT id,name,pinned,revision,deleted FROM smart_lists WHERE (deleted=0 OR ?1) AND id>?2 ORDER BY id LIMIT 51")?;let mut rows=statement.query_map(params![q.include_deleted,q.after.unwrap_or_default()],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"kind":"smart","pinned":r.get::<_,bool>(2)?,"revision":r.get::<_,i64>(3)?,"deleted":r.get::<_,bool>(4)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;let more=rows.len()>50;rows.truncate(50);let next=if more{rows.last().map(|v|v["id"].clone())}else{None};Ok(Json(json!({"lists":rows,"next_after":next})))}

#[cfg(test)]mod tests{
 use super::*;
 fn save(revision:i64,request:&str)->Edit{Edit{revision,request_id:request.into(),edit:Action::Save{name:"Chinese backlog".into(),description:"Private rules".into(),pinned:true,definition:Definition{schema_version:1,scope:Scope::Collection,sort:Sort::Title,descending:false,rule:Rule::Field{predicate:crate::smart_rules::Predicate{field:Field::PersonalTags,mode:crate::smart_rules::SetMode::Any,values:vec!["tag".into()],exclude:false,include_unknown:false}}}}}}
 fn apply(db:&crate::db::Db,input:&Edit)->R<Value>{let mut c=db.lock().unwrap();let tx=c.transaction()?;let result=change(&tx,"s",input)?;tx.commit()?;Ok(result)}
 #[test]fn persisted_rules_receipts_dependencies_and_backup_restore(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute("INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag')",[]).unwrap();let input=save(0,"create");let first=apply(&db,&input).ok().unwrap();assert_eq!(apply(&db,&input).ok().unwrap(),first);assert!(apply(&db,&save(0,"stale")).is_err());assert!(apply(&db,&save(1,"create")).is_err());
 {let c=db.lock().unwrap();let before=read_one(&c,"s").ok().unwrap();assert_eq!(before["needs_repair"],false);c.execute("UPDATE custom_tags SET deleted=1 WHERE id='tag'",[]).unwrap();let after=read_one(&c,"s").ok().unwrap();assert_eq!(after["needs_repair"],true);assert_eq!(after["definition"],before["definition"]);}
 assert!(apply(&db,&save(1,"invalid-dependency")).is_err());apply(&db,&Edit{revision:1,request_id:"delete".into(),edit:Action::Delete}).ok().unwrap();let backup=crate::backup::export(&db,t.path()).unwrap();let destination=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&destination.join("library.sqlite")).unwrap();assert_eq!(apply(&restored,&input).ok().unwrap(),first);apply(&restored,&Edit{revision:2,request_id:"restore".into(),edit:Action::Restore}).ok().unwrap();let row=read_one(&restored.lock().unwrap(),"s").ok().unwrap();assert_eq!(row["revision"],3);assert_eq!(row["needs_repair"],true);assert_eq!(row["definition"]["rule"]["predicate"]["values"],json!(["tag"]));
 }
 #[test]fn receipt_failure_rolls_back_and_invalid_definitions_never_save(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag'); CREATE TRIGGER fail_receipt BEFORE INSERT ON smart_requests BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();assert!(apply(&db,&save(0,"failure")).is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM smart_lists",[],|r|r.get::<_,i64>(0)).unwrap(),0);db.lock().unwrap().execute_batch("DROP TRIGGER fail_receipt").unwrap();let mut input=save(0,"invalid");if let Action::Save{definition,..}=&mut input.edit{definition.schema_version=9;}assert!(apply(&db,&input).is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM smart_requests",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn schema22_upgrade_preserves_manual_lists_and_private_tags(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute_batch("INSERT INTO curated_lists(id,name) VALUES('l','Manual'); INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag'); DROP TABLE smart_lists; DROP TABLE smart_requests; PRAGMA user_version=22;").unwrap();drop(db);let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);assert_eq!(c.query_row("SELECT name FROM curated_lists WHERE id='l'",[],|r|r.get::<_,String>(0)).unwrap(),"Manual");assert_eq!(c.query_row("SELECT name FROM custom_tags WHERE id='tag'",[],|r|r.get::<_,String>(0)).unwrap(),"Tag");assert_eq!(std::fs::read_dir(t.path().join("migration-backups")).unwrap().count(),1);
 }
}

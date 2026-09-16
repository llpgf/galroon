//! Personal assignments use provider identity, never names. No provider writes.
use axum::{extract::{State,Path},http::HeaderMap,Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use crate::{ApiError,smart_rules::{Field,KnownSet}};
type R<T>=Result<T,ApiError>;
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS custom_tag_entities(tag_id TEXT NOT NULL REFERENCES custom_tags(id),entity_id TEXT NOT NULL,PRIMARY KEY(tag_id,entity_id)); CREATE INDEX IF NOT EXISTS tag_entity_lookup ON custom_tag_entities(entity_id,tag_id);")}
fn valid(id:&str)->bool{['s','c','p'].iter().any(|prefix|crate::exploration::valid_id(id,*prefix))}
#[derive(Serialize,Deserialize)]pub struct Edit{tag_id:String,revision:i64,assigned:bool,request_id:String}
fn change(c:&Connection,entity:&str,input:&Edit)->R<Value>{
 use sha2::{Digest,Sha256};
 if !valid(entity){return Err(ApiError("A person, character or company identity is required".into()));}
 if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&("entity-tag",entity,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((saved,result))=c.query_row("SELECT digest,result FROM tag_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if saved!=digest{return Err(ApiError("Request ID already used for another edit".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let(revision,deleted):(i64,bool)=c.query_row("SELECT revision,deleted FROM custom_tags WHERE id=?1",[&input.tag_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
 if revision!=input.revision||deleted{return Err(ApiError("Tag changed or deleted. Reload before editing.".into()));}
 if input.assigned{c.execute("INSERT OR IGNORE INTO custom_tag_entities(tag_id,entity_id) VALUES(?1,?2)",params![input.tag_id,entity])?;}else{c.execute("DELETE FROM custom_tag_entities WHERE tag_id=?1 AND entity_id=?2",params![input.tag_id,entity])?;}
 c.execute("UPDATE custom_tags SET revision=revision+1 WHERE id=?1",[&input.tag_id])?;
 let result=json!({"tag_id":input.tag_id,"entity_id":entity,"revision":revision+1,"assigned":input.assigned});
 c.execute("INSERT INTO tag_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub async fn edit(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Edit>)->crate::Result<Value>{crate::auth(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=change(&tx,&id,&input)?;tx.commit()?;Ok(Json(result))}
pub async fn read(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{crate::access::authenticate(&a,&h)?;if !valid(&id){return Err(ApiError("Invalid entity identity".into()));}let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let mut q=c.prepare("SELECT t.id,t.name,t.revision FROM custom_tags t JOIN custom_tag_entities e ON e.tag_id=t.id WHERE e.entity_id=?1 AND t.deleted=0 ORDER BY t.name_key,t.id")?;let rows=q.query_map([id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(Json(json!(rows)))}
fn explain(c:&Connection,work:&str,now:i64)->R<Value>{
 let vndb:Option<String>=c.query_row("SELECT vndb_id FROM works WHERE id=?1 AND merged_into IS NULL",[work],|r|r.get(0))?;
 explain_provider(c,vndb.as_deref(),now)
}
fn explain_provider(c:&Connection,vndb:Option<&str>,now:i64)->R<Value>{
 let projection=match vndb{Some(id)=>crate::smart_relations::load_local(c,id,now)?,None=>Default::default()};
 let facts=related(c,&projection.fields)?;let mut tags=std::collections::BTreeMap::<String,Value>::new();
 let mut q=c.prepare_cached("SELECT t.id,t.name FROM custom_tags t JOIN custom_tag_entities e ON e.tag_id=t.id WHERE e.entity_id=?1 AND t.deleted=0 ORDER BY t.id")?;
 for(field,key,kind)in [(Field::Brands,"brands","company"),(Field::People,"people","person"),(Field::Characters,"characters","character")]{if let Some(values)=projection.fields.get(&field){for entity in &values.values{let name=projection.options.get(key).and_then(|names|names.get(entity)).cloned().unwrap_or_else(||entity.clone());for row in q.query_map([entity],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?{let(id,tag)=row?;let entry=tags.entry(id.clone()).or_insert_with(||json!({"id":id,"name":tag,"reasons":[]}));entry["reasons"].as_array_mut().unwrap().push(json!({"entity_id":entity,"name":name,"kind":kind}));}}}}
 Ok(json!({"tags":tags.into_values().collect::<Vec<_>>(),"complete":facts.complete}))
}
pub async fn work_reasons(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{crate::access::authenticate(&a,&h)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(explain(&c,&id,crate::db::now())?))}
pub async fn reference_reasons(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{crate::access::authenticate(&a,&h)?;if !crate::exploration::valid_id(&id,'v'){return Err(ApiError("Invalid work identity".into()));}let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(explain_provider(&c,Some(&id),crate::db::now())?))}
// Only the already spoiler-filtered, direct work relations may contribute.
pub(crate) fn related(c:&Connection,fields:&std::collections::BTreeMap<Field,KnownSet>)->R<KnownSet>{
 let mut result=KnownSet{values:Default::default(),complete:true};
 let mut q=c.prepare_cached("SELECT t.id FROM custom_tags t JOIN custom_tag_entities e ON e.tag_id=t.id WHERE e.entity_id=?1 AND t.deleted=0")?;
 for field in [Field::Brands,Field::People,Field::Characters]{match fields.get(&field){Some(known)=>{result.complete&=known.complete;for id in &known.values{for row in q.query_map([id],|r|r.get::<_,String>(0))?{result.values.insert(row?);}}},None=>result.complete=false}}
 Ok(result)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn external_cached_reasons_never_insert_collection_members(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Tag','tag'); INSERT INTO custom_tag_entities VALUES('t','p1');").unwrap();
 let value=json!({"fetched_at":100,"vn":{"developers":[{"id":"p1","name":"Company"}],"staff":[],"va":[]},"characters":[],"more":false});c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v99.1',?1)",[value.to_string()]).unwrap();let before=c.total_changes();
 let reasons=explain_provider(&c,Some("v99"),101).ok().unwrap();assert_eq!(reasons["tags"][0]["reasons"][0]["entity_id"],"p1");assert_eq!(reasons["complete"],true);let missing=explain_provider(&c,Some("v98"),101).ok().unwrap();assert_eq!(missing,json!({"tags":[],"complete":false}));assert_eq!(c.total_changes(),before);assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn assignments_survive_reopen_backup_and_reject_stale_replays(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Favourite','favourite')",[]).unwrap();
 let input=Edit{tag_id:"t".into(),revision:1,assigned:true,request_id:"assign".into()};
 let result={let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let result=change(&tx,"s1",&input).ok().unwrap();tx.commit().unwrap();result};drop(db);let db=crate::db::open(&path).unwrap();
 {let c=db.lock().unwrap();assert_eq!(change(&c,"s1",&input).ok().unwrap(),result);assert!(change(&c,"c1",&input).is_err());assert!(change(&c,"s1",&Edit{request_id:"stale".into(),..input}).is_err());}
 let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();assert_eq!(c.query_row("SELECT entity_id FROM custom_tag_entities",[],|r|r.get::<_,String>(0)).unwrap(),"s1");
 let mut fields=std::collections::BTreeMap::new();for field in [Field::Brands,Field::People,Field::Characters]{fields.insert(field,KnownSet{values:Default::default(),complete:true});}fields.get_mut(&Field::People).unwrap().values.insert("s1".into());let related=super::related(&c,&fields).ok().unwrap();assert!(related.complete&&related.values.contains("t"));
 c.execute("UPDATE custom_tags SET deleted=1 WHERE id='t'",[]).unwrap();assert!(super::related(&c,&fields).ok().unwrap().values.is_empty());c.execute("UPDATE custom_tags SET deleted=0 WHERE id='t'",[]).unwrap();assert!(super::related(&c,&fields).ok().unwrap().values.contains("t"));
 }
 #[test]fn independent_related_tag_rule_matrix_and_catalog_invalidation(){
 use crate::smart_rules::{Rule,Predicate,SetMode};
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();let now=crate::db::now();
 c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Chosen','chosen'),('u','Other','other'); INSERT INTO custom_tag_entities VALUES('t','p1'),('t','s1'),('t','c1'),('u','s1');").unwrap();
 // Expected identities are specified below, independently from the projection algorithm.
 for n in 1..=9{let vn=format!("v{n}");c.execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES(?1,?1,?1,?2)",params![format!("w{n}"),vn]).unwrap();if n==7{continue;}
 let mut value=json!({"fetched_at":now,"vn":{"developers":[],"staff":[],"va":[],"relations":[{"id":"v1"}]},"characters":[],"more":false});
 if n==1||n==4{value["vn"]["developers"]=json!([{"id":"p1","name":"Studio"}]);}
 if n==2||n==4||n==5{value["vn"]["staff"]=json!([{"id":"s1","name":"Alias A","role":"scenario"},{"id":"s1","name":"Alias B","role":"art"}]);}
 if n==3||n==4||n==6{value["characters"]=json!([{"id":"c1","name":"Character","vns":[{"id":vn,"spoiler":if n==6{1}else{0}}]}]);value["vn"]["va"]=json!([{"character":{"id":"c1"},"staff":{"id":"s1","name":"Voice"}}]);}
 if n==5||n==9{value["fetched_at"]=json!(now-90000);}
 c.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",params![format!("exploration.v4.work.{vn}.1"),value.to_string()]).unwrap();
 }
 let check=|mode:SetMode,values:Vec<String>,include_unknown:bool,expected:Vec<&str>,unknown:u64|{
 let definition=crate::smart_lists::Definition{schema_version:1,scope:crate::smart_lists::Scope::Collection,sort:crate::smart_lists::Sort::Title,descending:false,rule:Rule::Field{predicate:Predicate{field:Field::RelatedPersonalTags,mode,values,exclude:false,include_unknown}}};
 let result=crate::smart_snapshots::preview_definition(&c,&definition,&Default::default()).ok().unwrap();let keys=result["sample"].as_array().unwrap().iter().map(|v|v["work_key"].as_str().unwrap()).collect::<Vec<_>>();assert_eq!(keys,expected);assert_eq!(result["unknown_count"],unknown);
 };
 let reasons=explain(&c,"w4",now).ok().unwrap();assert_eq!(reasons["tags"].as_array().unwrap().len(),2);assert_eq!(reasons["tags"][0]["reasons"].as_array().unwrap().len(),3);assert_eq!(reasons["complete"],true);
 let hidden=explain(&c,"w6",now).ok().unwrap();assert!(hidden["tags"].as_array().unwrap().is_empty());assert_eq!(hidden["complete"],false);assert!(!hidden.to_string().contains("c1"));assert!(!hidden.to_string().contains("s1"));
 check(SetMode::Any,vec!["t".into()],false,vec!["local:w1","local:w2","local:w3","local:w4","local:w5"],3);
 check(SetMode::None,vec!["t".into()],false,vec!["local:w8"],3);
 check(SetMode::All,vec!["t".into(),"u".into()],false,vec!["local:w2","local:w3","local:w4","local:w5"],3);
 check(SetMode::Any,vec!["t".into()],true,vec!["local:w1","local:w2","local:w3","local:w4","local:w5","local:w6","local:w7","local:w9"],0);
 let before:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();
 change(&c,"p1",&Edit{tag_id:"t".into(),revision:1,assigned:false,request_id:"remove-studio".into()}).ok().unwrap();
 assert!(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap()>before);
 check(SetMode::Any,vec!["t".into()],false,vec!["local:w2","local:w3","local:w4","local:w5"],3);
 }
 #[test]fn hidden_or_missing_relations_cannot_supply_tags_or_prove_absence(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('t','Tag','tag'); INSERT INTO custom_tag_entities VALUES('t','c1'),('t','s1');").unwrap();
 let value=json!({"fetched_at":100,"vn":{"developers":[],"staff":[],"va":[{"character":{"id":"c1"},"staff":{"id":"s1"}}]},"characters":[{"id":"c1","name":"Hidden","vns":[{"id":"v1","spoiler":1}]}],"more":false});
 let projection=crate::smart_relations::project(&value,"v1",101);let facts=related(&c,&projection.fields).ok().unwrap();assert!(facts.values.is_empty());assert!(!facts.complete);assert!(!related(&c,&Default::default()).ok().unwrap().complete);
 assert!(change(&c,"v1",&Edit{tag_id:"t".into(),revision:1,assigned:true,request_id:"invalid".into()}).is_err());
 }
}

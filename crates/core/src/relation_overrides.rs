//! Local relationship decisions remain separate from provider snapshots.
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::collections::BTreeSet;
use crate::ApiError;
type R<T>=Result<T,ApiError>;
use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
fn authorize(a:&crate::App,h:&HeaderMap)->R<()>{
 let identity=crate::access::authenticate(a,h)?;
 if !matches!(identity.role.as_str(),"owner"|"desktop"){return Err(ApiError("Local owner access required".into()));}Ok(())
}
#[derive(Deserialize)]pub struct HistoryPage{before:Option<i64>}
pub async fn get(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{
 authorize(&a,&h)?;validate(&id,&Hidden::default())?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let(revision,hidden)=read(&c,&id)?;let labels=labels(&c,&id,&hidden)?;Ok(Json(json!({"vndb_id":id,"revision":revision,"hidden":hidden,"labels":labels})))
}
// Only names for existing manual decisions, never the entire unfiltered provider payload.
fn labels(c:&Connection,id:&str,hidden:&Hidden)->R<Value>{
 let raw:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[format!("exploration.v4.work.{id}.1")],|r|r.get(0)).optional()?;
 let value=raw.and_then(|raw|serde_json::from_str::<Value>(&raw).ok()).unwrap_or(Value::Null);let mut labels=serde_json::Map::new();
 let mut add=|kind:&str,rows:Option<&Vec<Value>>,ids:&BTreeSet<String>,field:&str|{if let Some(rows)=rows{for row in rows{if let(Some(id),Some(name))=(row["id"].as_str(),row[field].as_str()){if ids.contains(id)&&!name.trim().is_empty(){labels.entry(format!("{kind}:{id}")).or_insert_with(||json!(name.chars().take(300).collect::<String>()));}}}}};
 add("people",value["vn"]["staff"].as_array(),&hidden.people,"name");
 let voices=value["vn"]["va"].as_array().map(|rows|rows.iter().map(|v|v["staff"].clone()).collect::<Vec<_>>());add("people",voices.as_ref(),&hidden.people,"name");
 add("characters",value["characters"].as_array(),&hidden.characters,"name");add("companies",value["vn"]["developers"].as_array(),&hidden.companies,"name");add("works",value["vn"]["relations"].as_array(),&hidden.works,"title");Ok(Value::Object(labels))
}
pub async fn edit(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Edit>)->crate::Result<Value>{
 authorize(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=change(&tx,&id,&input)?;tx.commit()?;Ok(Json(result))
}
pub async fn history(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<HistoryPage>)->crate::Result<Value>{
 authorize(&a,&h)?;validate(&id,&Hidden::default())?;if page.before.is_some_and(|v|v<=0){return Err(ApiError("Invalid history cursor".into()));}
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
 let mut q=c.prepare("SELECT revision,created,before_json,after_json FROM relation_override_history WHERE vndb_id=?1 AND (?2 IS NULL OR revision<?2) ORDER BY revision DESC LIMIT 51")?;
 let rows=q.query_map(params![id,page.before],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=rows.len()>50;let mut items=Vec::new();
 for(revision,created,before,after)in rows.into_iter().take(50){items.push(json!({"revision":revision,"created":created,"before":serde_json::from_str::<Hidden>(&before).map_err(|e|ApiError(e.to_string()))?,"after":serde_json::from_str::<Hidden>(&after).map_err(|e|ApiError(e.to_string()))?}));}
 let next=if more{items.last().map(|v|v["revision"].clone())}else{None};Ok(Json(json!({"items":items,"next":next})))
}
#[derive(Default,Serialize,Deserialize,Clone,Debug,PartialEq,Eq)]#[serde(default,deny_unknown_fields)]pub struct Hidden{pub people:BTreeSet<String>,pub characters:BTreeSet<String>,pub companies:BTreeSet<String>,pub works:BTreeSet<String>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct Edit{pub revision:i64,pub request_id:String,pub hidden:Hidden}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS relation_overrides(vndb_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,hidden_json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS relation_override_history(vndb_id TEXT NOT NULL,revision INTEGER NOT NULL,created INTEGER NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,PRIMARY KEY(vndb_id,revision)); CREATE TABLE IF NOT EXISTS relation_override_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL);")}
pub fn read(c:&Connection,id:&str)->R<(i64,Hidden)>{let row:Option<(i64,String)>=c.prepare_cached("SELECT revision,hidden_json FROM relation_overrides WHERE vndb_id=?1")?.query_row([id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;match row{Some((revision,raw))=>Ok((revision,serde_json::from_str(&raw).map_err(|e|ApiError(e.to_string()))?)),None=>Ok((0,Hidden::default()))}}
fn validate(id:&str,hidden:&Hidden)->R<()>{if !crate::exploration::valid_id(id,'v'){return Err(ApiError("Invalid work identity".into()));}let mut count=0;for(values,prefix)in [(&hidden.people,'s'),(&hidden.characters,'c'),(&hidden.companies,'p'),(&hidden.works,'v')]{count+=values.len();if values.iter().any(|value|!crate::exploration::valid_id(value,prefix)){return Err(ApiError("Invalid relationship identity".into()));}}if count>200{return Err(ApiError("At most 200 hidden relationships per work".into()));}Ok(())}
// Caller must use a transaction so decision, history and receipt commit together.
pub fn change(c:&Connection,id:&str,input:&Edit)->R<Value>{
 use sha2::{Digest,Sha256};validate(id,&input.hidden)?;if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((old,result))=c.query_row("SELECT digest,result FROM relation_override_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=digest{return Err(ApiError("Request ID already used for another correction".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let (revision,before)=read(c,id)?;if revision!=input.revision{return Err(ApiError("Relationship decisions changed. Reload first.".into()));}
 let before=serde_json::to_string(&before).map_err(|e|ApiError(e.to_string()))?;let after=serde_json::to_string(&input.hidden).map_err(|e|ApiError(e.to_string()))?;
 c.execute("INSERT INTO relation_overrides(vndb_id,revision,hidden_json) VALUES(?1,?2,?3) ON CONFLICT(vndb_id) DO UPDATE SET revision=excluded.revision,hidden_json=excluded.hidden_json",params![id,revision+1,after])?;
 c.execute("INSERT INTO relation_override_history(vndb_id,revision,created,before_json,after_json) VALUES(?1,?2,?3,?4,?5)",params![id,revision+1,crate::db::now(),before,after])?;
 c.execute("UPDATE relation_correction_state SET revision=revision+1 WHERE singleton=1",[])?;let result=json!({"vndb_id":id,"revision":revision+1});c.execute("INSERT INTO relation_override_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub fn project(value:&mut Value,hidden:&Hidden){
 if let Some(rows)=value.get_mut("vn").and_then(|vn|vn.get_mut("developers")).and_then(Value::as_array_mut){rows.retain(|row|!row["id"].as_str().is_some_and(|id|hidden.companies.contains(id)));}
 if let Some(rows)=value.get_mut("vn").and_then(|vn|vn.get_mut("staff")).and_then(Value::as_array_mut){rows.retain(|row|!row["id"].as_str().is_some_and(|id|hidden.people.contains(id)));}
 if let Some(rows)=value.get_mut("characters").and_then(Value::as_array_mut){rows.retain(|row|!row["id"].as_str().is_some_and(|id|hidden.characters.contains(id)));}
 if let Some(rows)=value.get_mut("vn").and_then(|vn|vn.get_mut("va")).and_then(Value::as_array_mut){rows.retain(|row|!row["staff"]["id"].as_str().is_some_and(|id|hidden.people.contains(id))&&!row["character"]["id"].as_str().is_some_and(|id|hidden.characters.contains(id)));}
 if let Some(rows)=value.get_mut("vn").and_then(|vn|vn.get_mut("relations")).and_then(Value::as_array_mut){rows.retain(|row|!row["id"].as_str().is_some_and(|id|hidden.works.contains(id)));}
}
/// Apply current local decisions to a copy of a provider profile page.
/// Keep provider pagination unchanged: filtering a page does not exhaust later pages.
pub fn project_profile(c:&Connection,value:&mut Value,kind:&str,id:&str)->R<()>{
 let mut filtered=0;
 if let Some(rows)=value.get_mut("works").and_then(Value::as_array_mut){
  let original_count=rows.len();
  let mut effective=Vec::with_capacity(rows.len());
  for mut row in std::mem::take(rows){
   let Some(work)=row["id"].as_str() else {effective.push(row);continue;};
   let (_,hidden)=read(c,work)?;
   let removed=match kind{"person"=>hidden.people.contains(id),"character"=>hidden.characters.contains(id),"company"=>hidden.companies.contains(id),_=>false};
   if removed{continue;}
   let credited=|row:&Value|row["staff"].as_array().is_some_and(|v|v.iter().any(|s|s["id"]==id))||row["va"].as_array().is_some_and(|v|v.iter().any(|s|s["staff"]["id"]==id));
   let was_credited=kind=="person"&&credited(&row);
   let mut wrapped=json!({"vn":row});project(&mut wrapped,&hidden);row=wrapped["vn"].take();
   // A person's sole voice credit may have disappeared with a hidden character.
   if was_credited&&!credited(&row){continue;}
   effective.push(row);
  }
  filtered=original_count-effective.len();*rows=effective;
 }
 if filtered>0&&value["more"]==true{value["continuation_verified"]=json!(true);}
 if kind=="character"{if let Some(rows)=value.get_mut("character").and_then(|character|character.get_mut("vns")).and_then(Value::as_array_mut){
  let mut effective=Vec::with_capacity(rows.len());
  for row in std::mem::take(rows){if let Some(work)=row["id"].as_str(){if read(c,work)?.1.characters.contains(id){continue;}}effective.push(row);}
  *rows=effective;
 }}
 Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn cached_labels_only_describe_explicit_decisions(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
  let raw=json!({"vn":{"staff":[{"id":"s1","name":"Writer"},{"id":"s2","name":"Unselected person"}],"va":[{"staff":{"id":"s3","name":"Voice"}}],"developers":[{"id":"p1","name":"Studio"}],"relations":[{"id":"v2","title":"Sequel"}]},"characters":[{"id":"c1","name":"Chosen character"},{"id":"c2","name":"Spoiler character"}]});
  c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v1.1',?1)",[raw.to_string()]).unwrap();let hidden=Hidden{people:["s1".into(),"s3".into(),"s99".into()].into(),characters:["c1".into()].into(),companies:["p1".into()].into(),works:["v2".into()].into()};
  assert_eq!(labels(&c,"v1",&hidden).ok().unwrap(),json!({"people:s1":"Writer","people:s3":"Voice","characters:c1":"Chosen character","companies:p1":"Studio","works:v2":"Sequel"}));assert_eq!(labels(&c,"v1",&Hidden::default()).ok().unwrap(),json!({}));assert_eq!(labels(&c,"v99",&hidden).ok().unwrap(),json!({}));
 }
 #[tokio::test]async fn api_history_is_private_paged_and_edits_are_replayable(){
  use tower::ServiceExt;use axum::{body::{Body,to_bytes},http::Request};
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let app=crate::router(a.clone());
  let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
  for revision in 0..52{let input=Edit{revision,request_id:format!("edit-{revision}"),hidden:Hidden{people:if revision%2==0{["s1".into()].into()}else{BTreeSet::new()},..Default::default()}};
   let body=serde_json::to_string(&input).unwrap();let response=app.clone().oneshot(Request::builder().method("POST").uri("/api/vns/v1/relationship-decisions").header("host","127.0.0.1").header("authorization",h["authorization"].clone()).header("content-type","application/json").body(Body::from(body)).unwrap()).await.unwrap();assert_eq!(response.status(),200);
  }
  let first=history(State(a.clone()),h.clone(),Path("v1".into()),Query(HistoryPage{before:None})).await.ok().unwrap().0;assert_eq!(first["items"].as_array().unwrap().len(),50);assert_eq!(first["next"],3);
  let replay=Edit{revision:51,request_id:"edit-51".into(),hidden:Hidden::default()};assert_eq!(edit(State(a.clone()),h.clone(),Path("v1".into()),Json(replay)).await.ok().unwrap().0["revision"],52);
  assert!(edit(State(a.clone()),h.clone(),Path("v1".into()),Json(Edit{revision:0,request_id:"stale-http".into(),hidden:Hidden::default()})).await.is_err());
  let _ = edit(State(a.clone()),h.clone(),Path("v1".into()),Json(Edit{revision:52,request_id:"new-http".into(),hidden:Hidden::default()})).await.ok().unwrap();
  let second=history(State(a.clone()),h.clone(),Path("v1".into()),Query(HistoryPage{before:Some(3)})).await.ok().unwrap().0;assert_eq!(second["items"].as_array().unwrap().iter().map(|v|v["revision"].as_i64().unwrap()).collect::<Vec<_>>(),vec![2,1]);assert!(second["next"].is_null());
  crate::access::setup(&a.db,"Generated-fixture-password-729!",false).unwrap();let login=crate::access::web_login(State(a.clone()),Json(json!({"password":"Generated-fixture-password-729!","name":"fixture"}))).await.ok().unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap();
  for(method,path)in [("GET","relationship-decisions"),("GET","relationship-history"),("POST","relationship-decisions")]{let response=app.clone().oneshot(Request::builder().method(method).uri(format!("/api/vns/v1/{path}")).header("host","127.0.0.1").header("cookie",cookie).header("content-type","application/json").body(Body::from("{}")).unwrap()).await.unwrap();assert_eq!(response.status(),403);let bytes=to_bytes(response.into_body(),4096).await.unwrap();assert!(!String::from_utf8_lossy(&bytes).contains("s1"));}
  assert_eq!(get(State(a.clone()),h,Path("v1".into())).await.ok().unwrap().0["revision"],53);
  assert!(get(State(a),HeaderMap::new(),Path("v1".into())).await.is_err());
 }
 #[test]fn reverse_profiles_apply_current_decisions_without_truncating_pagination(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  let tx=c.transaction().unwrap();change(&tx,"v1",&Edit{revision:0,request_id:"reverse".into(),hidden:Hidden{characters:["c1".into()].into(),companies:["p1".into()].into(),..Default::default()}}).ok().unwrap();tx.commit().unwrap();
  let raw=json!({"works":[{"id":"v1","staff":[],"va":[{"staff":{"id":"s1"},"character":{"id":"c1"}}]},{"id":"v2","staff":[],"va":[]}],"character":{"id":"c1","vns":[{"id":"v1"},{"id":"v2"}]},"more":true,"page":2});
  for(kind,id)in [("person","s1"),("character","c1"),("company","p1")]{let mut value=raw.clone();project_profile(&c,&mut value,kind,id).ok().unwrap();assert_eq!(value["works"].as_array().unwrap().len(),1);assert_eq!(value["works"][0]["id"],"v2");assert_eq!(value["more"],true);assert_eq!(value["page"],2);assert_eq!(value["continuation_verified"],true);if kind=="character"{assert_eq!(value["character"]["vns"],json!([{"id":"v2"}]));}}
  let mut other_credit=raw.clone();other_credit["works"][0]["staff"]=json!([{"id":"s1","role":"scenario"}]);project_profile(&c,&mut other_credit,"person","s1").ok().unwrap();assert_eq!(other_credit["works"].as_array().unwrap().len(),2);assert!(other_credit["works"][0]["va"].as_array().unwrap().is_empty());
  let tx=c.transaction().unwrap();change(&tx,"v1",&Edit{revision:1,request_id:"reverse-reset".into(),hidden:Hidden::default()}).ok().unwrap();tx.commit().unwrap();let mut reset=raw.clone();project_profile(&c,&mut reset,"character","c1").ok().unwrap();assert_eq!(reset,raw);
 }
 #[test]fn history_replay_reset_and_backup_preserve_source(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
 let raw=json!({"vn":{"developers":[{"id":"p1"}],"staff":[{"id":"s1","aid":1},{"id":"s1","aid":2},{"id":"s2"}],"va":[{"staff":{"id":"s2"},"character":{"id":"c1"}}],"relations":[{"id":"v2"}]},"characters":[{"id":"c1"},{"id":"c2"}]});
 let hidden=Hidden{people:["s1".into()].into(),characters:["c1".into()].into(),companies:["p1".into()].into(),works:["v2".into()].into()};
 let input=Edit{revision:0,request_id:"hide".into(),hidden};
 {let mut c=db.lock().unwrap();c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v1.1',?1)",[raw.to_string()]).unwrap();let tx=c.transaction().unwrap();change(&tx,"v1",&input).ok().unwrap();tx.commit().unwrap();}drop(db);let db=crate::db::open(&path).unwrap();
 {let c=db.lock().unwrap();assert_eq!(change(&c,"v1",&input).ok().unwrap()["revision"],1);let mut projected=raw.clone();project(&mut projected,&read(&c,"v1").ok().unwrap().1);assert_eq!(projected["vn"]["staff"],json!([{"id":"s2"}]));assert_eq!(projected["characters"],json!([{"id":"c2"}]));for field in ["developers","va","relations"]{assert!(projected["vn"][field].as_array().unwrap().is_empty());}assert_eq!(c.query_row("SELECT value FROM settings WHERE key='exploration.v4.work.v1.1'",[],|r|r.get::<_,String>(0)).unwrap(),raw.to_string());}
 let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restore"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();let mut c=restored.lock().unwrap();assert_eq!(read(&c,"v1").ok().unwrap().1,input.hidden);let tx=c.transaction().unwrap();assert!(change(&tx,"v1",&Edit{revision:0,request_id:"stale".into(),hidden:Hidden::default()}).is_err());change(&tx,"v1",&Edit{revision:1,request_id:"reset".into(),hidden:Hidden::default()}).ok().unwrap();tx.commit().unwrap();let mut restored=raw.clone();project(&mut restored,&read(&c,"v1").ok().unwrap().1);assert_eq!(restored,raw);assert_eq!(c.query_row("SELECT count(*) FROM relation_override_history",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn receipt_failure_rolls_back_and_invalid_identity_rejects(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute_batch("CREATE TRIGGER fail_override_receipt BEFORE INSERT ON relation_override_requests BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
 {let tx=c.transaction().unwrap();assert!(change(&tx,"v1",&Edit{revision:0,request_id:"failure".into(),hidden:Hidden::default()}).is_err());}
 assert_eq!(read(&c,"v1").ok().unwrap().0,0);assert_eq!(c.query_row("SELECT count(*) FROM relation_override_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 assert!(validate("v1",&Hidden{people:["p1".into()].into(),..Default::default()}).is_err());
 }
}

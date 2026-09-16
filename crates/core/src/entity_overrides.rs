//! Private entity text decisions; null follows source, empty description intentionally hides it.
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Serialize,Deserialize};use serde_json::{Value,json};use crate::ApiError;
type R<T>=Result<T,ApiError>;
use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
fn authorize(a:&crate::App,h:&HeaderMap)->R<()>{
 let identity=crate::access::authenticate(a,h)?;
 if !matches!(identity.role.as_str(),"owner"|"desktop"){return Err(ApiError("Local owner access required".into()));}Ok(())
}
#[derive(Deserialize)]pub struct HistoryPage{before:Option<i64>}
pub async fn get(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{
 authorize(&a,&h)?;validate(&id,&Fields::default())?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let(revision,fields)=read(&c,&id)?;Ok(Json(json!({"entity_id":id,"revision":revision,"fields":fields})))
}
pub async fn edit(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Edit>)->crate::Result<Value>{
 authorize(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=change(&tx,&id,&input)?;tx.commit()?;Ok(Json(result))
}
pub async fn history(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<HistoryPage>)->crate::Result<Value>{
 authorize(&a,&h)?;validate(&id,&Fields::default())?;if page.before.is_some_and(|v|v<=0){return Err(ApiError("Invalid history cursor".into()));}
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
 let mut q=c.prepare("SELECT revision,created,before_json,after_json FROM entity_override_history WHERE entity_id=?1 AND (?2 IS NULL OR revision<?2) ORDER BY revision DESC LIMIT 51")?;
 let rows=q.query_map(params![id,page.before],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=rows.len()>50;let mut items=Vec::new();
 for(revision,created,before,after)in rows.into_iter().take(50){items.push(json!({"revision":revision,"created":created,"before":serde_json::from_str::<Fields>(&before).map_err(|e|ApiError(e.to_string()))?,"after":serde_json::from_str::<Fields>(&after).map_err(|e|ApiError(e.to_string()))?}));}
 let next=if more{items.last().map(|v|v["revision"].clone())}else{None};Ok(Json(json!({"items":items,"next":next})))
}

#[derive(Default,Clone,Debug,PartialEq,Serialize,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct Fields{pub name:Option<String>,pub description:Option<String>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct Edit{pub revision:i64,pub request_id:String,pub fields:Fields}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS entity_overrides(entity_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,fields_json TEXT NOT NULL);CREATE TABLE IF NOT EXISTS entity_override_history(entity_id TEXT NOT NULL,revision INTEGER NOT NULL,created INTEGER NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,PRIMARY KEY(entity_id,revision));CREATE TABLE IF NOT EXISTS entity_override_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL);")}
pub fn read(c:&Connection,id:&str)->R<(i64,Fields)>{let row:Option<(i64,String)>=c.query_row("SELECT revision,fields_json FROM entity_overrides WHERE entity_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;match row{Some((revision,raw))=>Ok((revision,serde_json::from_str(&raw).map_err(|e|ApiError(e.to_string()))?)),None=>Ok((0,Fields::default()))}}
fn validate(id:&str,fields:&Fields)->R<()>{
 if !['s','c','p'].iter().any(|prefix|crate::exploration::valid_id(id,*prefix)){return Err(ApiError("Invalid entity identity".into()));}
 if fields.name.as_ref().is_some_and(|name|name.trim().is_empty()||name.chars().count()>300||name.chars().any(char::is_control)){return Err(ApiError("Name must contain 1–300 characters without control characters".into()));}
 if fields.description.as_ref().is_some_and(|text|text.chars().count()>20000||text.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\r'|'\t'))){return Err(ApiError("Description is too long or contains unsupported control characters".into()));}Ok(())
}
// Caller transaction commits text, history and receipt atomically.
pub fn change(c:&Connection,id:&str,input:&Edit)->R<Value>{
 use sha2::{Digest,Sha256};validate(id,&input.fields)?;if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((old,result))=c.query_row("SELECT digest,result FROM entity_override_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=digest{return Err(ApiError("Request ID already used for another entity edit".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let(revision,before)=read(c,id)?;if revision!=input.revision{return Err(ApiError("Entity changed. Reload before editing.".into()));}
 let before=serde_json::to_string(&before).map_err(|e|ApiError(e.to_string()))?;let after=serde_json::to_string(&input.fields).map_err(|e|ApiError(e.to_string()))?;
 c.execute("INSERT INTO entity_overrides(entity_id,revision,fields_json) VALUES(?1,?2,?3) ON CONFLICT(entity_id) DO UPDATE SET revision=excluded.revision,fields_json=excluded.fields_json",params![id,revision+1,after])?;
 c.execute("INSERT INTO entity_override_history(entity_id,revision,created,before_json,after_json) VALUES(?1,?2,?3,?4,?5)",params![id,revision+1,crate::db::now(),before,after])?;
 let result=json!({"entity_id":id,"revision":revision+1});c.execute("INSERT INTO entity_override_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub fn project(value:&mut Value,fields:&Fields){if let Some(object)=value.as_object_mut(){let mut local=Vec::new();if fields.name.is_some(){local.push("name");}if fields.description.is_some(){local.push("description");}if !local.is_empty(){object.insert("local_fields".into(),json!(local));}if let Some(name)=&fields.name{object.insert("name".into(),json!(name));}if let Some(description)=&fields.description{object.insert("description".into(),json!(description));}}}
/// Update reference labels without replacing per-work staff credit aliases or copying biographies.
pub fn project_names(c:&Connection,value:&mut Value)->R<()>{
 fn visit(c:&Connection,value:&mut Value,names:&mut std::collections::HashMap<String,Option<String>>,depth:usize)->R<()>{
  if depth>32{return Err(ApiError("Entity reference nesting is too deep".into()));}
  match value{
   Value::Array(rows)=>for row in rows{visit(c,row,names,depth+1)?;},
   Value::Object(object)=>{
    let id=object.get("id").and_then(Value::as_str).filter(|id|['s','c','p'].iter().any(|prefix|crate::exploration::valid_id(id,*prefix))).map(str::to_owned);
    if object.contains_key("name"){if let Some(id)=id{if !names.contains_key(&id){names.insert(id.clone(),read(c,&id)?.1.name);}if let Some(Some(name))=names.get(&id){object.insert(if id.starts_with('s'){"display_name"}else{"name"}.into(),json!(name));}}}
    for child in object.values_mut(){visit(c,child,names,depth+1)?;}
   },_=>{}
  }Ok(())
 }
 visit(c,value,&mut Default::default(),0)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn reference_names_change_without_replacing_aliases_or_copying_biographies(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();
  for(id,name)in [("s1","Personal person"),("c1","Personal character"),("p1","Personal company")]{change(&tx,id,&Edit{revision:0,request_id:id.into(),fields:Fields{name:Some(name.into()),description:Some("Private long biography".into())}}).ok().unwrap();}tx.commit().unwrap();
  let raw=json!({"vn":{"developers":[{"id":"p1","name":"Source company"}],"staff":[{"id":"s1","name":"Credit alias","aid":9}],"va":[{"staff":{"id":"s1","name":"Voice alias","aid":10},"character":{"id":"c1","name":"Source character"}}]},"characters":[{"id":"c1","name":"Source character"}]});let mut value=raw.clone();project_names(&c,&mut value).ok().unwrap();assert_eq!(value["vn"]["developers"][0]["name"],"Personal company");assert_eq!(value["characters"][0]["name"],"Personal character");assert_eq!(value["vn"]["va"][0]["character"]["name"],"Personal character");assert_eq!(value["vn"]["staff"][0]["name"],"Credit alias");assert_eq!(value["vn"]["staff"][0]["display_name"],"Personal person");assert_eq!(value["vn"]["va"][0]["staff"]["name"],"Voice alias");assert!(!value.to_string().contains("Private long biography"));assert_eq!(raw["vn"]["developers"][0]["name"],"Source company");
 }
 #[tokio::test]async fn api_projects_profiles_preserves_credit_aliases_and_denies_web(){
  use axum::{body::{Body,to_bytes},http::Request};use tower::ServiceExt;
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let app=crate::router(a.clone());
  for(kind,id,endpoint)in [("person","s1","people"),("character","c1","characters"),("company","p1","companies")]{
   let raw=json!({(kind):{"id":id,"name":"Source","description":"Biography","vns":[{"id":"v1","spoiler":0}]},"works":[{"id":"v1","staff":[{"id":"s1","name":"Actual credit alias","aid":8}]}],"more":false,"page":1,"fetched_at":crate::db::now()});let key=format!("exploration.v3.{kind}.{id}.1");crate::exploration::save(&a,&key,&raw).unwrap();
   let body=json!({"revision":0,"request_id":id,"fields":{"name":"My display name","description":""}});
   let response=app.clone().oneshot(Request::builder().method("POST").uri(format!("/api/entities/{id}/field-decisions")).header("host","127.0.0.1").header("authorization",format!("Bearer {}",a.token)).header("content-type","application/json").body(Body::from(body.to_string())).unwrap()).await.unwrap();assert_eq!(response.status(),200);
   let response=app.clone().oneshot(Request::builder().uri(format!("/api/{endpoint}/{id}")).header("host","127.0.0.1").header("authorization",format!("Bearer {}",a.token)).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),200);let value:Value=serde_json::from_slice(&to_bytes(response.into_body(),65536).await.unwrap()).unwrap();assert_eq!(value[kind]["name"],"My display name");assert_eq!(value[kind]["description"],"");assert_eq!(value["works"][0]["staff"][0]["name"],"Actual credit alias");assert_eq!(crate::exploration::cached(&a,&key).unwrap().unwrap(),raw);
  }
  crate::access::setup(&a.db,"Generated-fixture-password-924!",false).unwrap();let login=crate::access::web_login(State(a.clone()),Json(json!({"password":"Generated-fixture-password-924!","name":"fixture"}))).await.ok().unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap();
  for(method,path)in [("GET","field-decisions"),("POST","field-decisions"),("GET","field-history")]{let response=app.clone().oneshot(Request::builder().method(method).uri(format!("/api/entities/s1/{path}")).header("host","127.0.0.1").header("cookie",cookie).header("content-type","application/json").body(Body::from("{}")).unwrap()).await.unwrap();assert_eq!(response.status(),403);}
 }
 #[test]fn text_decisions_backup_reset_and_replay_preserve_source(){let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();let raw=json!({"id":"c1","name":"Source name","description":"Source biography"});let input=Edit{revision:0,request_id:"edit".into(),fields:Fields{name:Some("Personal name".into()),description:Some("".into())}};
 {let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();change(&tx,"c1",&input).ok().unwrap();tx.commit().unwrap();}drop(db);let db=crate::db::open(&path).unwrap();{let c=db.lock().unwrap();assert_eq!(change(&c,"c1",&input).ok().unwrap()["revision"],1);let mut value=raw.clone();project(&mut value,&read(&c,"c1").ok().unwrap().1);assert_eq!(value["name"],"Personal name");assert_eq!(value["description"],"");}
 let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restore"),None).unwrap();let copy=crate::db::open(&restored.join("library.sqlite")).unwrap();let mut c=copy.lock().unwrap();assert_eq!(read(&c,"c1").ok().unwrap().1,input.fields);let tx=c.transaction().unwrap();assert!(change(&tx,"c1",&Edit{revision:0,request_id:"stale".into(),fields:Default::default()}).is_err());change(&tx,"c1",&Edit{revision:1,request_id:"reset".into(),fields:Default::default()}).ok().unwrap();tx.commit().unwrap();let mut value=raw.clone();project(&mut value,&read(&c,"c1").ok().unwrap().1);assert_eq!(value,raw);assert_eq!(c.query_row("SELECT count(*) FROM entity_override_history",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn invalid_fields_and_failed_receipt_do_not_commit(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();assert!(validate("v1",&Fields::default()).is_err());assert!(validate("s1",&Fields{name:Some(" ".into()),..Default::default()}).is_err());c.execute_batch("CREATE TRIGGER reject_entity_receipt BEFORE INSERT ON entity_override_requests BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();{let tx=c.transaction().unwrap();assert!(change(&tx,"s1",&Edit{revision:0,request_id:"fail".into(),fields:Fields{name:None,description:Some("Biography".into())}}).is_err());}assert_eq!(read(&c,"s1").ok().unwrap().0,0);assert_eq!(c.query_row("SELECT count(*) FROM entity_override_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
}

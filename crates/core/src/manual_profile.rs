//! Separate bounded page of manual relationship memberships, independent of provider pagination.
use rusqlite::{Connection,OptionalExtension};use serde_json::{Value,json};
fn visible(row:&Value,spoilers:bool)->bool{row["local_relation"]==true&&(spoilers||row["spoiler"].as_u64()==Some(0))}
fn belongs(v:&Value,kind:&str,id:&str,spoilers:bool)->bool{
 let any=|field:&str,predicate:&dyn Fn(&Value)->bool|v["vn"][field].as_array().is_some_and(|rows|rows.iter().any(|r|visible(r,spoilers)&&predicate(r)));
 match kind{"company"=>any("developers",&|r|r["id"]==id),"person"=>any("staff",&|r|r["id"]==id)||any("va",&|r|r["staff"]["id"]==id),"character"=>v["characters"].as_array().is_some_and(|rows|rows.iter().any(|r|r["local_relation"]==true&&r["id"]==id&&(spoilers||r["vns"].as_array().is_some_and(|vns|vns.iter().any(|v|v["spoiler"]==0)))))||any("va",&|r|r["character"]["id"]==id),_=>false}
}
pub(crate) fn metadata(c:&Connection,work:&str)->Result<Option<(String,String,String)>,String>{
  let cached:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[format!("exploration.v4.work.{work}.1")],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  let cached=cached.and_then(|v|serde_json::from_str::<Value>(&v).ok());
  let local:Option<(String,String,String)>=c.query_row("SELECT title,original_title,released FROM works WHERE vndb_id=?1 AND merged_into IS NULL ORDER BY id LIMIT 1",[work],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?;
  let data=if let Some(local)=local{local}else if let Some(v)=cached.as_ref().filter(|v|v["vn"]["id"]==work&&v["vn"]["title"].as_str().is_some_and(|s|!s.is_empty())){(v["vn"]["title"].as_str().unwrap().to_string(),v["vn"]["alttitle"].as_str().unwrap_or("").into(),v["vn"]["released"].as_str().unwrap_or("").into())}else{return Ok(None);};
 Ok(Some(data))
}
pub fn page(c:&Connection,kind:&str,id:&str,after:Option<&str>,revision:Option<i64>,spoilers:bool)->Result<Value,String>{
 if !matches!(kind,"person"|"character"|"company"){return Err("Invalid manual profile kind".into());}
 let candidates=crate::relation_correction_index::page(c,kind,id,after,revision)?;let mut works=vec![];let mut missing=vec![];
 for work in candidates["candidate_work_ids"].as_array().ok_or("Invalid candidate page")?.iter().filter_map(Value::as_str){
  let cached:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[format!("exploration.v4.work.{work}.1")],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  let minimal=cached.and_then(|s|serde_json::from_str::<Value>(&s).ok()).unwrap_or_else(||json!({"vn":{"id":work},"characters":[]}));let projected=crate::relation_projection::work_page(c,&minimal,work,spoilers)?;
  // Resolve metadata only after membership/visibility, so hidden missing IDs are not disclosed.
  if !belongs(&projected,kind,id,spoilers){continue;}
  let Some((title,original,released))=metadata(c,work)?else{missing.push(work.to_string());continue;};
  // This is an explicitly manual credit subset; callers merge with provider credits by edge identity.
  let mut row=json!({"id":work,"title":title,"alttitle":original,"released":released,"local_relationships":true,"profile_membership":true,"staff":[],"va":[]});
  for field in ["staff","va"]{row[field]=json!(projected["vn"][field].as_array().map(|rows|rows.iter().filter(|v|visible(v,spoilers)).cloned().collect::<Vec<_>>()).unwrap_or_default());}
  if kind=="character"{row["profile_spoiler"]=projected["characters"].as_array().into_iter().flatten().find(|r|r["id"]==id).and_then(|r|r["vns"].as_array()).into_iter().flatten().filter(|v|v["id"]==work).filter_map(|v|v["spoiler"].as_u64()).min().map_or(json!(2),|v|json!(v));}
  works.push(row);
 }
 Ok(json!({"works":works,"missing_work_ids":missing,"partial":!missing.is_empty(),"next":candidates["next"],"revision":candidates["revision"],"manual_credit_subset":true}))
}
#[cfg(test)]mod tests{
 use super::*;use crate::{relation_corrections::{Correction,Key,Link},relation_correction_store::{change,Edit}};
 fn add(c:&mut Connection,work:&str,spoiler:u8){change(c,work,&Edit{revision:0,request_id:format!("r{work}"),corrections:vec![Correction{id:"add".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:Some(17),role:"scenario".into(),note:"".into()},name:"Writer alias".into(),character_name:None,spoiler})}]}).ok().unwrap();}
 #[test]fn manual_only_work_uses_saved_title_without_inserting_collection_entry(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();add(&mut c,"v1",0);c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v1.1',?1)",[json!({"vn":{"id":"v1","title":"External work"}}).to_string()]).unwrap();let result=page(&c,"person","s1",None,None,false).unwrap();assert_eq!(result["works"][0]["title"],"External work");assert_eq!(result["works"][0]["staff"][0]["name"],"Writer alias");assert_eq!(result["works"][0]["staff"][0]["aid"],17);assert_eq!(result["partial"],false);assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
 #[test]fn hidden_or_spoiler_missing_metadata_does_not_leak_but_visible_missing_is_explicit(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();add(&mut c,"v1",2);let hidden=page(&c,"person","s1",None,None,false).unwrap();assert_eq!(hidden["missing_work_ids"],json!([]));assert_eq!(hidden["works"],json!([]));let shown=page(&c,"person","s1",None,None,true).unwrap();assert_eq!(shown["missing_work_ids"],json!(["v1"]));assert_eq!(shown["partial"],true);c.execute("INSERT INTO relation_overrides VALUES('v1',1,?1)",[json!({"people":["s1"]}).to_string()]).unwrap();assert_eq!(page(&c,"person","s1",None,None,true).unwrap()["missing_work_ids"],json!([]));}
 #[test]fn removed_source_target_does_not_become_manual_membership(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();change(&mut c,"v1",&Edit{revision:0,request_id:"hide".into(),corrections:vec![Correction{id:"hide".into(),replaces:Some(Key::Company{id:"p1".into()}),link:None}]}).ok().unwrap();let result=page(&c,"company","p1",None,None,false).unwrap();assert_eq!(result["works"],json!([]));assert_eq!(result["partial"],false);}
}
#[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]
pub struct Query {pub after:Option<String>,pub revision:Option<i64>,#[serde(default)]pub spoilers:bool}
pub async fn get(axum::extract::State(a):axum::extract::State<crate::App>,h:axum::http::HeaderMap,axum::extract::Path(id):axum::extract::Path<String>,axum::extract::Query(query):axum::extract::Query<Query>)->crate::Result<Value>{
 crate::access::authenticate(&a,&h)?;
 let kind=match id.as_bytes().first(){Some(b's')=>"person",Some(b'c')=>"character",Some(b'p')=>"company",_=>return Err(crate::ApiError("Invalid profile identity".into()))};
 let c=a.db.lock().map_err(|e|crate::ApiError(e.to_string()))?;
 Ok(axum::Json(page(&c,kind,&id,query.after.as_deref(),query.revision,query.spoilers).map_err(crate::ApiError)?))
}
#[cfg(test)]mod api_tests{
 use super::*;use tower::ServiceExt;use axum::{body::{Body,to_bytes},http::Request,extract::State,Json};
 #[tokio::test]async fn authenticated_readonly_page_hides_spoilers_and_rejects_writes(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();{
   use crate::{relation_corrections::{Correction,Key,Link},relation_correction_store::{change,Edit}};
   let mut c=a.db.lock().unwrap();change(&mut c,"v1",&Edit{revision:0,request_id:"seed".into(),corrections:vec![Correction{id:"one".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:Some(17),role:"scenario".into(),note:"".into()},name:"Generated secret credit".into(),character_name:None,spoiler:2})}]}).ok().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id,notes) VALUES('w1','Generated title','Generated title','v1','Private notes not for this endpoint')",[]).unwrap();
  }
  crate::access::setup(&a.db,"Generated manual-profile password!",false).unwrap();let login=crate::access::web_login(State(a.clone()),Json(json!({"password":"Generated manual-profile password!","name":"test"}))).await.ok().unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap();let router=crate::router(a.clone());
  let response=router.clone().oneshot(Request::builder().uri("/api/entities/s1/manual-works").header("host","127.0.0.1").body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),401);
  for spoilers in [false,true]{let response=router.clone().oneshot(Request::builder().uri(format!("/api/entities/s1/manual-works?spoilers={spoilers}")).header("host","127.0.0.1").header("cookie",cookie).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),200);let bytes=to_bytes(response.into_body(),100000).await.unwrap();let text=String::from_utf8(bytes.to_vec()).unwrap();assert!(!text.contains("Private notes"));assert_eq!(text.contains("Generated secret credit"),spoilers);assert_eq!(text.contains("Generated title"),spoilers);}
  let response=router.clone().oneshot(Request::builder().method("POST").uri("/api/entities/s1/manual-works").header("host","127.0.0.1").header("cookie",cookie).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),403);
  let response=router.oneshot(Request::builder().uri("/api/entities/s1/manual-works?after=v1").header("host","127.0.0.1").header("cookie",cookie).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),400);
 }
}

#[cfg(test)]mod voice_privacy_tests{
 use super::*;use crate::{relation_corrections::{Correction,Key,Link},relation_correction_store::{change,Edit}};
 #[test]fn safe_voice_addition_does_not_override_hidden_source_appearance(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  change(&mut c,"v1",&Edit{revision:0,request_id:"voice".into(),corrections:vec![Correction{id:"voice".into(),replaces:None,link:Some(Link{key:Key::Voice{id:"s1".into(),character:"c1".into(),alias:1,note:"".into()},name:"Safe voice".into(),character_name:Some("Generated character".into()),spoiler:0})}]}).map_err(|e|e.0).unwrap();
  c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v1.1',?1)",[json!({"vn":{"id":"v1","title":"Secret membership"},"characters":[{"id":"c1","vns":[{"id":"v1","spoiler":2}]}]}).to_string()]).unwrap();
  for(kind,id)in [("person","s1"),("character","c1")]{let safe=page(&c,kind,id,None,None,false).unwrap();assert_eq!(safe["works"],json!([]));assert_eq!(safe["missing_work_ids"],json!([]));assert_eq!(page(&c,kind,id,None,None,true).unwrap()["works"].as_array().unwrap().len(),1);}
 }
}

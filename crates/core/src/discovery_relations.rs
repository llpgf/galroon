//! Search predicates evaluated independently from provider work membership.
use crate::{ApiError,App,exploration,discovery::Search};
use serde_json::{Value,json};use std::collections::BTreeSet;use sha2::{Digest,Sha256};

pub(crate) struct Verdict{pub matches:BTreeSet<String>,pub partial:bool,pub stale:bool,pub warning:Option<String>}
fn any(mut filters:Vec<Value>)->Value{if filters.len()==1{filters.remove(0)}else{let mut result=vec![json!("or")];result.extend(filters);json!(result)}}
fn visible(v:&Value,work:&str,spoilers:bool)->bool{v["vns"].as_array().is_some_and(|rows|rows.iter().any(|v|v["id"]==work&&(spoilers||v["spoiler"]==0)))}
fn candidates(c:&rusqlite::Connection,input:&Search,works:&[String])->Result<BTreeSet<String>,ApiError>{
 let mut ids=BTreeSet::new();
 for work in works{
  let projected=crate::relation_projection::work_page(c,&json!({"vn":{"id":work},"characters":[]}),work,input.spoilers)?;
  let rows=if input.kind=="studio"{&projected["vn"]["developers"]}else{&projected["characters"]};
  for row in rows.as_array().into_iter().flatten(){if let Some(id)=row["id"].as_str(){ids.insert(id.to_string());}}
 }Ok(ids)
}
pub(crate) fn predicate(input:&Search,works:&[String],ids:&BTreeSet<String>,include_source:bool)->Option<(&'static str,Value)>{
 let mut scope=ids.iter().map(|id|json!(["id","=",id])).collect::<Vec<_>>();
 if input.kind=="trait"&&include_source&&!works.is_empty(){scope.push(json!(["vn","=",any(works.iter().map(|id|json!(["id","=",id])).collect())]));}
 if scope.is_empty(){return None;}
 let(endpoint,filter,fields)=if input.kind=="studio"{("producer",json!(["search","=",input.value.trim()]),"")}else{("character",json!(["dtrait","=",[input.value.trim(),if input.spoilers{2}else{0}]]),"vns.id,vns.spoiler")};
 Some((endpoint,json!({"filters":["and",filter,any(scope)],"fields":fields,"results":100,"page":1,"sort":"id","reverse":false})))
}
pub(crate) fn cache_key(endpoint:&str,body:&Value)->String{format!("discovery.edges.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes())))}
/// Cache only raw predicate evidence. Local edits are always projected after loading.
async fn evidence(a:&App,endpoint:&str,body:Value)->Result<Value,ApiError>{
 let key=cache_key(endpoint,&body);let fresh=|v:&Value|v["fetched_at"].as_i64().is_some_and(|t|t<=crate::db::now()&&crate::db::now()-t<3600);
 let old=exploration::cached(a,&key)?;if let Some(v)=old.as_ref().filter(|v|fresh(v)){return Ok(v.clone());}
 let mut gate=exploration::PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
 if let Some(v)=exploration::cached(a,&key)?.filter(fresh){return Ok(v);}
 let load=async{
  let mut rows=Vec::new();let mut seen=BTreeSet::new();let mut more=true;
  for page in 1..=10{let mut request=body.clone();request["page"]=json!(page);let response=exploration::query(endpoint,request,&mut gate).await?;
   let items=response["results"].as_array().ok_or("Invalid relationship search evidence")?;more=response["more"].as_bool().ok_or("Invalid relationship search continuation")?;
   for row in items{let id=row["id"].as_str().ok_or("Missing relationship identity")?;if !exploration::valid_id(id,if endpoint=="producer"{'p'}else{'c'})||!seen.insert(id.to_string()){return Err("Repeated relationship search evidence".into());}rows.push(row.clone());}
   if !more{break;}if items.is_empty(){return Err("Empty relationship search continuation".into());}
  }
  Ok::<Value,String>(json!({"results":rows,"partial":more,"fetched_at":crate::db::now()}))
 }.await;
 match load{Ok(value)=>{if value["partial"]==true{if let Some(previous)=old.filter(|v|v["partial"]!=true){return Ok(exploration::stale(previous,"Relationship search is incomplete. Retry.".into()));}}exploration::save(a,&key,&value)?;Ok(value)},Err(error)=>old.map(|v|exploration::stale(v,error.clone())).ok_or(ApiError(error))}
}
pub(crate) fn evaluate(c:&rusqlite::Connection,input:&Search,works:&[String],include_source:bool,evidence:&Value)->Result<Verdict,ApiError>{
 let rows=evidence["results"].as_array().ok_or_else(||ApiError("Invalid relationship evidence".into()))?;
 let eligible=rows.iter().filter_map(|v|v["id"].as_str()).collect::<BTreeSet<_>>();let mut matches=BTreeSet::new();
 for work in works{
  // Keep source appearances: an added safe voice must not reveal a hidden existing character.
  let characters=if input.kind=="trait"{rows.iter().filter(|v|v["vns"].as_array().is_some_and(|vs|vs.iter().any(|v|v["id"]==*work))).cloned().collect::<Vec<_>>()}else{vec![]};
  let value=crate::relation_projection::work_page(c,&json!({"vn":{"id":work},"characters":characters}),work,input.spoilers)?;
  let visible=if input.kind=="studio"{value["vn"]["developers"].as_array().into_iter().flatten().any(|v|v["id"].as_str().is_some_and(|id|eligible.contains(id)))}else{value["characters"].as_array().into_iter().flatten().any(|v|visible(v,work,input.spoilers)&&(include_source||v["local_relation"]==true)&&v["id"].as_str().is_some_and(|id|eligible.contains(id)))};
  if visible{matches.insert(work.clone());}
 }
 Ok(Verdict{matches,partial:evidence["partial"]==true||(input.kind=="trait"&&rows.iter().any(|r|!r["vns"].is_array())),stale:evidence["stale"]==true,warning:evidence["warning"].as_str().map(str::to_string)})
}
pub(crate) async fn memberships(a:&App,input:&Search,works:&[String],include_source:bool)->Result<Verdict,ApiError>{
 let ids={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;candidates(&c,input,works)?};
 // Keep every provider request below the documented 1000-predicate limit.
 let all=ids.into_iter().collect::<Vec<_>>();let chunks=if all.is_empty(){vec![BTreeSet::new()]}else{all.chunks(400).map(|v|v.iter().cloned().collect()).collect::<Vec<_>>()};
 let mut source=json!({"results":[],"partial":false});let mut rows=std::collections::BTreeMap::new();
 for (index,ids) in chunks.into_iter().enumerate(){if let Some((endpoint,body))=predicate(input,works,&ids,include_source&&index==0){let part=evidence(a,endpoint,body).await?;
  if part["partial"]==true{source["partial"]=json!(true);}if part["stale"]==true{source["stale"]=json!(true);}if part["warning"].is_string(){source["warning"]=part["warning"].clone();}
  for row in part["results"].as_array().into_iter().flatten(){if let Some(id)=row["id"].as_str(){rows.insert(id.to_string(),row.clone());}}
 }}source["results"]=json!(rows.into_values().collect::<Vec<_>>());
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;evaluate(&c,input,works,include_source,&source)
}

pub struct ManualSearch{input:Search,after:Option<String>}
// serde_urlencoded parses numeric/boolean fields directly; flatten loses its scalar coercion.
impl<'de> serde::Deserialize<'de> for ManualSearch{
 fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error>{
  #[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]struct Fields{kind:String,value:String,target:String,#[serde(default)]spoilers:bool,revision:Option<i64>,after:Option<String>}
  let fields=Fields::deserialize(deserializer)?;Ok(Self{input:Search{kind:fields.kind,value:fields.value,target:fields.target,page:1,spoilers:fields.spoilers,revision:fields.revision},after:fields.after})
 }
}
pub async fn manual(axum::extract::State(a):axum::extract::State<App>,h:axum::http::HeaderMap,axum::extract::Query(query):axum::extract::Query<ManualSearch>)->crate::Result<Value>{
 crate::auth(&a,&h)?;crate::discovery::request(&query.input)?;
 if query.after.as_ref().is_some_and(|v|!exploration::valid_id(v,'v'))||query.after.is_some()&&query.input.revision.is_none(){return Err(ApiError("Invalid manual search cursor".into()));}
 let(revision,ids,next)={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let revision=crate::relation_correction_index::current_revision(&c,query.input.revision)?;
  let kind=if query.input.kind=="studio"{"company"}else{"character"};
  let mut ids=if query.input.target=="works"&&matches!(query.input.kind.as_str(),"trait"|"studio"){c.prepare_cached("SELECT DISTINCT vndb_id FROM relation_correction_targets WHERE kind=?1 AND vndb_id>?2 ORDER BY vndb_id LIMIT 51")?.query_map(rusqlite::params![kind,query.after.as_deref().unwrap_or("")],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?}else{vec![]};
  let more=ids.len()>50;ids.truncate(50);let next=if more{ids.last().cloned()}else{None};(revision,ids,next)};
 let verdict=memberships(&a,&query.input,&ids,false).await?;
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;crate::relation_correction_index::current_revision(&c,Some(revision))?;
 let mut works=vec![];let mut missing=vec![];
 for work in &ids{if !verdict.matches.contains(work){continue;}let Some((title,original,released))=crate::manual_profile::metadata(&c,work)?else{missing.push(work.clone());continue;};
  let projected=crate::relation_projection::work_page(&c,&json!({"vn":{"id":work},"characters":[]}),work,query.input.spoilers)?;
  works.push(json!({"id":work,"title":title,"alttitle":original,"released":released,"developers":projected["vn"]["developers"].as_array().cloned().unwrap_or_default(),"profile_membership":true,"local_relationships":true}));
 }
 Ok(axum::Json(json!({"works":works,"next":next,"revision":revision,"partial":verdict.partial||verdict.stale||!missing.is_empty(),"missing_work_ids":missing,"warning":verdict.warning,"manual_credit_subset":true})))
}

#[cfg(test)]mod tests{
 use super::*;use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};use axum::extract::{State,Query};
 fn input(kind:&str,spoilers:bool)->Search{serde_json::from_value(json!({"kind":kind,"value":if kind=="trait"{"i1"}else{"Studio"},"target":"works","spoilers":spoilers})).unwrap()}
 fn add(c:&mut rusqlite::Connection,work:&str,key:Key,spoiler:u8){change(c,work,&Edit{revision:0,request_id:format!("r{work}"),corrections:vec![Correction{id:"one".into(),replaces:None,link:Some(Link{character_name:if matches!(key,Key::Voice{..}){Some("Generated character".into())}else{None},key,name:"Generated local edge".into(),spoiler})}]}).map_err(|e|e.0).unwrap();}
 #[test]fn exact_trait_membership_requires_visible_matching_appearance(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  change(&mut c,"v1",&Edit{revision:0,request_id:"hide".into(),corrections:vec![Correction{id:"hide".into(),replaces:Some(Key::Character{id:"c2".into()}),link:None}]}).map_err(|e|e.0).unwrap();
  add(&mut c,"v2",Key::Character{id:"c3".into()},0);add(&mut c,"v3",Key::Character{id:"c3".into()},2);add(&mut c,"v4",Key::Voice{id:"s1".into(),character:"c4".into(),alias:1,note:"".into()},0);add(&mut c,"v5",Key::Character{id:"c5".into()},0);
  let evidence=json!({"results":[{"id":"c1","vns":[{"id":"v1","spoiler":2}]},{"id":"c2","vns":[{"id":"v1","spoiler":0}]},{"id":"c3","vns":[]},{"id":"c4","vns":[{"id":"v4","spoiler":2}]}],"partial":false});
  let works=(1..=5).map(|i|format!("v{i}")).collect::<Vec<_>>();
  let safe=evaluate(&c,&input("trait",false),&works,true,&evidence).map_err(|e|e.0).unwrap();assert_eq!(safe.matches,["v2".into()].into());assert!(!safe.partial);
  let shown=evaluate(&c,&input("trait",true),&works,true,&evidence).map_err(|e|e.0).unwrap();assert_eq!(shown.matches,["v1".into(),"v2".into(),"v3".into(),"v4".into()].into());
  let manual=evaluate(&c,&input("trait",true),&works,false,&evidence).map_err(|e|e.0).unwrap();assert_eq!(manual.matches,["v2".into(),"v3".into()].into());
  assert!(evaluate(&c,&input("trait",false),&works,true,&json!({"results":[{"id":"c1"}]})).map_err(|e|e.0).unwrap().partial);
 }
 #[tokio::test]async fn manual_company_pages_are_bounded_private_revision_guarded_and_readonly(){
  use tower::ServiceExt;use axum::{body::{Body,to_bytes},http::Request};
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();{
   let mut c=a.db.lock().unwrap();for n in 1..=53{let work=format!("v{n:03}");add(&mut c,&work,Key::Company{id:"p1".into()},if n==53{2}else{0});if n!=52{c.execute("INSERT INTO works(id,title,original_title,vndb_id,notes) VALUES(?1,?1,?1,?2,'Private generated note')",rusqlite::params![format!("Generated {n}"),work]).unwrap();}}
  }
  let(ep,body)=predicate(&input("studio",false),&[],&["p1".into()].into(),false).unwrap();let key=cache_key(ep,&body);let raw=json!({"results":[{"id":"p1"}],"partial":false,"fetched_at":crate::db::now()});exploration::save(&a,&key,&raw).unwrap();
  let mut h=axum::http::HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
  let first=manual(State(a.clone()),h.clone(),Query(ManualSearch{input:input("studio",false),after:None})).await.map_err(|e|e.0).unwrap().0;assert_eq!(first["works"].as_array().unwrap().len(),50);assert_eq!(first["partial"],false);assert_eq!(first["revision"],53);
  let mut second_input=input("studio",false);second_input.revision=Some(53);let second=manual(State(a.clone()),h.clone(),Query(ManualSearch{input:second_input.clone(),after:first["next"].as_str().map(str::to_string)})).await.map_err(|e|e.0).unwrap().0;assert_eq!(second["works"].as_array().unwrap().len(),1);assert_eq!(second["missing_work_ids"],json!(["v052"]));assert_eq!(second["partial"],true);assert!(!second.to_string().contains("v053"));assert!(!first.to_string().contains("Private generated note"));assert!(second["next"].is_null());
  {let mut c=a.db.lock().unwrap();add(&mut c,"v054",Key::Company{id:"p1".into()},0);}
  assert!(manual(State(a.clone()),h.clone(),Query(ManualSearch{input:second_input,after:Some("v050".into())})).await.is_err());
  assert_eq!(exploration::cached(&a,&key).unwrap().unwrap(),raw);
  crate::access::setup(&a.db,"Generated discovery password!",false).unwrap();let login=crate::access::web_login(State(a.clone()),axum::Json(json!({"password":"Generated discovery password!","name":"fixture"}))).await.map_err(|e|e.0).unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap();let router=crate::router(a.clone());
  for(method,auth,status)in [("GET",false,401),("GET",true,200),("POST",true,403)]{let mut req=Request::builder().method(method).uri("/api/discover/manual?kind=studio&value=Studio&target=works&revision=54&spoilers=false").header("host","127.0.0.1");if auth{req=req.header("cookie",cookie);}let response=router.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status().as_u16(),status);let bytes=to_bytes(response.into_body(),1000000).await.unwrap();assert!(!String::from_utf8_lossy(&bytes).contains("Private generated note"));}
 }
}

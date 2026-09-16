//! Owner/Desktop exact editor. Preview fingerprints bind the complete input and read model.
use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
use rusqlite::{Connection,OptionalExtension};
use serde::Deserialize;
use serde_json::{Value,json};
use crate::{ApiError,App,relation_correction_store::{self as store,Edit},relation_corrections::Key};
type R<T>=Result<T,ApiError>;
fn authorize(a:&App,h:&HeaderMap)->R<()>{if !matches!(crate::access::authenticate(a,h)?.role.as_str(),"owner"|"desktop"){return Err(ApiError("Local owner access required".into()));}Ok(())}
#[derive(Default,Deserialize)]#[serde(deny_unknown_fields)]pub struct Page{#[serde(default)]spoilers:bool,before:Option<i64>}
fn revealed(value:bool)->R<()>{if !value{return Err(ApiError("Relationship editing includes hidden credits. Enable spoilers to review them.".into()));}Ok(())}
fn raw(c:&Connection,id:&str)->R<Value>{
 crate::relation_corrections::validate(id,&[]).map_err(ApiError)?;
 let saved:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[format!("exploration.v4.work.{id}.1")],|r|r.get(0)).optional()?;
 let value=match saved{Some(s)=>serde_json::from_str::<Value>(&s).map_err(|e|ApiError(e.to_string()))?,None=>json!({"vn":{"id":id},"source_unavailable":true})};
 if value["vn"]["id"]!=id{return Err(ApiError("Work source identity unavailable. Refresh the work first.".into()));}Ok(value)
}
fn relation_rows(raw:&Value,key:&Key)->Vec<Value>{let p=crate::relation_projection::path(key);if p=="characters"{&raw[p]}else{&raw["vn"][p]}.as_array().cloned().unwrap_or_default()}
/// Only editable relationship fields are returned, never provider descriptions or private work data.
fn edges(raw:&Value)->Vec<Value>{
 let mut result=vec![];
 for kind in ["company","character","staff","voice","work"]{
  let path=match kind{"company"=>"developers","character"=>"characters","staff"=>"staff","voice"=>"va",_=>"relations"};
  let rows=if kind=="character"{&raw[path]}else{&raw["vn"][path]};
  for row in rows.as_array().into_iter().flatten(){
   let text=|v:&Value,k:&str|v[k].as_str().unwrap_or("").to_owned();
   let key=match kind{
    "company"=>Key::Company{id:text(row,"id")},"character"=>Key::Character{id:text(row,"id")},
    "staff"=>Key::Staff{id:text(row,"id"),aid:row["aid"].as_i64(),role:text(row,"role"),note:text(row,"note")},
    "voice"=>{let Some(alias)=row["staff"]["aid"].as_i64()else{continue;};Key::Voice{id:text(&row["staff"],"id"),character:text(&row["character"],"id"),alias,note:text(row,"note")}},
    _=>Key::Work{id:text(row,"id"),relation:text(row,"relation")},
   };
   let name=if kind=="voice"{text(&row["staff"],"name")}else if kind=="work"{text(row,"title")}else{text(row,"name")};
   result.push(json!({"key":key,"name":name,"character_name":if kind=="voice"{row["character"]["name"].clone()}else{Value::Null},"local":row["local_relation"]==true,"correction_id":row["correction_id"],"spoiler":row["spoiler"],"appearances":if kind=="character"{row["vns"].clone()}else{Value::Null}}));
  }
 }result
}
pub async fn get(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->crate::Result<Value>{
 authorize(&a,&h)?;revealed(page.spoilers)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let(revision,corrections)=store::read(&c,&id)?;let source=raw(&c,&id)?;let(broad_revision,hidden)=crate::relation_overrides::read(&c,&id)?;
 let effective=crate::relation_projection::project(&source,&id,&corrections,&hidden).map_err(ApiError)?;
 Ok(Json(json!({"vndb_id":id,"revision":revision,"corrections":corrections,"source":edges(&source),"effective":edges(&effective),"source_unavailable":source["source_unavailable"]==true,"source_partial":source["partial"]==true||source["more"]==true,"broad_revision":broad_revision,"hidden":hidden})))
}
pub async fn history(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->crate::Result<Value>{authorize(&a,&h)?;revealed(page.spoilers)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(store::history(&c,&id,page.before)?))}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Preview{pub edit:Edit,pub spoilers:bool,#[serde(default)]pub history_revision:Option<i64>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Commit{pub edit:Edit,pub spoilers:bool,pub preview_digest:String,#[serde(default)]pub history_revision:Option<i64>}
fn digest(value:&Value)->R<String>{use sha2::{Digest,Sha256};Ok(hex::encode(Sha256::digest(serde_json::to_vec(value).map_err(|e|ApiError(e.to_string()))?)))}
fn prepare(c:&Connection,id:&str,input:&Edit,history_revision:Option<i64>)->R<Value>{
 crate::relation_corrections::validate(id,&input.corrections).map_err(ApiError)?;
 if input.request_id.trim().is_empty()||input.request_id.len()>128||input.request_id.chars().any(char::is_control){return Err(ApiError("Invalid request identity".into()));}
 let(revision,before)=store::read(c,id)?;if revision!=input.revision{return Err(ApiError("Relationship corrections changed. Reload before editing.".into()));}
 let historical=if let Some(target)=history_revision{
  if target<=0{return Err(ApiError("Invalid history revision".into()));}
  let saved:Option<String>=c.query_row("SELECT before_json FROM relation_correction_history WHERE vndb_id=?1 AND revision=?2",rusqlite::params![id,target],|r|r.get(0)).optional()?;
  let historical:Vec<crate::relation_corrections::Correction>=serde_json::from_str(&saved.ok_or_else(||ApiError("History entry unavailable".into()))?).map_err(|e|ApiError(e.to_string()))?;
  if historical!=input.corrections{return Err(ApiError("Historical state changed in the draft. Resolve edited identities before review.".into()));}true
 }else{false};
 let source=raw(c,id)?;let(broad_revision,hidden)=crate::relation_overrides::read(c,id)?;
 // New source choices must exist exactly once; saved orphaned decisions survive provider changes.
 for change in &input.corrections{if let Some(key)=&change.replaces{
  if !historical&&!before.iter().any(|old|old.id==change.id&&old.replaces.as_ref()==Some(key))&&relation_rows(&source,key).iter().filter(|row|crate::relation_projection::matches(row,key)).count()!=1{return Err(ApiError("Source credit is missing or ambiguous. Refresh and select its exact alias and role.".into()));}
 }}
 let mut resolved=vec![];
 for change in &input.corrections{if let Some(link)=&change.link{
  if !historical&&!before.iter().any(|old|old.link.as_ref().is_some_and(|old|old.key==link.key)){
   resolved.push(resolve_link(c,&link.key)?);
  }
 }}
 let prior=crate::relation_projection::project(&source,id,&before,&hidden).map_err(ApiError)?;
 let after=crate::relation_projection::project(&source,id,&input.corrections,&hidden).map_err(ApiError)?;
 let fingerprint=digest(&json!({"format":1,"work":id,"edit":input,"before":before,"source":source,"broad_revision":broad_revision,"hidden":hidden,"resolved":resolved,"history_revision":history_revision}))?;
 let shadowed=input.corrections.iter().filter(|change|change.link.is_some()&&!edges(&after).iter().any(|row|row["correction_id"]==change.id)).map(|v|v.id.clone()).collect::<Vec<_>>();
 Ok(json!({"preview_digest":fingerprint,"revision":revision,"before":edges(&prior),"after":edges(&after),"corrections_before":before,"corrections_after":input.corrections,"resolved":resolved,"shadowed_correction_ids":shadowed,"source_unavailable":source["source_unavailable"]==true,"source_partial":source["partial"]==true||source["more"]==true,"history_revision":history_revision}))
}
pub async fn preview(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Preview>)->crate::Result<Value>{authorize(&a,&h)?;revealed(input.spoilers)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(prepare(&c,&id,&input.edit,input.history_revision)?))}
pub async fn commit(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Commit>)->crate::Result<Value>{
 authorize(&a,&h)?;revealed(input.spoilers)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
 if let Some(result)=store::replay(&c,&id,&input.edit)?{return Ok(Json(result));}
 let preview=prepare(&c,&id,&input.edit,input.history_revision)?;if preview["preview_digest"]!=input.preview_digest{return Err(ApiError("Relationship preview changed. Review the current before and after again.".into()));}
 Ok(Json(store::change(&mut c,&id,&input.edit)?))
}
fn candidate_kind(id:&str)->R<(&'static str,&'static str,&'static str)>{
 for(prefix,kind,endpoint,fields)in [('s',"person","staff","name,original,aliases.aid,aliases.name,aliases.latin,aliases.ismain"),('c',"character","character","name,original"),('p',"company","producer","name,original"),('v',"work","vn","title,alttitle")]{if crate::exploration::valid_id(id,prefix){return Ok((kind,endpoint,fields));}}
 Err(ApiError("Invalid candidate identity".into()))
}
fn cached_candidate(c:&Connection,id:&str)->R<Value>{let(kind,_,_)=candidate_kind(id)?;
 let own:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[format!("relationship.candidate.{id}")],|r|r.get(0)).optional()?;
 if let Some(value)=own.and_then(|s|serde_json::from_str::<Value>(&s).ok()).filter(|v|v["id"]==id&&v["kind"]==kind){return Ok(value);}
 let key=if kind=="work"{format!("exploration.v4.work.{id}.1")}else{format!("exploration.v3.profile.{kind}.{id}")};
 let value:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[key],|r|r.get(0)).optional()?;
 let value=value.and_then(|s|serde_json::from_str::<Value>(&s).ok()).ok_or_else(||ApiError(format!("Resolve candidate {id} before reviewing this change.")))?;
 let profile=if kind=="work"{&value["vn"]}else{&value["profile"]};
 if profile["id"]!=id{return Err(ApiError("Candidate identity changed. Resolve it again.".into()));}
 Ok(json!({"id":id,"kind":kind,"name":if kind=="work"{&profile["title"]}else{&profile["name"]},"original":if kind=="work"{&profile["alttitle"]}else{&profile["original"]},"aliases":profile["aliases"],"fetched_at":value["fetched_at"]}))
}
fn resolve_link(c:&Connection,key:&Key)->R<Value>{
 let id=match key{Key::Company{id}|Key::Character{id}|Key::Work{id,..}|Key::Staff{id,..}|Key::Voice{id,..}=>id};
 let mut result=cached_candidate(c,id)?;
 let alias=match key{Key::Staff{aid,..}=>{if aid.is_none(){return Err(ApiError("Select the actual staff credit alias.".into()));}*aid},Key::Voice{alias,..}=>Some(*alias),_=>None};
 if let Some(aid)=alias{let aliases=result["aliases"].as_array().ok_or_else(||ApiError("Candidate aliases unavailable. Resolve the person again.".into()))?;let matching=aliases.iter().filter(|a|a["aid"]==aid).collect::<Vec<_>>();if matching.len()!=1{return Err(ApiError("Credit alias does not belong to the selected person.".into()));}result["selected_alias"]=matching[0].clone();}
 if let Key::Voice{character,..}=key{result["character"]=cached_candidate(c,character)?;}Ok(result)
}
#[derive(Default,Deserialize)]#[serde(deny_unknown_fields)]pub struct CandidateQuery{#[serde(default)]refresh:bool}
pub async fn candidate(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<CandidateQuery>)->crate::Result<Value>{
 authorize(&a,&h)?;let(kind,endpoint,fields)=candidate_kind(&id)?;
 let previous={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;cached_candidate(&c,&id).ok()};
 if !page.refresh&&previous.as_ref().is_some_and(|v|crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<86400){return Ok(Json(previous.unwrap()));}
 let mut gate=crate::exploration::PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
 let filters=if kind=="person"{json!(["and",["id","=",id],["ismain","=",1]])}else{json!(["id","=",id])};
 let response=crate::exploration::query(endpoint,json!({"filters":filters,"fields":fields,"results":1}),&mut gate).await;
 let profile=response.and_then(|r|r["results"].as_array().filter(|r|r.len()==1).and_then(|r|r.first()).filter(|r|r["id"]==id).cloned().ok_or("Candidate not found".into()));
 match profile{
  Ok(profile)=>{
   // Minimal work candidates must not replace the complete exploration cache.
   let key=format!("relationship.candidate.{id}");
   let value=json!({"id":id,"kind":kind,"name":if kind=="work"{&profile["title"]}else{&profile["name"]},"original":if kind=="work"{&profile["alttitle"]}else{&profile["original"]},"aliases":profile["aliases"],"fetched_at":crate::db::now()});
   crate::exploration::save(&a,&key,&value)?;Ok(Json(value))
  },Err(e)=>match previous{Some(mut v)=>{v["stale"]=json!(true);v["warning"]=json!(e);Ok(Json(v))},None=>Err(ApiError(e))}
 }
}

#[cfg(test)]mod tests{
 use super::*;use crate::relation_corrections::{Correction,Link};
 fn fixture()->(tempfile::TempDir,App,HeaderMap){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
  let source=json!({"vn":{"id":"v1","developers":[],"staff":[{"id":"s1","aid":1,"name":"Alias one","role":"scenario"},{"id":"s1","aid":2,"name":"Alias two","role":"scenario"}],"va":[],"relations":[]},"characters":[],"fetched_at":123,"private_unrequested":"do not return"});
  crate::exploration::save(&a,"exploration.v4.work.v1.1",&source).unwrap();
  for(id,kind,name,aliases)in [("s2","person","Canonical",json!([{"aid":9,"name":"Actual alias"}])),("p2","company","Studio",Value::Null)]{
   crate::exploration::save(&a,&format!("relationship.candidate.{id}"),&json!({"id":id,"kind":kind,"name":name,"aliases":aliases,"fetched_at":crate::db::now()})).unwrap();
  }(t,a,h)
 }
 fn edit()->Edit{Edit{revision:0,request_id:"generated-edit".into(),corrections:vec![Correction{id:"rebind-one".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:Some(1),role:"scenario".into(),note:"".into()}),link:Some(Link{key:Key::Staff{id:"s2".into(),aid:Some(9),role:"scenario".into(),note:"".into()},name:"Actual alias".into(),character_name:None,spoiler:2})}]}}
 #[tokio::test]async fn preview_commit_replay_undo_and_exact_aliases(){
  let(_t,a,h)=fixture();let input=edit();let before=crate::exploration::cached(&a,"exploration.v4.work.v1.1").unwrap().unwrap();
  let p=preview(State(a.clone()),h.clone(),Path("v1".into()),Json(Preview{edit:input.clone(),spoilers:true,history_revision:None})).await.ok().unwrap().0;
  assert_eq!(p["resolved"][0]["selected_alias"]["name"],"Actual alias");assert!(!p.to_string().contains("private_unrequested"));assert_eq!(p["after"][0]["key"]["aid"],2);
  let commit_input=||Commit{history_revision:None,edit:input.clone(),spoilers:true,preview_digest:p["preview_digest"].as_str().unwrap().into()};
  assert_eq!(commit(State(a.clone()),h.clone(),Path("v1".into()),Json(commit_input())).await.ok().unwrap().0["revision"],1);
  let undo=Edit{revision:1,request_id:"generated-undo".into(),corrections:vec![]};let up=preview(State(a.clone()),h.clone(),Path("v1".into()),Json(Preview{edit:undo.clone(),spoilers:true,history_revision:None})).await.ok().unwrap().0;
  assert_eq!(commit(State(a.clone()),h.clone(),Path("v1".into()),Json(Commit{history_revision:None,edit:undo,spoilers:true,preview_digest:up["preview_digest"].as_str().unwrap().into()})).await.ok().unwrap().0["revision"],2);
  // Lost-response replay cannot overwrite a subsequent undo, even if source metadata vanished.
  a.db.lock().unwrap().execute("DELETE FROM settings WHERE key='relationship.candidate.s2'",[]).unwrap();
  assert_eq!(commit(State(a.clone()),h.clone(),Path("v1".into()),Json(commit_input())).await.ok().unwrap().0["revision"],1);
  assert_eq!(store::read(&a.db.lock().unwrap(),"v1").ok().unwrap().0,2);
  let hist=history(State(a.clone()),h.clone(),Path("v1".into()),Query(Page{spoilers:true,before:None})).await.ok().unwrap().0;assert_eq!(hist["items"].as_array().unwrap().len(),2);
  assert_eq!(crate::exploration::cached(&a,"exploration.v4.work.v1.1").unwrap().unwrap(),before);
  let mut reused=input;reused.corrections.clear();assert!(commit(State(a),h,Path("v1".into()),Json(Commit{history_revision:None,edit:reused,spoilers:true,preview_digest:"".into()})).await.is_err());
 }
 #[test]fn preview_rejects_ambiguous_source_wrong_alias_and_changed_evidence(){
  let(_t,a,_)=fixture();let c=a.db.lock().unwrap();let input=edit();let first=prepare(&c,"v1",&input,None).ok().unwrap();
  let mut bad=input.clone();if let Key::Staff{aid,..}=&mut bad.corrections[0].link.as_mut().unwrap().key{*aid=Some(999);}assert!(prepare(&c,"v1",&bad,None).is_err());
  if let Key::Staff{aid,..}=&mut bad.corrections[0].link.as_mut().unwrap().key{*aid=None;}assert!(prepare(&c,"v1",&bad,None).is_err());
  let mut bad=input.clone();if let Some(Key::Staff{aid,..})=&mut bad.corrections[0].replaces{*aid=Some(99);}assert!(prepare(&c,"v1",&bad,None).is_err());
  c.execute("INSERT INTO relation_overrides VALUES('v1',1,?1)",[json!({"people":["s2"],"characters":[],"companies":[],"works":[]}).to_string()]).unwrap();
  let second=prepare(&c,"v1",&input,None).ok().unwrap();assert_ne!(first["preview_digest"],second["preview_digest"]);assert_eq!(second["shadowed_correction_ids"],json!(["rebind-one"]));
  let mut source=raw(&c,"v1").ok().unwrap();let row=source["vn"]["staff"][0].clone();source["vn"]["staff"].as_array_mut().unwrap().push(row);c.execute("UPDATE settings SET value=?1 WHERE key='exploration.v4.work.v1.1'",[source.to_string()]).unwrap();assert!(prepare(&c,"v1",&input,None).is_err());
 }
 #[test]fn complete_history_state_can_be_restored_without_source_or_candidate_cache(){
  let(_t,a,_)=fixture();let mut c=a.db.lock().unwrap();let original=edit();store::change(&mut c,"v1",&original).ok().unwrap();
  store::change(&mut c,"v1",&Edit{revision:1,request_id:"clear".into(),corrections:vec![]}).ok().unwrap();
  c.execute("DELETE FROM settings WHERE key LIKE 'exploration.%' OR key LIKE 'relationship.candidate.%'",[]).unwrap();
  let mut restore=Edit{revision:2,request_id:"restore-history".into(),corrections:original.corrections};
  assert!(prepare(&c,"v1",&restore,None).is_err());assert!(prepare(&c,"v1",&restore,Some(1)).is_err());
  let preview=prepare(&c,"v1",&restore,Some(2)).ok().unwrap();assert_eq!(preview["history_revision"],2);assert_eq!(preview["source_unavailable"],true);assert_eq!(preview["after"][0]["key"]["aid"],9);
  restore.corrections[0].link.as_mut().unwrap().name="Modified historical draft".into();assert!(prepare(&c,"v1",&restore,Some(2)).is_err());
 }
 #[tokio::test]async fn private_routes_require_owner_or_desktop_and_explicit_spoiler_review(){
  use tower::ServiceExt;use axum::{body::Body,http::Request};use sha2::{Digest,Sha256};
  let(_t,a,h)=fixture();let app=crate::router(a.clone());
  let token="d".repeat(72);{let c=a.db.lock().unwrap();c.execute("INSERT INTO sessions(token_hash,role,created,id,name,expires,last_used) VALUES(?1,'desktop',0,'paired','Generated',?2,0)",rusqlite::params![hex::encode(Sha256::digest(token.as_bytes())),crate::db::now()+600]).unwrap();}
  for auth in [h["authorization"].to_str().unwrap().to_owned(),format!("Bearer {token}")]{let response=app.clone().oneshot(Request::builder().uri("/api/vns/v1/relationship-corrections?spoilers=true").header("host","127.0.0.1").header("authorization",auth).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),200);}
  assert!(get(State(a.clone()),h.clone(),Path("v1".into()),Query(Page::default())).await.is_err());
  let cand=candidate(State(a.clone()),h.clone(),Path("s2".into()),Query(CandidateQuery::default())).await.ok().unwrap().0;assert_eq!(cand["aliases"][0]["aid"],9);
  crate::access::setup(&a.db,"Generated-fixture-password-729!",false).unwrap();let login=crate::access::web_login(State(a.clone()),Json(json!({"password":"Generated-fixture-password-729!","name":"fixture"}))).await.ok().unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap();
  for(method,path)in [("GET","/api/custom-tags/any/membership?ids=%5B%5D&revision=1&impact_revision=1"),("GET","/api/custom-tags?include_deleted=true&summary=true"),("GET","/api/vns/v1/relationship-corrections?spoilers=true"),("GET","/api/vns/v1/relationship-corrections/history?spoilers=true"),("GET","/api/relationship-candidates/s2"),("POST","/api/vns/v1/relationship-corrections/preview"),("POST","/api/vns/v1/relationship-corrections")]{let response=app.clone().oneshot(Request::builder().method(method).uri(path).header("host","127.0.0.1").header("cookie",cookie).header("content-type","application/json").body(Body::from("{}")).unwrap()).await.unwrap();assert_eq!(response.status(),403);}
  let p=preview(State(a.clone()),h.clone(),Path("v1".into()),Json(Preview{edit:edit(),spoilers:true,history_revision:None})).await.ok().unwrap().0;
  let bad=Commit{history_revision:None,edit:edit(),spoilers:true,preview_digest:"wrong".into()};assert!(commit(State(a.clone()),h.clone(),Path("v1".into()),Json(bad)).await.is_err());
  {let c=a.db.lock().unwrap();let mut source=raw(&c,"v1").ok().unwrap();source["fetched_at"]=json!(999);c.execute("UPDATE settings SET value=?1 WHERE key='exploration.v4.work.v1.1'",[source.to_string()]).unwrap();}
  assert!(commit(State(a.clone()),h,Path("v1".into()),Json(Commit{history_revision:None,edit:edit(),spoilers:true,preview_digest:p["preview_digest"].as_str().unwrap().into()})).await.is_err());assert_eq!(store::read(&a.db.lock().unwrap(),"v1").ok().unwrap().0,0);
 }
}

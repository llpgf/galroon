//! On-demand public VNDB metadata. Cache is separate from editable work metadata.
use crate::{App, ApiError};
use axum::{extract::{Path, Query, State}, http::HeaderMap, Json};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::OnceLock;

const TTL: i64 = 86400;
pub(crate) static PROVIDER: OnceLock<tokio::sync::Mutex<i64>> = OnceLock::new();
const WORK_FIELDS: &str = "title,alttitle,description,released,image.url,image.sexual,image.violence,developers.id,developers.name,developers.original,tags.id,tags.name,tags.spoiler,tags.lie,staff.id,staff.aid,staff.name,staff.original,staff.role,staff.eid,staff.note,va.staff.id,va.staff.aid,va.staff.name,va.staff.original,va.character.id,va.note,relations.id,relations.title,relations.relation,relations.image.url,relations.image.sexual,relations.image.violence";
const CHARACTER_DETAIL_FIELDS: &str = "name,original,description,aliases,image.url,image.sexual,image.violence,birthday,age,height,blood_type,traits.id,traits.name,traits.group_name,traits.spoiler,traits.lie,traits.sexual,vns.id,vns.role,vns.spoiler";
const PROFILE_WORK_FIELDS: &str = "title,alttitle,released,image.url,image.sexual,image.violence,staff.id,staff.aid,staff.name,staff.original,staff.role,staff.note,staff.eid,va.staff.id,va.staff.aid,va.staff.name,va.staff.original,va.note,va.character.id,va.character.name,va.character.original,va.character.image.url,va.character.image.sexual,va.character.image.violence,va.character.vns.id,va.character.vns.spoiler";
const CHARACTER_FIELDS: &str = "name,original,description,image.url,image.sexual,image.violence,vns.id,vns.role,vns.spoiler";

pub(crate) fn valid_id(id: &str, prefix: char) -> bool {
    id.strip_prefix(prefix).is_some_and(|s| !s.is_empty() && s.len() <= 12 && s.bytes().all(|c| c.is_ascii_digit()))
}
pub(crate) fn cached(a: &App, key: &str) -> Result<Option<Value>, String> {
    let raw: Option<String> = a.db.lock().map_err(|e|e.to_string())?.query_row("SELECT value FROM settings WHERE key=?1", [key], |r|r.get(0)).optional().map_err(|e|e.to_string())?;
    Ok(raw.and_then(|s|serde_json::from_str(&s).ok()))
}
pub(crate) fn save(a: &App, key: &str, value: &Value) -> Result<(), String> {
    a.db.lock().map_err(|e|e.to_string())?.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,?2)", params![key,value.to_string()]).map_err(|e|e.to_string())?;
    Ok(())
}
pub(crate) async fn query(endpoint: &str, body: Value, cooldown: &mut i64) -> Result<Value, String> {
    if *cooldown > crate::db::now() { return Err("VNDB is cooling down. Try again in a few minutes.".into()); }
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    let response = reqwest::Client::new().post(format!("https://api.vndb.org/kana/{endpoint}"))
        .timeout(std::time::Duration::from_secs(12)).json(&body).send().await.map_err(|e|e.to_string())?;
    if !response.status().is_success() {
        *cooldown = crate::db::now() + if response.status().as_u16()==429 { 300 } else { 30 };
        return Err(format!("VNDB returned {}. Try again later.",response.status()));
    }
    response.json().await.map_err(|e|e.to_string())
}
pub(crate) fn stale(mut value: Value, error: String) -> Value { value["stale"]=json!(true); value["warning"]=json!(error); value }
fn retain_complete(previous:Option<Value>,result:Result<Value,String>)->Result<Value,String>{
 match result{
  Ok(value) if value["partial"]==true=>match previous.filter(|old|old["partial"]!=true){Some(old)=>Ok(stale(old,value["warning"].as_str().unwrap_or("Incomplete provider response").to_owned())),None=>Ok(value)},
  Ok(value)=>Ok(value),Err(error)=>previous.map(|old|stale(old,error.clone())).ok_or(error)
 }
}

async fn load(a: &App, kind: &str, id: &str, page: u32, refresh: bool) -> Result<Value, String> {
    let version=if kind=="work"{4}else{3};
    let key=format!("exploration.v{version}.{kind}.{id}.{page}");
    let previous=cached(a,&key)?;
    if let Some(v)=&previous { if !refresh && crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<TTL {return Ok(v.clone());} }
    // Bound demand and deduplicate concurrent cache misses across clients.
    let mut gate=PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
    if let Some(v)=cached(a,&key)? {if !refresh && crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<TTL{return Ok(v);}}
    let result=async {
        let mut value=if kind=="work" {
            let vn=query("vn",json!({"filters":["id","=",id],"fields":WORK_FIELDS,"results":1}),&mut gate).await?;
            let vn=vn["results"].as_array().and_then(|v|v.first()).ok_or("VNDB work not found")?.clone();
            let mut characters=Vec::new();let mut more=false;let mut warning=None;
            for p in 1..=10 {
                let response=match query("character",json!({"filters":["vn","=",["id","=",id]],"fields":CHARACTER_FIELDS,"results":100,"page":p}),&mut gate).await {Ok(value)=>value,Err(error)=>{warning=Some(error);break;}};
                characters.extend(response["results"].as_array().ok_or("Invalid VNDB character response")?.iter().cloned());
                more=response["more"].as_bool().unwrap_or(false);if !more {break;}
            }
            json!({"vn":vn,"characters":characters,"more":more,"partial":warning.is_some(),"warning":warning})
        } else {
            let (endpoint,fields,filters)=match kind {
                "character"=>("character",CHARACTER_DETAIL_FIELDS,json!(["id","=",id])),
                "company"=>("producer","name,original,description,aliases,lang,type,extlinks.url,extlinks.label",json!(["id","=",id])),
                _=>("staff","name,original,description,lang,gender,aliases.aid,aliases.name,aliases.latin,aliases.ismain,extlinks.url,extlinks.label",json!(["and",["id","=",id],["ismain","=",1]])),
            };
            let profile_key=format!("exploration.v3.profile.{kind}.{id}");
            let saved=cached(a,&profile_key)?;
            let profile=if !refresh&&saved.as_ref().is_some_and(|v|crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<TTL){saved.unwrap()["profile"].clone()}else{
                let response=query(endpoint,json!({"filters":filters,"fields":fields,"results":1}),&mut gate).await?;
                let profile=response["results"].as_array().and_then(|v|v.first()).ok_or("VNDB entry not found")?.clone();
                save(a,&profile_key,&json!({"profile":profile,"fetched_at":crate::db::now()}))?;profile
            };
            let filter=match kind {"character"=>json!(["character","=",["id","=",id]]),"company"=>json!(["developer","=",["id","=",id]]),_=>json!(["staff","=",["id","=",id]])};
            let fields=if kind=="company"{"title,alttitle,released,image.url,image.sexual,image.violence"}else{PROFILE_WORK_FIELDS};
            // Keep the profile visible if its separately loaded work list fails.
            match query("vn",json!({"filters":filter,"fields":fields,"sort":"released","reverse":true,"results":20,"page":page}),&mut gate).await {
                Ok(works)=>json!({(kind):profile,"works":works["results"],"more":works["more"],"page":page}),
                Err(error)=>json!({(kind):profile,"works":[],"more":false,"page":page,"partial":true,"warning":error}),
            }

        };
        value["fetched_at"]=json!(crate::db::now());value["stale"]=json!(false);if value["partial"]!=true {save(a,&key,&value)?;}Ok::<Value,String>(value)
    }.await;
    retain_complete(previous,result)
}
pub async fn work(State(a):State<App>, h:HeaderMap, Path(id):Path<String>, Query(page):Query<Page>) -> Result<Json<Value>,ApiError> {
    crate::auth(&a,&h)?;
    let vndb:Option<String>=a.db.lock().map_err(|e|ApiError(e.to_string()))?.query_row("SELECT vndb_id FROM works WHERE id=?1",[id],|r|r.get(0))?;
    let Some(vndb)=vndb.filter(|s|!s.is_empty()) else {return Ok(Json(json!({"vn":null,"characters":[],"more":false,"local":true})));};
    if !valid_id(&vndb,'v'){return Err(ApiError("Invalid VNDB work ID".into()));}
    let mut value=load(&a,"work",&vndb,1,page.refresh).await?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;value=crate::relation_projection::work_page(&c,&value,&vndb,page.spoilers)?;crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
pub async fn reference_work(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->Result<Json<Value>,ApiError>{
    crate::auth(&a,&h)?;if !valid_id(&id,'v'){return Err(ApiError("Invalid VNDB work ID".into()));}
    let mut value=load(&a,"work",&id,1,page.refresh).await?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;value=crate::relation_projection::work_page(&c,&value,&id,page.spoilers)?;crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
#[derive(serde::Deserialize)] pub struct Page {#[serde(default="first")]page:u32,#[serde(default)]refresh:bool,#[serde(default)]spoilers:bool,revision:Option<i64>}
fn first()->u32{1}
pub async fn person(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->Result<Json<Value>,ApiError>{
    crate::auth(&a,&h)?;if !valid_id(&id,'s')||page.page==0||page.page>1000{return Err(ApiError("Invalid person or page".into()));}
    let mut value=load(&a,"person",&id,page.page,page.refresh).await?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;value=crate::relation_projection::visible_profile_page(&c,&value,"person",&id,page.spoilers)?;let revision=crate::relation_correction_index::current_revision(&c,page.revision)?;value["relationship_revision"]=json!(revision);if let Some(entity)=value.get_mut("person"){crate::entity_overrides::project(entity,&crate::entity_overrides::read(&c,&id)?.1);}crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
pub async fn character(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->Result<Json<Value>,ApiError>{
    crate::auth(&a,&h)?;if !valid_id(&id,'c')||page.page==0||page.page>1000{return Err(ApiError("Invalid character or page".into()));}
    let mut value=load(&a,"character",&id,page.page,page.refresh).await?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;value=crate::relation_projection::visible_profile_page(&c,&value,"character",&id,page.spoilers)?;let revision=crate::relation_correction_index::current_revision(&c,page.revision)?;value["relationship_revision"]=json!(revision);if let Some(entity)=value.get_mut("character"){crate::entity_overrides::project(entity,&crate::entity_overrides::read(&c,&id)?.1);}crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
pub async fn company(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(page):Query<Page>)->Result<Json<Value>,ApiError>{
    crate::auth(&a,&h)?;if !valid_id(&id,'p')||page.page==0||page.page>1000{return Err(ApiError("Invalid company or page".into()));}
    let mut value=load(&a,"company",&id,page.page,page.refresh).await?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;value=crate::relation_projection::visible_profile_page(&c,&value,"company",&id,page.spoilers)?;let revision=crate::relation_correction_index::current_revision(&c,page.revision)?;value["relationship_revision"]=json!(revision);if let Some(entity)=value.get_mut("company"){crate::entity_overrides::project(entity,&crate::entity_overrides::read(&c,&id)?.1);}crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
#[cfg(test)] mod tests {
 #[test]fn partial_refresh_preserves_complete_page_and_first_load_stays_partial(){
  use serde_json::json;
  let old=json!({"person":{"id":"s1","description":"Saved introduction"},"works":[{"id":"v1"}],"page":2,"more":true,"fetched_at":123});
  let partial=json!({"person":{"id":"s1","description":"New introduction"},"works":[],"partial":true,"warning":"Offline","more":false,"fetched_at":456});
  let result=super::retain_complete(Some(old.clone()),Ok(partial.clone())).unwrap();assert_eq!(result["person"],old["person"]);assert_eq!(result["works"],old["works"]);assert_eq!(result["page"],2);assert_eq!(result["more"],true);assert_eq!(result["fetched_at"],123);assert_eq!(result["stale"],true);assert_eq!(result["warning"],"Offline");
  assert_eq!(super::retain_complete(None,Ok(partial.clone())).unwrap(),partial);let fresh=json!({"works":[],"more":false});assert_eq!(super::retain_complete(Some(old.clone()),Ok(fresh.clone())).unwrap(),fresh);
  let failed=super::retain_complete(Some(old),Err("Unavailable".into())).unwrap();assert_eq!(failed["stale"],true);assert!(super::retain_complete(None,Err("Unavailable".into())).is_err());
 }
    use super::*;
    #[tokio::test] async fn endpoints_require_auth_and_local_works_do_not_contact_provider(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();
        a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('local','Local','Local')",[]).unwrap();
        assert!(work(State(a.clone()),HeaderMap::new(),Path("local".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert!(person(State(a.clone()),HeaderMap::new(),Path("s240".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        let result=work(State(a.clone()),h.clone(),Path("local".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.ok().unwrap();assert_eq!(result.0["local"],true);
        assert!(person(State(a),h,Path("s240".into()),Query(Page{page:0,refresh:false,spoilers:false,revision:None})).await.is_err());
    }
    #[tokio::test] async fn profiles_use_cached_pages_and_enforce_auth(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();
        let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        for (kind,id) in [("character","c1"),("company","p1"),("person","s1")] {
            let value=json!({(kind):{"id":id,"name":"Cached","description":null},"works":[{"id":"v1","title":"One"}],"more":true,"page":2,"fetched_at":crate::db::now()});
            save(&a,&format!("exploration.v3.{kind}.{id}.2"),&value).unwrap();
            assert_eq!(load(&a,kind,id,2,false).await.unwrap(),value);
        }
        assert!(character(State(a.clone()),HeaderMap::new(),Path("c1".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert!(company(State(a.clone()),HeaderMap::new(),Path("p1".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert!(character(State(a.clone()),h.clone(),Path("c1".into()),Query(Page{page:0,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert!(company(State(a.clone()),h.clone(),Path("p1".into()),Query(Page{page:1001,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert_eq!(company(State(a),h,Path("p1".into()),Query(Page{page:2,refresh:false,spoilers:false,revision:None})).await.ok().unwrap().0["company"]["id"],"p1");
    }
    #[tokio::test] async fn reference_work_is_authenticated_read_only(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();
        assert!(reference_work(State(a.clone()),HeaderMap::new(),Path("v1".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        let value=json!({"vn":{"id":"v1","title":"Reference"},"characters":[],"fetched_at":crate::db::now()});
        save(&a,"exploration.v4.work.v1.1",&value).unwrap();
        let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        assert!(reference_work(State(a.clone()),h.clone(),Path("s1".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.is_err());
        assert_eq!(reference_work(State(a.clone()),h,Path("v1".into()),Query(Page{page:1,refresh:false,spoilers:false,revision:None})).await.ok().unwrap().0,value);
        assert_eq!(a.db.lock().unwrap().query_row("SELECT COUNT(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]fn validates_public_identifiers(){assert!(valid_id("v2002",'v'));for id in ["v","s2002","v../../x","v1?x","v１２","v1234567890123"]{assert!(!valid_id(id,'v'));}}
    #[test]fn stale_keeps_payload_and_marks_warning(){let v=stale(json!({"characters":[{"id":"c1"}],"fetched_at":1}),"offline".into());assert_eq!(v["characters"][0]["id"],"c1");assert_eq!(v["stale"],true);assert_eq!(v["warning"],"offline");}
    #[test]fn cache_does_not_modify_work_metadata(){let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('w','My title','Original')",[]).unwrap();save(&a,"exploration.v2.work.v1.1",&json!({"fetched_at":123,"characters":[]})).unwrap();assert_eq!(cached(&a,"exploration.v2.work.v1.1").unwrap().unwrap()["fetched_at"],123);assert_eq!(a.db.lock().unwrap().query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"My title");}
}

#[cfg(test)] mod corrected_profile_api_tests {
 use super::*;
 #[tokio::test] async fn runtime_profiles_project_current_corrections_without_mutating_cached_page(){
  use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let mut headers=HeaderMap::new();headers.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
  let raw=json!({"person":{"id":"s1","name":"Actor"},"works":[{"id":"v1","title":"Generated work","staff":[],"va":[]}],"more":true,"page":1,"fetched_at":crate::db::now()});save(&a,"exploration.v3.person.s1.1",&raw).unwrap();
  {let mut c=a.db.lock().unwrap();change(&mut c,"v1",&Edit{revision:0,request_id:"profile-api".into(),corrections:vec![Correction{id:"one".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:Some(8),role:"music".into(),note:"".into()},name:"Generated spoiler alias".into(),character_name:None,spoiler:2})}]}).ok().unwrap();}
  for spoilers in [false,true]{let result=person(State(a.clone()),headers.clone(),Path("s1".into()),Query(Page{page:1,refresh:false,spoilers,revision:Some(1)})).await.ok().unwrap().0;assert_eq!(result["relationship_revision"],1);assert_eq!(result.to_string().contains("Generated spoiler alias"),spoilers);assert_eq!(result["works"].as_array().unwrap().len(),usize::from(spoilers));}
  assert_eq!(cached(&a,"exploration.v3.person.s1.1").unwrap().unwrap(),raw);
  {let mut c=a.db.lock().unwrap();let tx=c.transaction().unwrap();let edit=crate::relation_overrides::Edit{revision:0,request_id:"broad".into(),hidden:crate::relation_overrides::Hidden{people:["s1".into()].into(),..Default::default()}};crate::relation_overrides::change(&tx,"v1",&edit).ok().unwrap();crate::relation_overrides::change(&tx,"v1",&edit).ok().unwrap();tx.commit().unwrap();assert_eq!(crate::relation_correction_index::current_revision(&c,None).unwrap(),2);}
  assert!(person(State(a),headers,Path("s1".into()),Query(Page{page:1,refresh:false,spoilers:true,revision:Some(1)})).await.is_err());
 }
}

#[cfg(test)]mod exact_forward_tests{
 use super::*;use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};
 #[tokio::test]async fn work_and_reference_project_identical_exact_edges_and_spoilers(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
  let raw=json!({"vn":{"id":"v1","title":"Work","staff":[{"id":"s1","aid":1,"role":"music","name":"Source one"},{"id":"s1","aid":2,"role":"music","name":"Source two"}],"va":[],"developers":[],"relations":[]},"characters":[],"more":false,"fetched_at":crate::db::now()});save(&a,"exploration.v4.work.v1.1",&raw).unwrap();
  {let mut c=a.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Work','Work','v1')",[]).unwrap();change(&mut c,"v1",&Edit{revision:0,request_id:"forward".into(),corrections:vec![Correction{id:"hide".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:Some(1),role:"music".into(),note:"".into()}),link:None},Correction{id:"secret".into(),replaces:None,link:Some(Link{key:Key::Company{id:"p9".into()},name:"Secret studio".into(),character_name:None,spoiler:2})}]}).ok().unwrap();}
  for spoilers in [false,true]{let q=||Query(Page{page:1,refresh:false,spoilers,revision:None});let reference=reference_work(State(a.clone()),h.clone(),Path("v1".into()),q()).await.ok().unwrap().0;let local=work(State(a.clone()),h.clone(),Path("w".into()),q()).await.ok().unwrap().0;assert_eq!(reference,local);assert_eq!(reference["vn"]["staff"].as_array().unwrap().len(),1);assert_eq!(reference["vn"]["staff"][0]["aid"],2);assert_eq!(reference.to_string().contains("Secret studio"),spoilers);}
  assert_eq!(cached(&a,"exploration.v4.work.v1.1").unwrap().unwrap(),raw);
 }
}

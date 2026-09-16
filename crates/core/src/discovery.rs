//! Bounded, read-only metadata discovery. Does not add results to the collection.
use crate::{App, ApiError, exploration};
use axum::{extract::{Query, State}, http::HeaderMap, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone,Deserialize)]
pub struct Search {
    pub(crate) kind: String,
    pub(crate) value: String,
    pub(crate) target: String,
    #[serde(default="first")] pub(crate) page: u32,
    #[serde(default)] pub(crate) spoilers: bool,
    #[serde(default)] pub(crate) revision: Option<i64>,
}
fn first() -> u32 { 1 }

fn date_filter(value: &str) -> Result<Value, String> {
    let parts: Vec<_> = value.split('-').collect();
    if !(1..=3).contains(&parts.len()) || parts[0].len()!=4 || parts.iter().any(|p|!p.bytes().all(|b|b.is_ascii_digit())) || parts.iter().skip(1).any(|p|p.len()!=2) {
        return Err("Invalid release date".into());
    }
    let year: u32=parts[0].parse().map_err(|_|"Invalid release year")?;
    if !(1..=9998).contains(&year) { return Err("Invalid release year".into()); }
    let month=parts.get(1).map(|p|p.parse::<u32>().unwrap_or(0));
    if month.is_some_and(|m|!(1..=12).contains(&m)) { return Err("Invalid release month".into()); }
    if parts.len()==3 {
        let day=parts[2].parse::<u32>().unwrap_or(0);
        let days=match month.unwrap() { 2=>if year%4==0&&(year%100!=0||year%400==0){29}else{28},4|6|9|11=>30,_=>31 };
        if day==0||day>days { return Err("Invalid release day".into()); }
        return Ok(json!(["released","=",value]));
    }
    // Kana sorts partial dates after complete dates in the same month/year.
    let start=if month.is_some(){format!("{value}-01")}else{format!("{value}-01-01")};
    Ok(json!(["and",["released",">=",start],["released","<=",value]]))
}

pub(crate) fn request(search: &Search) -> Result<(&'static str, Value), String> {
    if search.page==0||search.page>1000 {return Err("Invalid search page".into());}
    if !matches!(search.target.as_str(),"works"|"characters") {return Err("Invalid search target".into());}
    let value=search.value.trim();
    let filter=match search.kind.as_str() {
        "trait" if exploration::valid_id(value,'i') => {
            let character=json!(["dtrait","=",[value,if search.spoilers{2}else{0}]]);
            if search.target=="works" {json!(["character","=",character])}else{character}
        },
        "studio" if search.target=="works" && !value.is_empty() && value.chars().count()<=300 && !value.chars().any(char::is_control) => json!(["developer","=",["search","=",value]]),
        "released" if search.target=="works" => date_filter(value)?,
        "title" if search.target=="works" && !value.is_empty() && value.chars().count()<=300 && !value.chars().any(char::is_control) => json!(["search","=",value]),
        _=>return Err("Invalid search condition".into()),
    };
    let (endpoint,fields,sort)=if search.target=="characters" {
        ("character","name,original,image.url,image.sexual,image.violence,vns.id,vns.title,vns.spoiler", "id")
    }else{
        ("vn","title,alttitle,released,image.url,image.sexual,image.violence,developers.id,developers.name",if search.kind=="title"{"searchrank"}else{"released"})
    };
    Ok((endpoint,json!({"filters":filter,"fields":fields,"sort":sort,"reverse":sort=="released","results":24,"page":search.page})))
}

// Provider membership remains separate from displayed local relationship metadata.
// Project after cache lookup so a saved decision applies without another provider fetch.
fn effective(a:&App,target:&str,mut value:Value,spoilers:bool)->Result<Json<Value>,ApiError>{
    let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
    let mut incomplete=false;
    if let Some(rows)=value.get_mut("results").and_then(Value::as_array_mut){for row in rows{
        let Some(id)=row["id"].as_str().map(str::to_owned)else{continue;};
        if target=="characters"{
            let wrapped=json!({"character":row.clone(),"works":[]});
            let mut wrapped=crate::relation_projection::visible_profile_page(&c,&wrapped,"character",&id,spoilers)?;
            *row=wrapped["character"].take();
            let mut after=None;let mut revision=None;
            for _ in 0..1000{
             let manual=crate::manual_profile::page(&c,"character",&id,after.as_deref(),revision,spoilers)?;revision=manual["revision"].as_i64();incomplete|=manual["partial"]==true;
             for work in manual["works"].as_array().into_iter().flatten(){
              let appearances=row.as_object_mut().ok_or_else(||ApiError("Invalid character search row".into()))?.entry("vns").or_insert_with(||json!([])).as_array_mut().ok_or_else(||ApiError("Invalid character appearances".into()))?;
              if !appearances.iter().any(|v|v["id"]==work["id"]){appearances.push(json!({"id":work["id"],"title":work["title"],"spoiler":work["profile_spoiler"],"local_relation":true}));}
             }
             after=manual["next"].as_str().map(str::to_string);if after.is_none(){break;}
            }
            if after.is_some(){incomplete=true;}

        }else{
            let wrapped=crate::relation_projection::work_page(&c,&json!({"vn":row.clone(),"characters":[]}),&id,spoilers)?;
            if let Some(developers)=wrapped["vn"].get("developers"){row["developers"]=developers.clone();}
        }
    }}
    if target=="characters"&&!spoilers{if let Some(rows)=value.get_mut("results").and_then(Value::as_array_mut){let old=rows.len();rows.retain(|r|r["vns"].as_array().is_some_and(|vs|vs.iter().any(|v|v["spoiler"]==0)));if old!=rows.len()&&value["more"]==true{value["continuation_verified"]=json!(true);}}}
    if incomplete{value["local_relationships_incomplete"]=json!(true);}
    crate::entity_overrides::project_names(&c,&mut value)?;Ok(Json(value))
}
// Exclusions belong inside the existential relationship predicate. Excluding a whole
// work would incorrectly remove works with another matching, retained relationship.
fn membership_request(input:&Search,work:&str,hidden:&crate::relation_overrides::Hidden)->Result<Option<Value>,String>{
 if input.target!="works"{return Ok(None);}
 let excluded=match input.kind.as_str(){"trait"=>&hidden.characters,"studio"=>&hidden.companies,_=>return Ok(None)};
 if excluded.is_empty(){return Ok(None);}
 let(_,mut body)=request(input)?;let relation=body["filters"][0].clone();let predicate=body["filters"][2].clone();
 let mut nested=vec![json!("and"),predicate];nested.extend(excluded.iter().map(|id|json!(["id","!=",id])));
 body["filters"]=json!(["and",["id","=",work],[relation,"=",nested]]);body["fields"]=json!("");body["results"]=json!(1);body["page"]=json!(1);body["sort"]=json!("id");body["reverse"]=json!(false);Ok(Some(body))
}
type Membership=(String,String,Value);
fn membership_batches(pending:Vec<Membership>)->Vec<Vec<Membership>>{
 let mut batches=Vec::new();let mut batch=Vec::new();let mut bytes=0;
 for item in pending{let size=item.2.to_string().len();if !batch.is_empty()&&(batch.len()>=8||bytes+size>16000){batches.push(std::mem::take(&mut batch));bytes=0;}bytes+=size;batch.push(item);}
 if !batch.is_empty(){batches.push(batch);}batches
}
fn batch_request(batch:&[Membership])->Value{
 let filters=if batch.len()==1{batch[0].2["filters"].clone()}else{let mut filters=vec![json!("or")];filters.extend(batch.iter().map(|item|item.2["filters"].clone()));json!(filters)};
 json!({"filters":filters,"fields":"","results":batch.len(),"page":1,"sort":"id","reverse":false})
}
fn batch_verdicts(batch:&[Membership],response:&Value)->Result<std::collections::BTreeSet<String>,ApiError>{
 let invalid=||ApiError("Incomplete relationship search response. Retry.".into());
 if response["more"]!=false{return Err(invalid());}
 let mut found=std::collections::BTreeSet::new();for row in response["results"].as_array().ok_or_else(invalid)?{
  let id=row["id"].as_str().ok_or_else(invalid)?;if !batch.iter().any(|item|item.0==id)||!found.insert(id.to_owned()){return Err(invalid());}
 }Ok(found)
}
pub async fn search(State(a):State<App>,h:HeaderMap,Query(input):Query<Search>)->Result<Json<Value>,ApiError>{
 crate::auth(&a,&h)?;request(&input)?;
 let epoch={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;crate::relation_correction_index::current_revision(&c,input.revision)?};
 let mut result=raw_search(State(a.clone()),h,Query(Search{kind:input.kind.clone(),value:input.value.clone(),target:input.target.clone(),page:input.page,spoilers:input.spoilers,revision:input.revision})).await?.0;
 if !result["results"].as_array().is_some_and(|rows|rows.len()<=24)||!result["more"].is_boolean(){return Err(ApiError("Invalid VNDB search response".into()));}
 let ids=result["results"].as_array().into_iter().flatten().filter_map(|row|row["id"].as_str().map(str::to_owned)).collect::<Vec<_>>();let mut remove=std::collections::BTreeSet::new();let mut observed=Vec::new();let mut pending=Vec::new();
 for id in ids{
  if input.target!="works"{continue;}
  let(revision,mut hidden)={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let(revision,mut hidden)=crate::relation_overrides::read(&c,&id)?;
   for edit in crate::relation_correction_store::read(&c,&id)?.1{match edit.replaces{Some(crate::relation_corrections::Key::Company{id})=>{hidden.companies.insert(id);},Some(crate::relation_corrections::Key::Character{id})=>{hidden.characters.insert(id);},_=>{}}} (revision,hidden)};
  // Trait membership is verified against actual appearances below, not only provider existence.
  if input.kind=="trait"{hidden.characters.clear();}
  observed.push((id.clone(),revision));let Some(body)=membership_request(&input,&id,&hidden)?else{continue;};
  let key=format!("discovery.membership.v1.{}",hex::encode(Sha256::digest(body.to_string().as_bytes())));
  let cached=exploration::cached(&a,&key)?.filter(|v|crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<3600);
  if let Some(keep)=cached.and_then(|v|v["keep"].as_bool()){if !keep{remove.insert(id);}}else{pending.push((id,key,body));}
 }
 for batch in membership_batches(pending){
  let mut gate=exploration::PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
  let mut missing=Vec::new();
  for item in batch{let keep=exploration::cached(&a,&item.1)?.filter(|v|crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<3600).and_then(|v|v["keep"].as_bool());if let Some(keep)=keep{if !keep{remove.insert(item.0);}}else{missing.push(item);}}
  if missing.is_empty(){continue;}
  let response=exploration::query("vn",batch_request(&missing),&mut gate).await?;let found=batch_verdicts(&missing,&response)?;
  for(id,key,_)in missing{let keep=found.contains(&id);exploration::save(&a,&key,&json!({"keep":keep,"fetched_at":crate::db::now()}))?;if !keep{remove.insert(id);}}
 }
 {let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;for(id,revision)in observed{if crate::relation_overrides::read(&c,&id)?.0!=revision{return Err(ApiError("Relationship decisions changed during search. Retry.".into()));}}}
 if input.target=="works"&&matches!(input.kind.as_str(),"trait"|"studio"){
  let ids=result["results"].as_array().into_iter().flatten().filter_map(|r|r["id"].as_str().map(str::to_string)).collect::<Vec<_>>();
  let verdict=crate::discovery_relations::memberships(&a,&input,&ids,input.kind=="trait").await?;
  if verdict.partial{result["partial"]=json!(true);}if verdict.stale{result["stale"]=json!(true);}if let Some(warning)=verdict.warning{result["warning"]=json!(warning);}
  if input.kind=="trait"{remove.extend(ids.into_iter().filter(|id|!verdict.matches.contains(id)));}else{remove.retain(|id|!verdict.matches.contains(id));}
 }
 {let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;crate::relation_correction_index::current_revision(&c,Some(epoch))?;}
 result["relationship_revision"]=json!(epoch);result["page"]=json!(input.page);
 if !remove.is_empty(){if let Some(rows)=result["results"].as_array_mut(){rows.retain(|row|!row["id"].as_str().is_some_and(|id|remove.contains(id)));}if result["more"]==true{result["continuation_verified"]=json!(true);}}
 Ok(Json(result))
}
async fn raw_search(State(a):State<App>,h:HeaderMap,Query(input):Query<Search>)->Result<Json<Value>,ApiError> {
    crate::auth(&a,&h)?;
    let (endpoint,body)=request(&input)?;
    let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes())));
    let previous=exploration::cached(&a,&key)?;
    let fresh=|v:&Value|crate::db::now()-v["fetched_at"].as_i64().unwrap_or(0)<3600;
    if let Some(v)=previous.as_ref().filter(|v|fresh(v)){return effective(&a,&input.target,v.clone(),input.spoilers);}
    let mut gate=exploration::PROVIDER.get_or_init(||tokio::sync::Mutex::new(0)).lock().await;
    if let Some(v)=exploration::cached(&a,&key)?.filter(fresh){return effective(&a,&input.target,v,input.spoilers);}
    match exploration::query(endpoint,body,&mut gate).await {
        Ok(mut result)=>{
            if !result["results"].is_array(){return Err(ApiError("Invalid VNDB search response".into()));}
            result["fetched_at"]=json!(crate::db::now());result["page"]=json!(input.page);
            exploration::save(&a,&key,&result)?;
            effective(&a,&input.target,result,input.spoilers)
        },
        Err(error)=>match previous {Some(value)=>effective(&a,&input.target,exploration::stale(value,error),input.spoilers),None=>Err(ApiError(error))},
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]fn batches_preserve_per_work_filters_and_reject_incomplete_verdicts(){
        let input=Search{kind:"trait".into(),value:"i1".into(),target:"works".into(),page:1,spoilers:false,revision:None};
        let rows=(1..=24).map(|i|{let id=format!("v{i}");let hidden=crate::relation_overrides::Hidden{characters:[format!("c{i}")].into(),..Default::default()};let body=membership_request(&input,&id,&hidden).unwrap().unwrap();(id,format!("key{i}"),body)}).collect::<Vec<_>>();
        let batches=membership_batches(rows);assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(),vec![8,8,8]);
        let batch=&batches[0];let request=batch_request(batch);assert_eq!(request["results"],8);assert_eq!(request["filters"][0],"or");for(i,item)in batch.iter().enumerate(){assert_eq!(request["filters"][i+1],item.2["filters"]);}
        assert_eq!(batch_verdicts(batch,&json!({"results":[{"id":"v2"},{"id":"v7"}],"more":false})).ok().unwrap(),["v2".into(),"v7".into()].into());
        for invalid in [json!({"results":[],"more":true}),json!({"results":[{"id":"v99"}],"more":false}),json!({"results":[{"id":"v2"},{"id":"v2"}],"more":false}),json!({"results":[],"more":null})]{assert!(batch_verdicts(batch,&invalid).is_err());}
        let large=(1..=8).map(|i|(format!("v{i}"),format!("key{i}"),json!({"filters":"x".repeat(9000)}))).collect();assert!(membership_batches(large).iter().all(|batch|batch.len()==1));
    }
    #[tokio::test]async fn membership_excludes_relationship_not_whole_work_and_reuses_verdicts(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        for(kind,field,prefix)in [("trait","character","c"),("studio","developer","p")]{
            let input=||Search{kind:kind.into(),value:if kind=="trait"{"i1".into()}else{"Studio".into()},target:"works".into(),page:1,spoilers:false,revision:None};
            let hidden=if kind=="trait"{crate::relation_overrides::Hidden{characters:["c1".into()].into(),..Default::default()}}else{crate::relation_overrides::Hidden{companies:["p1".into()].into(),..Default::default()}};
            let body=membership_request(&input(),"v1",&hidden).unwrap().unwrap();assert_eq!(body["filters"][2][0],field);assert_eq!(body["filters"][2][2][2],json!(["id","!=",format!("{prefix}1")]));assert_eq!(body["filters"][1],json!(["id","=","v1"]));
            let(endpoint,source)=request(&input()).unwrap();let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{source}").as_bytes())));let raw=json!({"results":[{"id":"v1"},{"id":"v2"}],"more":true,"page":1,"fetched_at":crate::db::now()});exploration::save(&a,&key,&raw).unwrap();
            {let mut c=a.db.lock().unwrap();let tx=c.transaction().unwrap();for id in ["v1","v2"]{let revision=crate::relation_overrides::read(&tx,id).ok().unwrap().0;crate::relation_overrides::change(&tx,id,&crate::relation_overrides::Edit{revision,request_id:format!("{kind}-{id}"),hidden:hidden.clone()}).ok().unwrap();}tx.commit().unwrap();}
            for(id,keep)in [("v1",false),("v2",true)]{let body=membership_request(&input(),id,&hidden).unwrap().unwrap();let key=format!("discovery.membership.v1.{}",hex::encode(Sha256::digest(body.to_string().as_bytes())));exploration::save(&a,&key,&json!({"keep":keep,"fetched_at":crate::db::now()})).unwrap();}
            if kind=="trait"{let(ep,body)=crate::discovery_relations::predicate(&input(),&["v1".into(),"v2".into()],&Default::default(),true).unwrap();let cache=crate::discovery_relations::cache_key(ep,&body);exploration::save(&a,&cache,&json!({"results":[{"id":"c1","vns":[{"id":"v1","spoiler":0},{"id":"v2","spoiler":0}]},{"id":"c2","vns":[{"id":"v2","spoiler":0}]}],"partial":false,"fetched_at":crate::db::now()})).unwrap();}
            let result=search(State(a.clone()),h.clone(),Query(input())).await.map_err(|e|e.0).unwrap().0;assert_eq!(result["results"],json!([{"id":"v2"}]));assert_eq!(result["continuation_verified"],true);assert_eq!(exploration::cached(&a,&key).unwrap().unwrap(),raw);
        }
    }
    #[tokio::test]async fn cached_search_projects_current_relationships_without_rewriting_source(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        let characters=json!({"results":[{"id":"c1","vns":[{"id":"v1","spoiler":0},{"id":"v2","spoiler":0}]}],"more":true,"page":1,"fetched_at":crate::db::now()});
        let works=json!({"results":[{"id":"v1","developers":[{"id":"p1","name":"Hidden"},{"id":"p2","name":"Retained"}]}],"more":true,"page":1,"fetched_at":crate::db::now()});
        let input=||Search{kind:"trait".into(),value:"i1".into(),target:"characters".into(),page:1,spoilers:false,revision:None};let(endpoint,body)=request(&input()).unwrap();let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes())));exploration::save(&a,&key,&characters).unwrap();
        {let mut c=a.db.lock().unwrap();let tx=c.transaction().unwrap();crate::relation_overrides::change(&tx,"v1",&crate::relation_overrides::Edit{revision:0,request_id:"search-hide".into(),hidden:crate::relation_overrides::Hidden{characters:["c1".into()].into(),companies:["p1".into()].into(),..Default::default()}}).ok().unwrap();tx.commit().unwrap();}
        let result=search(State(a.clone()),h.clone(),Query(input())).await.map_err(|e|e.0).unwrap().0;assert_eq!(result["results"][0]["vns"],json!([{"id":"v2","spoiler":0}]));assert_eq!(result["more"],true);assert_eq!(exploration::cached(&a,&key).unwrap().unwrap(),characters);
        let projected=effective(&a,"works",works.clone(),false).ok().unwrap().0;assert_eq!(projected["results"][0]["developers"],json!([{"id":"p2","name":"Retained"}]));assert_eq!(projected["more"],true);
        let stale=effective(&a,"characters",exploration::stale(characters.clone(),"fixture".into()),false).ok().unwrap().0;assert_eq!(stale["stale"],true);assert_eq!(stale["results"],result["results"]);
        {let mut c=a.db.lock().unwrap();let tx=c.transaction().unwrap();crate::relation_overrides::change(&tx,"v1",&crate::relation_overrides::Edit{revision:1,request_id:"search-restore".into(),hidden:Default::default()}).ok().unwrap();tx.commit().unwrap();}
        let mut expected=characters.clone();expected["relationship_revision"]=json!(2);assert_eq!(search(State(a.clone()),h,Query(input())).await.map_err(|e|e.0).unwrap().0,expected);assert_eq!(effective(&a,"works",works.clone(),false).ok().unwrap().0,works);
    }
    #[test] fn exact_traits_are_separate_from_names_and_respect_spoilers(){
        let mut input=Search{kind:"trait".into(),value:"i4".into(),target:"characters".into(),page:1,spoilers:false,revision:None};
        assert_eq!(request(&input).unwrap().1["filters"],json!(["dtrait","=",["i4",0]]));
        input.target="works".into();input.spoilers=true;
        assert_eq!(request(&input).unwrap().1["filters"],json!(["character","=",["dtrait","=",["i4",2]]]));
        input.value="Hair Black".into();assert!(request(&input).is_err());
        input.value="i4".into();input.page=0;assert!(request(&input).is_err());
    }
    #[test] fn studio_search_is_a_developer_filter(){
        let mut input=Search{kind:"studio".into(),value:"Purple software".into(),target:"works".into(),page:1,spoilers:false,revision:None};
        assert_eq!(request(&input).unwrap().1["filters"],json!(["developer","=",["search","=","Purple software"]]));
        input.target="characters".into();assert!(request(&input).is_err());
    }
    #[test] fn dates_preserve_precision_without_excluding_complete_dates(){
        assert_eq!(date_filter("2026-06-26").unwrap(),json!(["released","=","2026-06-26"]));
        assert_eq!(date_filter("2026-06").unwrap(),json!(["and",["released",">=","2026-06-01"],["released","<=","2026-06"]]));
        assert_eq!(date_filter("2026").unwrap(),json!(["and",["released",">=","2026-01-01"],["released","<=","2026"]]));
        for bad in ["TBA","2026-02-29","2026-13","2026-06-00","2026-6","0000", "2026-01-01-01"]{assert!(date_filter(bad).is_err(),"{bad}");}
        assert!(date_filter("2024-02-29").is_ok());
    }
    #[tokio::test] async fn requires_auth_and_reuses_cached_response(){
        let dir=tempfile::tempdir().unwrap();let a=crate::initialize(dir.path().into()).unwrap();
        let input=||Search{kind:"title".into(),value:"マガルミナ".into(),target:"works".into(),page:1,spoilers:false,revision:None};
        assert!(search(State(a.clone()),HeaderMap::new(),Query(input())).await.is_err());
        let (endpoint,body)=request(&input()).unwrap();
        let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes())));
        let value=json!({"results":[{"id":"v1","title":"One"}],"more":false,"fetched_at":crate::db::now()});
        exploration::save(&a,&key,&value).unwrap();
        let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
        let mut expected=value.clone();expected["relationship_revision"]=json!(0);expected["page"]=json!(1);assert_eq!(search(State(a.clone()),h,Query(input())).await.map_err(|e|e.0).unwrap().0,expected);
        assert_eq!(a.db.lock().unwrap().query_row("SELECT COUNT(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
}

#[cfg(test)]mod exact_search_tests{
 use super::*;use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};
 fn save_source(a:&App,input:&Search,raw:&Value)->String{let(endpoint,body)=request(input).unwrap();let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes())));exploration::save(a,&key,raw).unwrap();key}
 fn auth(a:&App)->HeaderMap{let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());h}
 #[tokio::test]async fn exact_company_rebind_rechecks_predicate_and_rejects_old_epoch(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let input=Search{kind:"studio".into(),value:"Studio".into(),target:"works".into(),page:1,spoilers:false,revision:None};
  let raw=json!({"results":[{"id":"v1","title":"Generated work","developers":[{"id":"p1","name":"Original Studio"}]}],"more":false,"fetched_at":crate::db::now()});let key=save_source(&a,&input,&raw);
  {let mut c=a.db.lock().unwrap();change(&mut c,"v1",&Edit{revision:0,request_id:"rebind".into(),corrections:vec![Correction{id:"one".into(),replaces:Some(Key::Company{id:"p1".into()}),link:Some(Link{key:Key::Company{id:"p2".into()},name:"Correct Studio".into(),character_name:None,spoiler:0})}]}).map_err(|e|e.0).unwrap();}
  let excluded=crate::relation_overrides::Hidden{companies:["p1".into()].into(),..Default::default()};let body=membership_request(&input,"v1",&excluded).unwrap().unwrap();let verdict=format!("discovery.membership.v1.{}",hex::encode(Sha256::digest(body.to_string().as_bytes())));exploration::save(&a,&verdict,&json!({"keep":false,"fetched_at":crate::db::now()})).unwrap();
  let(endpoint,body)=crate::discovery_relations::predicate(&input,&["v1".into()],&["p2".into()].into(),false).unwrap();let local=crate::discovery_relations::cache_key(endpoint,&body);exploration::save(&a,&local,&json!({"results":[{"id":"p2"}],"partial":false,"fetched_at":crate::db::now()})).unwrap();
  let result=search(State(a.clone()),auth(&a),Query(input.clone())).await.map_err(|e|e.0).unwrap().0;assert_eq!(result["results"].as_array().unwrap().len(),1);assert_eq!(result["results"][0]["developers"][0]["id"],"p2");assert!(!result.to_string().contains("Original Studio"));assert_eq!(result["relationship_revision"],1);
  exploration::save(&a,&local,&json!({"results":[],"partial":false,"fetched_at":crate::db::now()})).unwrap();assert_eq!(search(State(a.clone()),auth(&a),Query(input.clone())).await.map_err(|e|e.0).unwrap().0["results"],json!([]));
  {let mut c=a.db.lock().unwrap();change(&mut c,"v1",&Edit{revision:1,request_id:"undo".into(),corrections:vec![]}).map_err(|e|e.0).unwrap();}
  let mut old=input.clone();old.revision=Some(1);old.page=2;assert!(search(State(a.clone()),auth(&a),Query(old)).await.is_err());let restored=search(State(a.clone()),auth(&a),Query(input)).await.map_err(|e|e.0).unwrap().0;assert_eq!(restored["results"],raw["results"]);assert_eq!(exploration::cached(&a,&key).unwrap().unwrap(),raw);
 }
 #[tokio::test]async fn character_search_adds_manual_appearances_and_hides_removed_and_secret_works(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let input=Search{kind:"trait".into(),value:"i1".into(),target:"characters".into(),page:1,spoilers:false,revision:None};let raw=json!({"results":[{"id":"c1","name":"Generated character","vns":[{"id":"v1","spoiler":0}]}],"more":true,"page":1,"fetched_at":crate::db::now()});let key=save_source(&a,&input,&raw);
  {let mut c=a.db.lock().unwrap();for(work,spoiler)in [("v1",None),("v2",Some(0)),("v3",Some(2))]{change(&mut c,work,&Edit{revision:0,request_id:work.into(),corrections:vec![Correction{id:"one".into(),replaces:if spoiler.is_none(){Some(Key::Character{id:"c1".into()})}else{None},link:spoiler.map(|spoiler|Link{key:Key::Character{id:"c1".into()},name:"Generated character".into(),character_name:None,spoiler})}]}).map_err(|e|e.0).unwrap();}c.execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Manual appearance','Manual appearance','v2')",[]).unwrap();}
  let result=search(State(a.clone()),auth(&a),Query(input.clone())).await.map_err(|e|e.0).unwrap().0;assert_eq!(result["results"][0]["vns"],json!([{"id":"v2","title":"Manual appearance","spoiler":0,"local_relation":true}]));assert!(!result.to_string().contains("v1"));assert!(!result.to_string().contains("v3"));
  {let mut c=a.db.lock().unwrap();change(&mut c,"v2",&Edit{revision:1,request_id:"undo".into(),corrections:vec![]}).map_err(|e|e.0).unwrap();}
  let result=search(State(a.clone()),auth(&a),Query(input)).await.map_err(|e|e.0).unwrap().0;assert_eq!(result["results"],json!([]));assert_eq!(result["continuation_verified"],true);assert_eq!(exploration::cached(&a,&key).unwrap().unwrap(),raw);
 }
}

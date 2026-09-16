//! Typed cached relations shared by smart facts and the editor's value picker.
use crate::smart_rules::{Field,KnownSet};
use serde_json::{Value,json};
use std::collections::BTreeMap;
use rusqlite::OptionalExtension;

#[derive(Default)]
pub struct Projection{
 pub fields:BTreeMap<Field,KnownSet>,
 pub options:BTreeMap<String,BTreeMap<String,String>>,
 pub valid_until:Option<i64>,
}
fn field_key(field:Field)->String{serde_json::to_value(field).unwrap().as_str().unwrap().into()}
fn add(out:&mut Projection,field:Field,id:&str,name:&str){
 out.fields.entry(field).or_default().values.insert(id.into());
 if !name.trim().is_empty(){out.options.entry(field_key(field)).or_default().entry(id.into()).or_insert_with(||name.into());}
}
fn id(v:&Value,prefix:char)->Option<&str>{v["id"].as_str().filter(|id|crate::exploration::valid_id(id,prefix))}
fn local_visible(v:&Value)->bool{v["local_relation"]!=true||v["spoiler"].as_u64()==Some(0)}
fn visible(v:&Value,work:&str)->bool{
 v["vns"].as_array().is_some_and(|rows|rows.iter().any(|v|v["id"]==work&&v["spoiler"].as_i64()==Some(0)))
}
pub(crate) fn project_local(c:&rusqlite::Connection,value:&Value,work:&str,now:i64)->Result<Projection,crate::ApiError>{
 let mut effective=crate::relation_projection::effective(c,value,work).map_err(crate::ApiError)?;crate::entity_overrides::project_names(c,&mut effective)?;Ok(project(&effective,work,now))
}
pub(crate) fn load_local(c:&rusqlite::Connection,work:&str,now:i64)->Result<Projection,crate::ApiError>{
 let raw:Option<String>=c.prepare_cached("SELECT value FROM settings WHERE key=?1")?.query_row([format!("exploration.v4.work.{work}.1")],|r|r.get(0)).optional()?;
 if let Some(value)=raw.and_then(|s|serde_json::from_str::<Value>(&s).ok()){return project_local(c,&value,work,now);}
 if crate::relation_correction_store::read(c,work)?.1.is_empty(){return Ok(Projection::default());}
 project_local(c,&json!({"vn":{"id":work}}),work,now)
}
pub fn project(v:&Value,work:&str,now:i64)->Projection{
 let mut out=Projection::default();
 let expiry=v["fetched_at"].as_i64().filter(|t|*t<=now).and_then(|t|t.checked_add(86400)).filter(|t|*t>now);
 let fresh=expiry.is_some()&&v["partial"]!=true&&v["stale"]!=true&&v["local_relationships_incomplete"]!=true;
 if let Some(rows)=v["vn"]["developers"].as_array(){
  let mut complete=fresh;
  for row in rows{if let Some(id)=id(row,'p').filter(|_|local_visible(row)){add(&mut out,Field::Brands,id,row["name"].as_str().unwrap_or(""));}else{complete=false;}}
  out.fields.entry(Field::Brands).or_default().complete=complete;
 }
 if let Some(rows)=v["vn"]["tags"].as_array(){
  let mut complete=fresh;
  for row in rows{
   if let Some(id)=id(row,'g').filter(|_|row["spoiler"].as_i64()==Some(0)&&row["lie"].as_bool()==Some(false)){
    add(&mut out,Field::SourceTags,id,row["name"].as_str().unwrap_or(""));
   }else{complete=false;}
  }
  out.fields.entry(Field::SourceTags).or_default().complete=complete;
 }
 let mut characters_complete=fresh&&v["more"].as_bool()==Some(false);
 let mut visible_characters=std::collections::BTreeSet::new();
 if let Some(rows)=v["characters"].as_array(){
  for row in rows{if let Some(id)=id(row,'c').filter(|_|visible(row,work)){
   visible_characters.insert(id.to_string());add(&mut out,Field::Characters,id,row["name"].as_str().unwrap_or(""));
  }else{characters_complete=false;}}
  out.fields.entry(Field::Characters).or_default().complete=characters_complete;
 }else{characters_complete=false;}
 let mut people_complete=fresh;let mut roles_complete=fresh;
 if let Some(rows)=v["vn"]["staff"].as_array(){
  for row in rows{
   if let Some(id)=id(row,'s').filter(|_|local_visible(row)){let name=row["display_name"].as_str().or_else(||row["name"].as_str()).unwrap_or("");add(&mut out,Field::People,id,name);
    if let Some(role)=row["role"].as_str().filter(|r|!r.is_empty()&&r.len()<=40&&r.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_')){
     add(&mut out,Field::PersonRoles,&format!("{id}:{role}"),&format!("{name} · {role}"));
    }else{roles_complete=false;}
   }else{people_complete=false;roles_complete=false;}
  }
 }else{people_complete=false;roles_complete=false;}
 if let Some(rows)=v["vn"]["va"].as_array(){
  for row in rows{
   let character=row["character"]["id"].as_str();
   if local_visible(row)&&character.is_some_and(|id|visible_characters.contains(id)){
    if let Some(id)=id(&row["staff"],'s'){let name=row["staff"]["display_name"].as_str().or_else(||row["staff"]["name"].as_str()).unwrap_or("");add(&mut out,Field::People,id,name);add(&mut out,Field::PersonRoles,&format!("{id}:voice"),&format!("{name} · voice"));}
    else{people_complete=false;roles_complete=false;}
   }else{people_complete=false;roles_complete=false;}
  }
 }else{people_complete=false;roles_complete=false;}
 // Hidden/missing characters can conceal a voice credit. Do not prove absence.
 out.fields.entry(Field::People).or_default().complete=people_complete&&characters_complete;
 out.fields.entry(Field::PersonRoles).or_default().complete=roles_complete&&characters_complete;
 if out.fields.values().any(|set|set.complete){out.valid_until=expiry;}
 out
}
pub async fn options(axum::extract::State(a):axum::extract::State<crate::App>,h:axum::http::HeaderMap)->crate::Result<Value>{
 crate::auth(&a,&h)?;
 let c=a.db.lock().map_err(|e|crate::ApiError(e.to_string()))?;
 let mut query=c.prepare("SELECT vndb_id FROM works WHERE merged_into IS NULL AND vndb_id IS NOT NULL UNION SELECT substr(e.work_key,6) FROM list_entries e JOIN curated_lists l ON l.id=e.list_id WHERE l.deleted=0 AND e.work_key LIKE 'vndb:%' ORDER BY 1")?;
 let ids=query.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let mut options:BTreeMap<String,BTreeMap<String,String>>=BTreeMap::new();
 for work in ids{
  if !crate::exploration::valid_id(&work,'v'){continue;}
  for(field,values)in load_local(&c,&work,crate::db::now())?.options{options.entry(field).or_default().extend(values);}
 }
 Ok(axum::Json(json!(options.into_iter().map(|(field,values)|{
  let mut values=values.into_iter().map(|(id,name)|json!({"id":id,"name":name})).collect::<Vec<_>>();
  values.sort_by_key(|v|v["name"].as_str().unwrap_or("").to_lowercase());(field,values)
 }).collect::<BTreeMap<_,_>>())))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn stable_ids_roles_hidden_credits_and_completeness(){
  let mut v=json!({"fetched_at":100,"more":false,"vn":{"developers":[{"id":"p1","name":"Studio"}],"staff":[{"id":"s1","name":"Alice","role":"scenario"},{"id":"s2","name":"Bob","role":"art"}],"va":[{"staff":{"id":"s3","name":"Carol"},"character":{"id":"c1"}}]},"characters":[{"id":"c1","name":"Hero","vns":[{"id":"v1","spoiler":0}]}]});
  let p=project(&v,"v1",101);assert!(p.fields[&Field::People].complete);
  assert!(p.fields[&Field::PersonRoles].values.contains("s1:scenario"));assert!(!p.fields[&Field::PersonRoles].values.contains("s1:art"));
  assert!(p.fields[&Field::People].values.contains("s3"));assert_eq!(p.valid_until,Some(86500));
  v["characters"][0]["vns"][0]["spoiler"]=json!(1);
  let p=project(&v,"v1",101);assert!(!p.fields[&Field::Characters].complete);assert!(!p.fields[&Field::People].values.contains("s3"));assert!(!p.options.contains_key("characters"));assert!(!p.fields[&Field::People].complete);
  v["characters"][0]["vns"][0]["spoiler"]=json!(0);v["more"]=json!(true);
  assert!(!project(&v,"v1",101).fields[&Field::Characters].complete);
  let p=project(&v,"v1",86500);assert!(!p.fields[&Field::Brands].complete);assert!(p.fields[&Field::Brands].values.contains("p1"));
 }
 #[test]fn source_tags_use_ids_and_never_infer_hidden_or_legacy_absence(){
  let mut v=json!({"fetched_at":100,"vn":{"tags":[{"id":"g1","name":"Same name","spoiler":0,"lie":false},{"id":"g2","name":"Same name","spoiler":0,"lie":false}]}});
  let p=project(&v,"v1",101);assert!(p.fields[&Field::SourceTags].complete);assert_eq!(p.fields[&Field::SourceTags].values.len(),2);assert_eq!(p.options["source_tags"].len(),2);
  v["vn"]["tags"][1]["spoiler"]=json!(1);
  let p=project(&v,"v1",101);assert!(!p.fields[&Field::SourceTags].complete);assert!(!p.fields[&Field::SourceTags].values.contains("g2"));assert!(!p.options["source_tags"].contains_key("g2"));
  v["vn"]["tags"][0]["lie"]=json!(true);assert!(project(&v,"v1",101).fields[&Field::SourceTags].values.is_empty());
  v["vn"]["tags"]=json!([]);assert!(project(&v,"v1",101).fields[&Field::SourceTags].complete);
  assert!(!project(&v,"v1",86500).fields[&Field::SourceTags].complete);
  v["vn"]["tags"]=json!(["Legacy name"]);assert!(!project(&v,"v1",101).fields[&Field::SourceTags].complete);
  v["vn"].as_object_mut().unwrap().remove("tags");assert!(!project(&v,"v1",101).fields.contains_key(&Field::SourceTags));
 }
 #[test]fn malformed_and_missing_arrays_never_prove_absence(){
  let v=json!({"fetched_at":100,"more":false,"vn":{"developers":[{"id":"not-an-id","name":"Bad"}]}});
  let p=project(&v,"v1",101);assert!(!p.fields[&Field::Brands].complete);assert!(p.fields[&Field::Brands].values.is_empty());assert!(!p.fields[&Field::People].complete);
  let v=json!({"fetched_at":100,"more":false,"vn":{"developers":[],"staff":[],"va":[]},"characters":[]});
  assert!(project(&v,"v1",101).fields.values().all(|set|set.complete));
 }
}

#[cfg(test)]mod exact_tests{
 use super::*;use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};
 #[test]fn exact_local_relations_feed_facts_tags_and_unknown_without_cache(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  let link=|id:&str,spoiler|Correction{id:id.into(),replaces:None,link:Some(Link{key:Key::Staff{id:id.into(),aid:Some(5),role:"music".into(),note:"".into()},name:format!("Credit {id}"),character_name:None,spoiler})};
  let input=Edit{revision:0,request_id:"exact-facts".into(),corrections:vec![link("s1",0),link("s2",2)]};
  let before:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();change(&mut c,"v1",&input).ok().unwrap();let changed:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();assert!(changed>before);change(&mut c,"v1",&input).ok().unwrap();assert_eq!(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),changed);
  let projection=load_local(&c,"v1",100).ok().unwrap();assert!(projection.fields[&Field::People].values.contains("s1"));assert!(!projection.fields[&Field::People].values.contains("s2"));assert!(!projection.fields[&Field::People].complete);assert!(!projection.options["people"].contains_key("s2"));
  c.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Generated tag','generated tag'),('hidden','Hidden tag','hidden tag'); INSERT INTO custom_tag_entities VALUES('tag','s1'),('hidden','s2');").unwrap();let tags=crate::entity_tags::related(&c,&projection.fields).ok().unwrap();assert_eq!(tags.values,["tag".into()].into());assert!(!tags.complete);
  let raw=json!({"vn":{"id":"v1","staff":[],"va":[],"developers":[],"tags":[]},"characters":[],"more":false,"fetched_at":100});let projection=project_local(&c,&raw,"v1",101).ok().unwrap();assert!(!projection.fields[&Field::People].complete);assert!(projection.fields[&Field::Brands].complete);assert_eq!(projection.fields[&Field::People].values,["s1".into()].into());
 }
 #[test]fn schema37_upgrade_invalidates_old_smart_results_once(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();let before;{let c=db.lock().unwrap();c.execute_batch("DROP TRIGGER smart_dirty_relation_corrections_INSERT; DROP TRIGGER smart_dirty_relation_corrections_UPDATE; DROP TRIGGER smart_dirty_relation_corrections_DELETE; PRAGMA user_version=37;").unwrap();before=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap();}drop(db);
  let db=crate::db::open(&path).unwrap();{let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),before+1);assert_eq!(c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name LIKE 'smart_dirty_relation_corrections_%'",[],|r|r.get::<_,i64>(0)).unwrap(),3);}drop(db);
  let db=crate::db::open(&path).unwrap();assert_eq!(db.lock().unwrap().query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),before+1);
 }
}

#[cfg(test)]mod exact_snapshot_tests{
 use super::*;use crate::relation_corrections::{Correction,Key,Link};use crate::relation_correction_store::{change,Edit};
 #[test]fn exact_edits_recompute_related_tag_snapshots_and_preserve_pinned_results(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"related_personal_tags","mode":"any","values":["tag"]}}});
  c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('list','Generated','',1,?1)",[definition.to_string()]).unwrap();
  c.execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Work','Work','v1'); INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Generated','generated'); INSERT INTO custom_tag_entities VALUES('tag','s1');").unwrap();
  let stop=std::sync::atomic::AtomicBool::new(false);assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());let empty=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(empty["total"],0);assert_eq!(empty["unknown_count"],1);
  let edits=|spoiler|vec![Correction{id:"credit".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:Some(1),role:"music".into(),note:"".into()},name:"Generated alias".into(),character_name:None,spoiler})}];
  change(&mut c,"v1",&Edit{revision:0,request_id:"add".into(),corrections:edits(0)}).ok().unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());let saved=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(saved["total"],1);assert_eq!(saved["unknown_count"],0);
  change(&mut c,"v1",&Edit{revision:1,request_id:"hide-spoiler".into(),corrections:edits(2)}).ok().unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());let hidden=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(hidden["total"],0);assert_eq!(hidden["unknown_count"],1);
  let pinned=crate::smart_snapshots::page(&c,"list",&crate::smart_snapshots::Page{snapshot:saved["snapshot"].as_str().map(str::to_string),after:None}).ok().unwrap();assert_eq!(pinned["total"],1);assert_eq!(pinned["newer_available"],true);
  change(&mut c,"v1",&Edit{revision:2,request_id:"undo".into(),corrections:edits(0)}).ok().unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());assert_eq!(crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap()["total"],1);
 }
}

//! Catalog projection for smart rules. No network or file operations; root observations belong to the caller's snapshot.
use crate::smart_rules::{WorkFacts,EditionFacts,KnownSet,Field,EditionField};
use rusqlite::{Connection,OptionalExtension,params};
use std::collections::{BTreeMap,BTreeSet};
use serde_json::Value;
type R<T>=Result<T,String>;
#[derive(Default)]pub(crate) struct Needs{fields:BTreeSet<Field>,editions:bool}
impl Needs{pub(crate) fn for_rule(rule:&crate::smart_rules::Rule)->Self{
 fn visit(rule:&crate::smart_rules::Rule,n:&mut Needs){use crate::smart_rules::Rule;match rule{Rule::Group{children,..}=>for child in children{visit(child,n)},Rule::Field{predicate}=>{n.fields.insert(predicate.field);},Rule::Edition{..}=>n.editions=true,Rule::Year{..}=>{}}}
 let mut n=Self::default();visit(rule,&mut n);n
}}
fn known(values:impl IntoIterator<Item=String>,complete:bool)->KnownSet{KnownSet{values:values.into_iter().collect(),complete}}
fn recorded(raw:&str)->KnownSet{let values:Option<Vec<String>>=serde_json::from_str(raw).ok();let complete=values.as_ref().is_some_and(|v|!v.is_empty());known(values.unwrap_or_default(),complete)}
pub(crate) fn reference(c:&Connection,key:&str,now:i64)->R<WorkFacts>{
 let mut facts=WorkFacts::default();
 let Some(id)=key.strip_prefix("vndb:").filter(|id|crate::exploration::valid_id(id,'v'))else{return Ok(facts);};
 let raw:Option<String>=c.prepare_cached("SELECT value FROM settings WHERE key=?1").map_err(|e|e.to_string())?.query_row([format!("exploration.v4.work.{id}.1")],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 if let Some(value)=raw.and_then(|raw|serde_json::from_str::<Value>(&raw).ok()){

  let expiry=value["fetched_at"].as_i64().filter(|t|*t<=now).and_then(|t|t.checked_add(86400)).filter(|t|*t>now);
  if value["partial"]!=true&&value["stale"]!=true&&expiry.is_some(){
   facts.year=value["vn"]["released"].as_str().and_then(|s|s.get(..4)).filter(|s|s.bytes().all(|b|b.is_ascii_digit())).and_then(|s|s.parse::<u16>().ok()).filter(|y|*y>0&&*y<9999);
   if facts.year.is_some(){facts.valid_until=expiry;}
  }
 }
 let projection=crate::smart_relations::load_local(c,id,now).map_err(|e|e.0)?;facts.fields=projection.fields;facts.valid_until=match(facts.valid_until,projection.valid_until){(Some(a),Some(b))=>Some(a.min(b)),(a,b)=>a.or(b)};
 facts.fields.insert(Field::RelatedPersonalTags,crate::entity_tags::related(c,&facts.fields).map_err(|e|e.0)?);
 Ok(facts)
}
pub fn load(c:&Connection,id:&str,root_online:&BTreeMap<String,bool>)->R<WorkFacts>{
 load_at(c,id,root_online,crate::db::now())
}
pub(crate) fn load_at(c:&Connection,id:&str,root_online:&BTreeMap<String,bool>,now:i64)->R<WorkFacts>{
 load_needed_at(c,id,root_online,now,None)
}
pub(crate) fn load_needed_at(c:&Connection,id:&str,root_online:&BTreeMap<String,bool>,now:i64,needs:Option<&Needs>)->R<WorkFacts>{
 let wants=|field|needs.map_or(true,|n|n.fields.contains(&field));
 let (status,released,vndb,favorite):(String,String,Option<String>,i64)=c.prepare_cached("SELECT status,released,vndb_id,favorite FROM works WHERE id=?1 AND merged_into IS NULL").map_err(|e|e.to_string())?.query_row([id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
 let mut work=WorkFacts::default();work.fields.insert(Field::Status,known([status],true));work.fields.insert(Field::MatchingState,known([if vndb.as_ref().is_some_and(|v|!v.is_empty()){"matched"}else{"unmatched"}.into()],true));
 work.fields.insert(Field::Favorite,if matches!(favorite,0|1){known([if favorite==1{"true".into()}else{"false".into()}],true)}else{KnownSet::default()});
 if wants(Field::OrganizingState){work.fields.insert(Field::OrganizingState,crate::smart_organization::load(c,id)?);}
 work.year=released.get(..4).filter(|v|v.bytes().all(|b|b.is_ascii_digit())).and_then(|v|v.parse::<u16>().ok()).filter(|v|*v>0&&*v<9999);
 if wants(Field::PersonalTags){let mut q=c.prepare_cached("WITH RECURSIVE ancestors(id) AS (SELECT ?1 UNION SELECT w.id FROM works w JOIN ancestors a ON w.merged_into=a.id) SELECT DISTINCT t.id FROM custom_tags t JOIN custom_tag_works m ON m.tag_id=t.id WHERE t.deleted=0 AND m.work_id IN (SELECT id FROM ancestors)").map_err(|e|e.to_string())?;
 let tags=q.query_map([id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;work.fields.insert(Field::PersonalTags,known(tags,true));}
 if wants(Field::SourceLocations){let mut q=c.prepare_cached("SELECT DISTINCT r.root_id FROM resource_bindings b JOIN resources r ON r.id=b.resource_id WHERE b.work_id=?1").map_err(|e|e.to_string())?;let roots=q.query_map([id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;work.fields.insert(Field::SourceLocations,known(roots,true));}
 // The editable developer/tag fields currently contain names, not stable provider IDs.
 // Only cached typed developer IDs may satisfy brand identity rules.
 if [Field::RelatedPersonalTags,Field::SourceTags,Field::Brands,Field::People,Field::PersonRoles,Field::Characters].into_iter().any(wants){if let Some(vndb)=vndb{let projection=crate::smart_relations::load_local(c,&vndb,now).map_err(|e|e.0)?;work.valid_until=projection.valid_until;work.fields.extend(projection.fields);}}
 if wants(Field::RelatedPersonalTags){work.fields.insert(Field::RelatedPersonalTags,crate::entity_tags::related(c,&work.fields).map_err(|e|e.0)?);}
 if needs.is_some_and(|n|!n.editions){return Ok(work);}
 let mut q=c.prepare_cached("SELECT e.id,e.languages,e.platforms FROM releases e JOIN work_releases w ON w.release_id=e.id WHERE w.work_id=?1 ORDER BY e.id").map_err(|e|e.to_string())?;
 let editions=q.query_map([id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 for (edition,languages,platforms) in editions{
  let mut facts=EditionFacts::default();facts.fields.insert(EditionField::ConfirmedLanguages,recorded(&languages));facts.fields.insert(EditionField::Platforms,recorded(&platforms));
  let mut q=c.prepare_cached("SELECT r.root_id,count(f.id),coalesce(sum(f.availability='missing'),0),coalesce(sum(f.availability='unverified'),0),coalesce(sum(f.availability NOT IN ('present','missing','unverified')),0) FROM resource_bindings b JOIN resources r ON r.id=b.resource_id LEFT JOIN resource_files rf ON rf.resource_id=r.id LEFT JOIN files f ON f.id=rf.file_id WHERE b.work_id=?1 AND b.release_id=?2 AND b.role='main' GROUP BY r.id ORDER BY r.id").map_err(|e|e.to_string())?;
  let rows=q.query_map(params![id,edition],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
  let mut states=BTreeSet::new();let mut complete=!rows.is_empty();for(root,count,missing,unverified,other)in rows{if count==0||other>0{complete=false;}if missing>0{states.insert("missing".into());}if unverified>0{states.insert("unverified".into());}match root_online.get(&root){Some(false)=>{states.insert("offline".into());},Some(true)=>{if count>0&&missing==0&&unverified==0&&other==0{states.insert("available".into());}},None=>complete=false}}
  facts.fields.insert(EditionField::Availability,KnownSet{values:states,complete});work.editions.push(facts);
 }
 work.editions_complete=true;Ok(work)
}
#[cfg(test)]mod tests{
 use super::*;use crate::smart_rules::{Rule,Join,SetMode,Predicate,Truth,evaluate_checked};
 #[test]fn catalog_language_and_main_resource_availability_stay_on_same_edition(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','W','W'); INSERT INTO roots(id,path,label) VALUES('root','generated','Root'); INSERT INTO releases(id,label,languages) VALUES('zh','Chinese','[\"zh\"]'),('en','English','[\"en\"]'); INSERT INTO work_releases VALUES('w','zh'),('w','en'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('rz','root','z','Z','archive'),('re','root','e','E','archive'); INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES('rz','w','zh','main'),('re','w','en','main'); INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES('fz','root','z','z',1,'1','zip','fixture','missing'),('fe','root','e','e',1,'1','zip','fixture','present'); INSERT INTO resource_files VALUES('rz','fz'),('re','fe');").unwrap();
 let rule=Rule::Edition{join:Join::All,conditions:vec![Predicate{field:EditionField::ConfirmedLanguages,mode:SetMode::Any,values:vec!["zh".into()],exclude:false,include_unknown:false},Predicate{field:EditionField::Availability,mode:SetMode::Any,values:vec!["available".into()],exclude:false,include_unknown:false}],exclude:false,include_unknown:false};let online=BTreeMap::from([("root".into(),true)]);
 assert_eq!(evaluate_checked(&rule,&load(&c,"w",&online).unwrap()).unwrap(),Truth::No);
 c.execute("UPDATE files SET availability='present' WHERE id='fz'",[]).unwrap();assert_eq!(evaluate_checked(&rule,&load(&c,"w",&online).unwrap()).unwrap(),Truth::Yes);
 c.execute("UPDATE resource_bindings SET role='patch' WHERE resource_id='rz'",[]).unwrap();assert_eq!(evaluate_checked(&rule,&load(&c,"w",&online).unwrap()).unwrap(),Truth::Unknown);
 c.execute("UPDATE resource_bindings SET role='main' WHERE resource_id='rz'",[]).unwrap();assert_eq!(evaluate_checked(&rule,&load(&c,"w",&BTreeMap::from([("root".into(),false)])).unwrap()).unwrap(),Truth::No);
 }
 #[test]fn unavailable_identity_fields_and_unrecorded_languages_remain_unknown(){
 let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,developer,tags) VALUES('w','W','W','Names are not IDs','[\"Name\"]'); INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag'); INSERT INTO custom_tag_works VALUES('tag','w'); INSERT INTO releases(id,label) VALUES('edition','Unknown'); INSERT INTO work_releases VALUES('w','edition');").unwrap();let facts=load(&c,"w",&BTreeMap::new()).unwrap();assert!(facts.fields[&Field::PersonalTags].values.contains("tag"));assert!(!facts.fields.contains_key(&Field::People));assert!(!facts.fields.contains_key(&Field::SourceTags));assert!(!facts.fields.contains_key(&Field::Brands));assert!(!facts.editions[0].fields[&EditionField::ConfirmedLanguages].complete);
 c.execute("UPDATE custom_tags SET deleted=1 WHERE id='tag'",[]).unwrap();assert!(load(&c,"w",&BTreeMap::new()).unwrap().fields[&Field::PersonalTags].values.is_empty());
 }
}

#[cfg(test)]mod favorite_tests{
 use super::*;use serde_json::json;use crate::smart_rules::{Rule,Truth,evaluate_checked,Field};
 #[test]fn favorite_is_independent_and_unknown_for_external_references(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,status,favorite) VALUES('yes','Yes','Yes','backlog',1),('no','No','No','completed',0);").unwrap();
  let rule=|mode:&str,value:&str|serde_json::from_value::<Rule>(json!({"kind":"field","predicate":{"field":"favorite","mode":mode,"values":[value]}})).unwrap();
  for(id,expected)in [("yes",Truth::Yes),("no",Truth::No)]{assert_eq!(evaluate_checked(&rule("any","true"),&load(&c,id,&Default::default()).unwrap()).unwrap(),expected);}
  assert_eq!(evaluate_checked(&rule("any","false"),&load(&c,"no",&Default::default()).unwrap()).unwrap(),Truth::Yes);
  assert_eq!(evaluate_checked(&rule("none","true"),&reference(&c,"vndb:v1",100).unwrap()).unwrap(),Truth::Unknown);
  assert!(crate::smart_rules::validate(&rule("any","favourite")).is_err());
  c.execute("UPDATE works SET favorite=7 WHERE id='no'",[]).unwrap();assert_eq!(evaluate_checked(&rule("any","false"),&load(&c,"no",&Default::default()).unwrap()).unwrap(),Truth::Unknown);
 }
 #[test]fn favorite_changes_recompute_but_pinned_snapshots_and_backup_keep_their_state(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();{
   let mut c=db.lock().unwrap();let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"favorite","mode":"any","values":["true"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('list','Favorites','',1,?1)",[definition.to_string()]).unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,status,favorite) VALUES('a','A','A','backlog',1),('b','B','B','completed',0);").unwrap();
   let stop=std::sync::atomic::AtomicBool::new(false);assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());let first=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(first["entries"][0]["work_key"],"local:a");
   c.execute_batch("UPDATE works SET favorite=0 WHERE id='a'; UPDATE works SET favorite=1 WHERE id='b';").unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());let second=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(second["entries"][0]["work_key"],"local:b");assert_eq!(second["unknown_count"],0);
   let old=crate::smart_snapshots::page(&c,"list",&crate::smart_snapshots::Page{snapshot:first["snapshot"].as_str().map(str::to_owned),after:None}).ok().unwrap();assert_eq!(old["entries"][0]["work_key"],"local:a");assert_eq!(old["newer_available"],true);
   assert_eq!(c.query_row("SELECT status FROM works WHERE id='a'",[],|r|r.get::<_,String>(0)).unwrap(),"backlog");assert_eq!(c.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  }
  let backup=crate::backup::export(&db,t.path()).unwrap();let target=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&target.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();assert_eq!(crate::smart_lists::read_one(&c,"list").ok().unwrap()["definition"]["rule"]["predicate"]["field"],"favorite");assert!(load(&c,"b",&Default::default()).unwrap().fields[&Field::Favorite].values.contains("true"));
 }
}

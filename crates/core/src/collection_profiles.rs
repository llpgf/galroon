//! Permanent entity biographies have no provider filmographies. Collection credits
//! are projected from saved owned work pages, never copied into every biography.
use crate::{App,artwork_library::Library};
use rusqlite::{Connection,OptionalExtension};
use serde_json::{Value,json};
use std::{collections::BTreeSet,path::Path};
type R<T>=Result<T,String>;
pub(crate) fn key(kind:&str,id:&str)->String{format!("collection.v1.{kind}.{id}")}
pub(crate) fn owned(c:&Connection)->R<BTreeSet<String>>{
 let mut s=c.prepare("SELECT DISTINCT vndb_id FROM works WHERE merged_into IS NULL AND vndb_id IS NOT NULL").map_err(|e|e.to_string())?;
 let rows=s.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<BTreeSet<_>>>().map_err(|e|e.to_string())?;Ok(rows)
}
pub(crate) fn basic(value:&Value,kind:&str,owned:&BTreeSet<String>)->Value{
 let mut entity=value[kind].clone();
 if let Some(vns)=entity.get_mut("vns").and_then(Value::as_array_mut){vns.retain(|v|v["id"].as_str().is_some_and(|id|owned.contains(id)));}
 json!({(kind):entity,"fetched_at":value["fetched_at"]})
}
fn work_pages(c:&Connection,library:&Library,owned:&BTreeSet<String>)->R<Vec<Value>>{
 let mut out=vec![];
 for id in owned{let key=format!("exploration.v4.work.{id}.1");
  let page=match library.profile(&key)?{Some(value)=>Some(value),None=>{
   let raw:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
   raw.map(|v|serde_json::from_str(&v).map_err(|e|e.to_string())).transpose()?
  }};
  if let Some(page)=page{out.push(page);}
 }Ok(out)
}
/// A marked subset is safe to browse offline but cannot stand in for a complete
/// provider filmography when building a list of all of somebody's works.
pub(crate) fn offline_page(a:&App,kind:&str,id:&str,page:u32)->R<Option<Value>>{
 let Some(mut value)=crate::artwork_library::profile(&a.state_dir,&key(kind,id))?else{return Ok(None);};
 let c=a.db.lock().map_err(|e|e.to_string())?;let library=Library::open(&a.state_dir)?;
 let mut works=vec![];
 for profile in work_pages(&c,&library,&owned(&c)?)?{
  if !crate::artwork_offline::entities(&profile).contains(&(kind,id.to_owned())){continue;}
  let mut vn=profile["vn"].clone();
  if let Some(voices)=vn.get_mut("va").and_then(Value::as_array_mut){for voice in voices{
   if let Some(character)=profile["characters"].as_array().into_iter().flatten().find(|v|v["id"]==voice["character"]["id"]){voice["character"]=character.clone();}
  }}
  works.push(vn);
 }
 works.sort_by(|a,b|b["released"].as_str().cmp(&a["released"].as_str()).then(a["id"].as_str().cmp(&b["id"].as_str())));
 let offset=(page as usize-1)*20;value["more"]=json!(works.len()>offset+20);value["works"]=json!(works.into_iter().skip(offset).take(20).collect::<Vec<_>>());value["page"]=json!(page);value["collection_only"]=json!(true);
 Ok(Some(value))
}
/// One-time, restartable conversion. The permanent transaction commits first;
/// cache removal and its marker follow, so interruption cannot lose biographies.
/// Only public metadata is rewritten; artwork and catalog/matching rows are untouched.
pub(crate) fn migrate(base:&Path,c:&mut Connection)->R<()>{
 let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key='collection.profiles.v1')",[],|r|r.get(0)).map_err(|e|e.to_string())?;if done{return Ok(());}
 if !base.join(crate::artwork_library::FILE).exists(){c.execute("INSERT INTO settings VALUES('collection.profiles.v1','1')",[]).map_err(|e|e.to_string())?;return Ok(());}
 let library=Library::open(base)?;let owned=owned(c)?;let mut related=BTreeSet::new();
 for page in work_pages(c,&library,&owned)?{related.extend(crate::artwork_offline::entities(&page));}
 for id in &owned{use crate::relation_corrections::Key;
  for correction in crate::relation_correction_store::read(c,id).map_err(|e|e.0)?.1{if let Some(link)=correction.link{match link.key{
   Key::Staff{id,..}=>{related.insert(("person",id));},Key::Voice{id,character,..}=>{related.insert(("person",id));related.insert(("character",character));},Key::Character{id,..}=>{related.insert(("character",id));},Key::Company{id}=>{related.insert(("company",id));},Key::Work{..}=>{}
  }}}
 }
 let tx=library.connection.unchecked_transaction().map_err(|e|e.to_string())?;
 // Stream old pages instead of holding hundreds of megabytes of JSON in memory.
 for (source,query) in [(&*c,"SELECT key,value FROM settings WHERE key GLOB 'exploration.v3.*'"),(&library.connection,"SELECT key,value FROM profiles WHERE key GLOB 'exploration.v3.*'")]{
  let mut s=source.prepare(query).map_err(|e|e.to_string())?;let mut rows=s.query([]).map_err(|e|e.to_string())?;
  while let Some(row)=rows.next().map_err(|e|e.to_string())?{
   let old_key:String=row.get(0).map_err(|e|e.to_string())?;let parts:Vec<_>=old_key.split('.').collect();
   let (kind,id,field)=match parts.as_slice(){["exploration","v3","profile",kind,id]=>(*kind,*id,"profile"),["exploration","v3",kind,id,_]=>(*kind,*id,*kind),_=>continue};
   if !related.contains(&(kind,id.to_owned())){continue;}
   let raw:String=row.get(1).map_err(|e|e.to_string())?;let value:Value=serde_json::from_str(&raw).map_err(|e|e.to_string())?;
   if value[field]["id"]!=id{continue;}
   let new_key=key(kind,id);let saved=library.profile(&new_key)?;
   if saved.as_ref().is_some_and(|v|v["fetched_at"].as_i64().unwrap_or(0)>=value["fetched_at"].as_i64().unwrap_or(0)){continue;}
   library.save_profile(&new_key,&basic(&json!({(kind):value[field],"fetched_at":value["fetched_at"]}),kind,&owned))?;
  }
 }
 tx.execute("DELETE FROM profiles WHERE key GLOB 'exploration.v3.*'",[]).map_err(|e|e.to_string())?;
 let keys={let mut s=tx.prepare("SELECT key FROM profiles").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;rows};
 for key in keys{let parts:Vec<_>=key.split('.').collect();let keep=match parts.as_slice(){["exploration","v4","work",id,"1"]=>owned.contains(*id),["collection","v1",kind,id]=>related.contains(&(*kind,id.to_string())),_=>false};if !keep{tx.execute("DELETE FROM profiles WHERE key=?1",[key]).map_err(|e|e.to_string())?;}}
 tx.commit().map_err(|e|e.to_string())?;
 let removed=c.execute("DELETE FROM settings WHERE key GLOB 'exploration.v3.*' OR key GLOB 'collection.v1.*'",[]).map_err(|e|e.to_string())?;
 library.connection.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e|e.to_string())?;
 if removed>0{c.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e|e.to_string())?;}
 c.execute("INSERT INTO settings VALUES('collection.profiles.v1','1')",[]).map_err(|e|e.to_string())?;Ok(())
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn migration_discards_filmographies_preserves_biographies_images_catalog_and_restart(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().join("data")).unwrap();
  let library=Library::open(&a.state_dir).unwrap();
  let profile=json!({"vn":{"id":"v1","title":"Owned","staff":[{"id":"s1"}],"developers":[{"id":"p1"}],"va":[]},"characters":[{"id":"c1","vns":[{"id":"v1","spoiler":0}]}],"more":false,"fetched_at":10});
  library.save_profile("exploration.v4.work.v1.1",&profile).unwrap();
  let image=crate::artwork_cache::Image{mime:"image/png".into(),bytes:b"\x89PNG\r\n\x1a\nfixture".to_vec()};library.put("image",&image).unwrap();
  let outside=json!({"id":"v999","description":"Excess filmography".repeat(10000)});
  {
   let c=a.db.lock().unwrap();c.execute_batch("DELETE FROM settings WHERE key='collection.profiles.v1'; INSERT INTO works(id,title,original_title,vndb_id,notes) VALUES('w','Personal title','Original','v1','Keep'); INSERT INTO settings VALUES('unrelated','keep');").unwrap();
   for(kind,id)in [("person","s1"),("character","c1"),("company","p1"),("person","s999")]{for page in 1..=3{
    let value=json!({(kind):{"id":id,"name":"Basic name","description":"Biography","vns":[{"id":"v1","spoiler":0},{"id":"v999","spoiler":0}]},"works":[outside.clone()],"more":page<3,"page":page,"fetched_at":page});
    let key=format!("exploration.v3.{kind}.{id}.{page}");library.save_profile(&key,&value).unwrap();c.execute("INSERT INTO settings VALUES(?1,?2)",rusqlite::params![key,value.to_string()]).unwrap();
   }}
  }
  let before=library.connection.query_row("PRAGMA page_count",[],|r|r.get::<_,i64>(0)).unwrap();
  migrate(&a.state_dir,&mut a.db.lock().unwrap()).unwrap();
  assert_eq!(library.stats().unwrap().profiles,4);assert_eq!(library.image("image").unwrap().unwrap().bytes,image.bytes);
  assert_eq!(library.profile("exploration.v4.work.v1.1").unwrap().unwrap(),profile);
  let saved=library.profile("collection.v1.character.c1").unwrap().unwrap();assert_eq!(saved["character"]["description"],"Biography");assert_eq!(saved["character"]["vns"],json!([{"id":"v1","spoiler":0}]));
  assert!(library.profiles().unwrap().iter().all(|(_,v)|!v.contains("v999")&&!v.contains("works")));
  assert!(library.connection.query_row("PRAGMA page_count",[],|r|r.get::<_,i64>(0)).unwrap()<before/2);
  {let c=a.db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key GLOB 'exploration.v3.*'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT title||notes FROM works",[],|r|r.get::<_,String>(0)).unwrap(),"Personal titleKeep");assert_eq!(c.query_row("SELECT value FROM settings WHERE key='unrelated'",[],|r|r.get::<_,String>(0)).unwrap(),"keep");}
  let saved=library.profiles().unwrap();a.db.lock().unwrap().execute("DELETE FROM settings WHERE key='collection.profiles.v1'",[]).unwrap();migrate(&a.state_dir,&mut a.db.lock().unwrap()).unwrap();assert_eq!(library.profiles().unwrap(),saved);
  let page=offline_page(&a,"person","s1",1).unwrap().unwrap();assert_eq!(page["works"].as_array().unwrap().len(),1);assert_eq!(page["works"][0]["id"],"v1");assert_eq!(page["collection_only"],true);
  let backup=crate::backup::export(&a.db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::initialize(restored).unwrap();assert_eq!(Library::open(&restored.state_dir).unwrap().profiles().unwrap(),saved);assert_eq!(offline_page(&restored,"person","s1",1).unwrap().unwrap(),page);
 }
 #[test]fn on_demand_refresh_never_promotes_filmography_to_permanent_storage(){
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().into()).unwrap();let library=Library::open(&a.state_dir).unwrap();library.save_profile("collection.v1.person.s1",&json!({"person":{"id":"s1","description":"Old"},"fetched_at":1})).unwrap();
  let value=json!({"person":{"id":"s1","description":"New"},"works":[{"id":"v999"}],"more":true,"page":1,"fetched_at":2});crate::exploration::save(&a,"exploration.v3.person.s1.1",&value).unwrap();
  assert_eq!(crate::exploration::cached(&a,"exploration.v3.person.s1.1").unwrap().unwrap(),value);
  let saved=library.profile("collection.v1.person.s1").unwrap().unwrap();assert_eq!(saved["person"]["description"],"New");assert!(saved.get("works").is_none());assert!(library.profile("exploration.v3.person.s1.1").unwrap().is_none());
 }
}

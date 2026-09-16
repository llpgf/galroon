//! Work-page projection for typed corrections. Caller supplies raw provider data, never its cache row.
use serde_json::{Value,json};
use crate::relation_corrections::{Correction,Key,Link,validate};
fn rows_mut<'a>(v:&'a mut Value,path:&str)->Result<&'a mut Vec<Value>,String>{
 let slot=if path=="characters"{v.as_object_mut().ok_or("Invalid work page")?.entry(path).or_insert_with(||json!([]))}else{v.get_mut("vn").and_then(Value::as_object_mut).ok_or("Work data unavailable")?.entry(path).or_insert_with(||json!([]))};slot.as_array_mut().ok_or_else(||"Invalid relationship rows".into())
}
fn field(v:&Value,k:&str)->String{v[k].as_str().unwrap_or("").into()}
pub(crate) fn matches(row:&Value,key:&Key)->bool{match key{
 Key::Company{id}|Key::Character{id}=>row["id"]==*id,
 Key::Work{id,relation}=>row["id"]==*id&&row["relation"]==*relation,
 Key::Staff{id,aid,role,note}=>row["id"]==*id&&row["role"]==*role&&row["aid"].as_i64()==*aid&&field(row,"note")==*note,
 Key::Voice{id,character,alias,note}=>row["staff"]["id"]==*id&&row["character"]["id"]==*character&&row["staff"]["aid"].as_i64()==Some(*alias)&&field(row,"note")==*note,
}}
pub(crate) fn path(key:&Key)->&'static str{match key{Key::Company{..}=>"developers",Key::Character{..}=>"characters",Key::Staff{..}=>"staff",Key::Voice{..}=>"va",Key::Work{..}=>"relations"}}
fn character(id:&str,name:&str,work:&str,spoiler:u8)->Value{json!({"id":id,"name":name,"vns":[{"id":work,"spoiler":spoiler}]})}
fn local(link:&Link,work:&str,correction:&str)->Value{
 let mut row=match &link.key{
  Key::Company{id}=>json!({"id":id,"name":link.name}),
  Key::Character{id}=>character(id,&link.name,work,link.spoiler),
  Key::Staff{id,aid,role,note}=>json!({"id":id,"aid":aid,"name":link.name,"role":role,"note":note}),
  Key::Voice{id,character:cid,alias,note}=>json!({"staff":{"id":id,"aid":alias,"name":link.name},"character":character(cid,link.character_name.as_deref().unwrap_or(""),work,link.spoiler),"note":note}),
  Key::Work{id,relation}=>json!({"id":id,"title":link.name,"relation":relation}),
 };row["local_relation"]=json!(true);row["correction_id"]=json!(correction);row["spoiler"]=json!(link.spoiler);row
}
/// Copy-on-write, deterministic and idempotent. Broad hide decisions always take precedence.
pub fn project(raw:&Value,work:&str,corrections:&[Correction],hidden:&crate::relation_overrides::Hidden)->Result<Value,String>{
 validate(work,corrections)?;if raw["vn"]["id"]!=work{return Err("Work identity unavailable or changed".into());}let mut result=raw.clone();
 // Remove all exact source edges before adding replacements, so correction order cannot erase additions.
 for correction in corrections{if let Some(source)=&correction.replaces{rows_mut(&mut result,path(source))?.retain(|row|!matches(row,source));if let Key::Character{id}=source{rows_mut(&mut result,"va")?.retain(|row|row["character"]["id"]!=*id);}}}
 for correction in corrections{if let Some(link)=&correction.link{
  let rows=rows_mut(&mut result,path(&link.key))?;rows.retain(|row|!matches(row,&link.key));rows.push(local(link,work,&correction.id));
  if let Key::Voice{character:cid,..}=&link.key{let characters=rows_mut(&mut result,"characters")?;
   if let Some(existing)=characters.iter_mut().find(|row|row["id"]==*cid){
    if existing["local_voice_appearance"]==true&&existing["vns"][0]["spoiler"].as_u64().is_some_and(|level|level>u64::from(link.spoiler)){
     existing["vns"][0]["spoiler"]=json!(link.spoiler);existing["name"]=json!(link.character_name);existing["correction_id"]=json!(&correction.id);
    }
   }else{let mut c=character(cid,link.character_name.as_deref().unwrap_or(""),work,link.spoiler);c["local_relation"]=json!(true);c["local_voice_appearance"]=json!(true);c["correction_id"]=json!(&correction.id);characters.push(c);}
  }
 }}
 // Appearance decisions dominate nested voice copies of the same character.
 let characters=result["characters"].as_array().cloned().unwrap_or_default();
 if let Some(voices)=result.get_mut("vn").and_then(|v|v.get_mut("va")).and_then(Value::as_array_mut){for voice in voices{
  if let Some(character)=characters.iter().find(|c|c["id"]==voice["character"]["id"]){voice["character"]["vns"]=character["vns"].clone();}
 }}crate::relation_overrides::project(&mut result,hidden);Ok(result)
}
#[cfg(test)]mod tests{
 use super::*;
 fn raw()->Value{json!({"vn":{"id":"v1","staff":[{"id":"s1","name":"Writer","role":"scenario"}],"va":[{"staff":{"id":"s1","aid":1,"name":"Alias one"},"character":{"id":"c1"}},{"staff":{"id":"s1","aid":2,"name":"Alias two"},"character":{"id":"c1"}}]},"characters":[{"id":"c1","name":"Source character"}],"stale":true,"fetched_at":1})}
 fn correction()->Correction{Correction{id:"voice-edit".into(),replaces:Some(Key::Voice{id:"s1".into(),character:"c1".into(),alias:1,note:"".into()}),link:Some(Link{key:Key::Voice{id:"s2".into(),character:"c1".into(),alias:3,note:"".into()},name:"Actual new alias".into(),character_name:Some("Character".into()),spoiler:2})}}
 #[test]fn rebind_preserves_other_alias_and_staff_and_raw_cache(){let raw=raw();let saved=raw.clone();let result=project(&raw,"v1",&[correction()],&Default::default()).unwrap();assert_eq!(raw,saved);assert_eq!(result["vn"]["staff"],raw["vn"]["staff"]);assert_eq!(result["vn"]["va"][0]["staff"]["aid"],2);assert_eq!(result["vn"]["va"][1]["staff"]["name"],"Actual new alias");assert_eq!(result["vn"]["va"][1]["spoiler"],2);assert_eq!(result["stale"],true);assert_eq!(result["fetched_at"],1);assert_eq!(project(&result,"v1",&[correction()],&Default::default()).unwrap(),result);}
 #[test]fn broad_hidden_decision_wins_over_local_addition(){let hidden=crate::relation_overrides::Hidden{people:["s2".into()].into(),..Default::default()};let result=project(&raw(),"v1",&[correction()],&hidden).unwrap();assert_eq!(result["vn"]["va"].as_array().unwrap().len(),1);assert_eq!(result["vn"]["va"][0]["staff"]["aid"],2);}
 #[test]fn all_edge_kinds_project_with_provenance_and_unknown_work_rejected(){let links=vec![Link{key:Key::Company{id:"p1".into()},name:"Studio".into(),character_name:None,spoiler:0},Link{key:Key::Character{id:"c2".into()},name:"Character".into(),character_name:None,spoiler:2},Link{key:Key::Staff{id:"s3".into(),aid:None,role:"music".into(),note:"Composer".into()},name:"Musician".into(),character_name:None,spoiler:0},Link{key:Key::Work{id:"v2".into(),relation:"sequel".into()},name:"Sequel".into(),character_name:None,spoiler:1}];let corrections:Vec<_>=links.into_iter().enumerate().map(|(i,link)|Correction{id:i.to_string(),replaces:None,link:Some(link)}).collect();let result=project(&raw(),"v1",&corrections,&Default::default()).unwrap();for field in ["developers","relations"]{assert_eq!(result["vn"][field][0]["local_relation"],true);}assert_eq!(result["characters"][1]["vns"][0]["spoiler"],2);assert!(project(&raw(),"v99",&[],&Default::default()).is_err());assert!(project(&json!({"vn":null}),"v1",&[],&Default::default()).is_err());}
}
/// Project an already loaded provider profile page. Manual-only works require a separate indexed page.
/// Does not infer absence when old provider rows omitted credit arrays.
pub fn profile_page(c:&rusqlite::Connection,raw:&Value,kind:&str,id:&str)->Result<Value,String>{
 if !matches!(kind,"person"|"character"|"company"){return Err("Invalid profile kind".into());}
 let mut result=raw.clone();let mut output=vec![];let mut removed=std::collections::BTreeSet::new();
 for row in raw["works"].as_array().ok_or("Profile works unavailable")?{
  let Some(work)=row["id"].as_str()else{output.push(row.clone());continue;};
  let corrections=crate::relation_correction_store::read(c,work).map_err(|e|e.0)?.1;
  let hidden=crate::relation_overrides::read(c,work).map_err(|e|e.0)?.1;
  if (kind=="person"&&hidden.people.contains(id))||(kind=="character"&&hidden.characters.contains(id))||(kind=="company"&&hidden.companies.contains(id)){removed.insert(work.to_string());continue;}
  if corrections.is_empty(){output.push(row.clone());continue;}
  // Person pages already carry authoritative appearance levels inside voice credits.
  let characters=row["va"].as_array().into_iter().flatten().filter_map(|v|v.get("character")).filter(|v|v["vns"].is_array()).cloned().collect::<Vec<_>>();
  let mut wrapped=json!({"vn":row,"characters":characters});
  // Provider company queries are filtered by this developer but return no developers field.
  if kind=="company"&&!row["developers"].is_array(){wrapped["vn"]["developers"]=json!([{"id":id,"name":raw["company"]["name"]}]);}
  if kind=="character"{if raw["character"]["id"]!=id{return Err("Character profile identity changed".into());}wrapped["characters"]=json!([raw["character"]]);}
  let known_person=row["staff"].is_array()&&row["va"].is_array();
  if kind=="person"&&!known_person{result["local_relationships_incomplete"]=json!(true);}
  let projected=project(&wrapped,work,&corrections,&hidden)?;
  let vn=&projected["vn"];
  let contains=match kind{
   "person"=>!known_person||vn["staff"].as_array().is_some_and(|rows|rows.iter().any(|r|r["id"]==id))||vn["va"].as_array().is_some_and(|rows|rows.iter().any(|r|r["staff"]["id"]==id)),
   "company"=>vn["developers"].as_array().is_some_and(|rows|rows.iter().any(|r|r["id"]==id)),
   _=>projected["characters"].as_array().is_some_and(|rows|rows.iter().any(|r|r["id"]==id)),
  };
  if contains{output.push(vn.clone());}else{removed.insert(work.to_string());}
 }
 result["works"]=Value::Array(output);
 if !removed.is_empty()&&raw["more"]==true{result["continuation_verified"]=json!(true);}
 if kind=="character"{if let Some(vns)=result.get_mut("character").and_then(|v|v.get_mut("vns")).and_then(Value::as_array_mut){vns.retain(|v|!v["id"].as_str().is_some_and(|work|removed.contains(work)));}}
 Ok(result)
}
#[cfg(test)]mod profile_tests{
 use super::*;use crate::relation_correction_store::{change,Edit};
 fn save(c:&mut rusqlite::Connection,source:Key,target:Option<Link>){change(c,"v1",&Edit{revision:0,request_id:"reverse-test".into(),corrections:vec![Correction{id:"one".into(),replaces:Some(source),link:target}]}).ok().unwrap();}
 #[test]fn last_voice_rebind_removes_old_person_but_other_duty_preserves_work(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();save(&mut c,Key::Voice{id:"s1".into(),character:"c1".into(),alias:1,note:"".into()},None);let mut raw=json!({"person":{"id":"s1"},"works":[{"id":"v1","staff":[],"va":[{"staff":{"id":"s1","aid":1},"character":{"id":"c1"}}]}],"more":true,"page":3,"stale":true});let result=profile_page(&c,&raw,"person","s1").unwrap();assert!(result["works"].as_array().unwrap().is_empty());assert_eq!(result["more"],true);assert_eq!(result["page"],3);assert_eq!(result["continuation_verified"],true);assert_eq!(result["stale"],true);raw["works"][0]["staff"]=json!([{"id":"s1","role":"music"}]);assert_eq!(profile_page(&c,&raw,"person","s1").unwrap()["works"].as_array().unwrap().len(),1);raw["works"][0].as_object_mut().unwrap().remove("staff");assert_eq!(profile_page(&c,&raw,"person","s1").unwrap()["works"].as_array().unwrap().len(),1);}
 #[test]fn filtered_company_rows_and_character_appearances_follow_exact_removal(){for(kind,id,key)in [("company","p1",Key::Company{id:"p1".into()}),("character","c1",Key::Character{id:"c1".into()})]{let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();save(&mut c,key,None);let raw=json!({kind:{"id":id,"name":"Source","vns":[{"id":"v1"},{"id":"v2"}]},"works":[{"id":"v1"}],"more":false,"page":1});let saved=raw.clone();let result=profile_page(&c,&raw,kind,id).unwrap();assert_eq!(raw,saved);assert!(result["works"].as_array().unwrap().is_empty());if kind=="character"{assert_eq!(result["character"]["vns"],json!([{"id":"v2"}]))}}}
}


#[cfg(test)] mod staff_alias_tests {
 use super::*;
 #[test] fn staff_rebind_preserves_same_role_aliases_and_legacy_is_not_wildcard() {
  let raw=json!({"vn":{"id":"v1","staff":[
   {"id":"s1","aid":1,"role":"scenario","name":"First alias"},
   {"id":"s1","aid":2,"role":"scenario","name":"Second alias"},
   {"id":"s1","role":"scenario","name":"Legacy credit"},
   {"id":"s1","aid":1,"role":"music","name":"First alias"}
  ],"va":[]},"characters":[]});
  let key=|aid|Key::Staff{id:"s1".into(),aid,role:"scenario".into(),note:"".into()};
  let edit=Correction{id:"rebind".into(),replaces:Some(key(Some(1))),link:Some(Link{key:Key::Staff{id:"s2".into(),aid:Some(9),role:"scenario".into(),note:"".into()},name:"New alias".into(),character_name:None,spoiler:0})};
  let saved=raw.clone();let result=project(&raw,"v1",&[edit.clone()],&Default::default()).unwrap();
  assert_eq!(raw,saved);let rows=result["vn"]["staff"].as_array().unwrap();assert_eq!(rows.len(),4);
  assert_eq!(rows[0]["aid"],2);assert_eq!(rows[1]["name"],"Legacy credit");assert_eq!(rows[2]["role"],"music");assert_eq!(rows[3]["aid"],9);
  assert_eq!(project(&result,"v1",&[edit],&Default::default()).unwrap(),result);
  let legacy=Correction{id:"legacy".into(),replaces:Some(key(None)),link:None};
  let result=project(&raw,"v1",&[legacy],&Default::default()).unwrap();
  assert_eq!(result["vn"]["staff"].as_array().unwrap().len(),3);
  assert!(result["vn"]["staff"].as_array().unwrap().iter().all(|r|r["aid"].is_number()));
 }
}

#[cfg(test)] mod staff_reverse_alias_tests {
 use super::*;
 #[test] fn reverse_membership_keeps_other_staff_alias_after_exact_hide() {
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  let hidden=Correction{id:"hide-one".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:Some(1),role:"scenario".into(),note:"".into()}),link:None};
  crate::relation_correction_store::change(&mut c,"v1",&crate::relation_correction_store::Edit{revision:0,request_id:"reverse-staff".into(),corrections:vec![hidden]}).ok().unwrap();
  let raw=json!({"person":{"id":"s1"},"works":[{"id":"v1","staff":[{"id":"s1","aid":1,"role":"scenario"},{"id":"s1","aid":2,"role":"scenario"}],"va":[]}],"more":false,"page":1});
  let projected=profile_page(&c,&raw,"person","s1").unwrap();assert_eq!(projected["works"].as_array().unwrap().len(),1);assert_eq!(projected["works"][0]["staff"],json!([{"id":"s1","aid":2,"role":"scenario"}]));
 }
}

/// Strip private local edges before returning an effective work to a read client.
/// Provider spoiler semantics remain on character appearances; local edges fail closed.
pub fn visible_edges(value:&mut Value,spoilers:bool) {
 if spoilers{return;}
 for field in ["staff","va","developers","relations"] {
  if let Some(rows)=value.get_mut("vn").and_then(|vn|vn.get_mut(field)).and_then(Value::as_array_mut){
   rows.retain(|row|row["local_relation"]!=true||row["spoiler"].as_u64()==Some(0));
  }
 }
 if let Some(rows)=value.get_mut("characters").and_then(Value::as_array_mut){
  rows.retain(|row|row["local_relation"]!=true||row["vns"].as_array().is_some_and(|vns|vns.iter().any(|v|v["spoiler"]==0)));
 }
}
fn restrict_appearance(character:&mut Value,work:&str,spoilers:bool){
 if let Some(rows)=character.get_mut("vns").and_then(Value::as_array_mut){rows.retain(|v|v["id"]==work&&(spoilers||v["spoiler"]==0));}
}
fn appearance_visible(character:&Value,work:&str,spoilers:bool)->bool {
 character["vns"].as_array().is_some_and(|rows|rows.iter().any(|v|v["id"]==work&&(spoilers||v["spoiler"]==0)))
}
/// Runtime profile projection combines legacy hides, exact edits and per-edge visibility.
/// Provider continuation is never inferred from the number of remaining visible rows.
pub fn visible_profile_page(c:&rusqlite::Connection,raw:&Value,kind:&str,id:&str,spoilers:bool)->Result<Value,String>{
 let mut broad=raw.clone();crate::relation_overrides::project_profile(c,&mut broad,kind,id).map_err(|e|e.0)?;
 let mut result=profile_page(c,&broad,kind,id)?;
 // The profile may contain appearances beyond this loaded work page. Correct them too,
 // so hidden membership is not left in the entity payload or accessibility consumers.
 if kind=="character" {
  let mut appearances=vec![];
  for entry in broad["character"]["vns"].as_array().into_iter().flatten(){
   let Some(work)=entry["id"].as_str()else{continue;};
   let edits=crate::relation_correction_store::read(c,work).map_err(|e|e.0)?.1;
   let hidden=crate::relation_overrides::read(c,work).map_err(|e|e.0)?.1;
   let projected=project(&json!({"vn":{"id":work},"characters":[broad["character"]]}),work,&edits,&hidden)?;
   if let Some(character)=projected["characters"].as_array().into_iter().flatten().find(|v|v["id"]==id){
    if let Some(appearance)=character["vns"].as_array().into_iter().flatten().find(|v|v["id"]==work&&(spoilers||v["spoiler"]==0)){appearances.push(appearance.clone());}
   }
  }
  if result["character"].is_object(){result["character"]["vns"]=json!(appearances);}
 }
 let mut works=vec![];let mut removed=false;
 let loaded=result["works"].as_array().ok_or("Profile works unavailable")?.clone();
 for row in &loaded {
  let Some(work)=row["id"].as_str()else{result["local_relationships_incomplete"]=json!(true);continue;};
  let mut wrapped=json!({"vn":row});visible_edges(&mut wrapped,spoilers);
  // A voice credit is visible only with its corresponding visible appearance.
  if let Some(voices)=wrapped.get_mut("vn").and_then(|v|v.get_mut("va")).and_then(Value::as_array_mut){voices.retain(|v|appearance_visible(&v["character"],work,spoilers));for voice in voices{if let Some(character)=voice.get_mut("character"){restrict_appearance(character,work,spoilers);}}}
  let vn=&mut wrapped["vn"];
  let known_person=row["staff"].is_array()&&row["va"].is_array();
  let belongs=match kind {
   "person"=>!known_person||vn["staff"].as_array().into_iter().flatten().any(|v|v["id"]==id)||vn["va"].as_array().into_iter().flatten().any(|v|v["staff"]["id"]==id),
   "character"=>appearance_visible(&result["character"],work,spoilers),
   _=>!vn["developers"].is_array()||vn["developers"].as_array().into_iter().flatten().any(|v|v["id"]==id),
  };
  if belongs {vn["profile_membership"]=json!(true);works.push(vn.clone());}else{removed=true;}
 }
 result["works"]=json!(works);if removed&&result["more"]==true{result["continuation_verified"]=json!(true);}Ok(result)
}

#[cfg(test)] mod visible_profile_tests {
 use super::*;use crate::relation_correction_store::{change,Edit};
 fn save(c:&mut rusqlite::Connection,work:&str,corrections:Vec<Correction>){change(c,work,&Edit{revision:0,request_id:format!("visible-{work}"),corrections}).ok().unwrap();}
 #[test] fn person_exact_alias_and_spoiler_projection_compose_with_broad_hides(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  save(&mut c,"v1",vec![Correction{id:"one".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:Some(1),role:"scenario".into(),note:"".into()}),link:Some(Link{key:Key::Staff{id:"s2".into(),aid:Some(9),role:"scenario".into(),note:"secret-note".into()},name:"Secret alias".into(),character_name:None,spoiler:2})}]);
  let raw=json!({"person":{"id":"s1"},"works":[{"id":"v1","title":"Retained work","staff":[{"id":"s1","aid":1,"role":"scenario"},{"id":"s1","aid":2,"role":"music"}],"va":[]}],"page":2,"more":true});
  let result=visible_profile_page(&c,&raw,"person","s1",false).unwrap();assert_eq!(result["works"][0]["staff"],json!([{"id":"s1","aid":2,"role":"music"}]));assert!(!result.to_string().contains("Secret"));assert!(!result.to_string().contains("secret-note"));assert_eq!(result["page"],2);assert_eq!(result["more"],true);
  let shown=visible_profile_page(&c,&raw,"person","s1",true).unwrap();assert!(shown.to_string().contains("Secret alias"));
  c.execute("INSERT INTO relation_overrides VALUES('v1',1,?1)",[json!({"people":["s1"]}).to_string()]).unwrap();assert!(visible_profile_page(&c,&raw,"person","s1",true).unwrap()["works"].as_array().unwrap().is_empty());
 }
 #[test] fn last_secret_credit_and_unloaded_character_appearances_do_not_leak_membership(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  save(&mut c,"v1",vec![Correction{id:"local".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:None,role:"music".into(),note:"".into()},name:"Secret".into(),character_name:None,spoiler:2})}]);
  let raw=json!({"person":{"id":"s1"},"works":[{"id":"v1","title":"Secret membership","staff":[],"va":[]}],"page":1,"more":true});
  let result=visible_profile_page(&c,&raw,"person","s1",false).unwrap();assert_eq!(result["works"],json!([]));assert_eq!(result["continuation_verified"],true);assert_eq!(visible_profile_page(&c,&raw,"person","s1",true).unwrap()["works"].as_array().unwrap().len(),1);
  save(&mut c,"v2",vec![Correction{id:"hide".into(),replaces:Some(Key::Character{id:"c1".into()}),link:None}]);
  let raw=json!({"character":{"id":"c1","vns":[{"id":"v1","spoiler":0},{"id":"v2","spoiler":0},{"id":"v3","spoiler":2}]},"works":[{"id":"v1","title":"Visible"}],"more":true,"page":1});
  let result=visible_profile_page(&c,&raw,"character","c1",false).unwrap();assert_eq!(result["character"]["vns"],json!([{"id":"v1","spoiler":0}]));assert_eq!(result["works"][0]["profile_membership"],true);assert!(!result.to_string().contains("v2"));assert!(!result.to_string().contains("v3"));
 }
 #[test] fn ambiguous_legacy_person_data_stays_explicitly_incomplete(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  save(&mut c,"v1",vec![Correction{id:"hide".into(),replaces:Some(Key::Staff{id:"s1".into(),aid:None,role:"music".into(),note:"".into()}),link:None}]);
  let result=visible_profile_page(&c,&json!({"person":{"id":"s1"},"works":[{"id":"v1","title":"Legacy"}],"more":false,"page":1}),"person","s1",false).unwrap();
  assert_eq!(result["local_relationships_incomplete"],true);
 }
}

/// Shared effective edges, independent of provider freshness. Cache keys identify legacy rows.
pub fn effective(c:&rusqlite::Connection,raw:&Value,work:&str)->Result<Value,String>{
 let corrections=crate::relation_correction_store::read(c,work).map_err(|e|e.0)?.1;
 let hidden=crate::relation_overrides::read(c,work).map_err(|e|e.0)?.1;
 if corrections.is_empty(){let mut value=raw.clone();crate::relation_overrides::project(&mut value,&hidden);return Ok(value);}
 let mut source=raw.clone();
 let complete=source["vn"]["developers"].is_array()&&source["vn"]["staff"].is_array()&&source["vn"]["va"].is_array()&&source["characters"].is_array();
 if !source["vn"].is_object(){source["vn"]=json!({"id":work});}
 if source["vn"]["id"].is_null(){source["vn"]["id"]=json!(work);}
 let mut value=project(&source,work,&corrections,&hidden)?;
 if !complete{value["local_relationships_incomplete"]=json!(true);}Ok(value)
}
/// Effective work response: local edge spoilers and character membership are projected together.
pub fn work_page(c:&rusqlite::Connection,raw:&Value,work:&str,spoilers:bool)->Result<Value,String>{
 let mut value=effective(c,raw,work)?;visible_edges(&mut value,spoilers);
 let mut visible=std::collections::BTreeSet::new();
 if let Some(rows)=value.get_mut("characters").and_then(Value::as_array_mut){rows.retain(|row|appearance_visible(row,work,spoilers));for row in rows{restrict_appearance(row,work,spoilers);if let Some(id)=row["id"].as_str(){visible.insert(id.to_string());}}}
 if let Some(voices)=value.get_mut("vn").and_then(|v|v.get_mut("va")).and_then(Value::as_array_mut){voices.retain(|v|v["character"]["id"].as_str().is_some_and(|id|visible.contains(id)));for voice in voices{if let Some(character)=voice.get_mut("character"){restrict_appearance(character,work,spoilers);}}}
 Ok(value)
}

#[cfg(test)] mod voice_appearance_tests {
 use super::*;
 #[test] fn mixed_voice_spoilers_are_order_independent_and_explicit_appearance_wins(){
  let voice=|id:&str,spoiler|Correction{id:id.into(),replaces:None,link:Some(Link{key:Key::Voice{id:if spoiler==0{"s1".into()}else{"s2".into()},character:"c1".into(),alias:1,note:"".into()},name:if spoiler==0{"Visible voice".into()}else{"Secret voice".into()},character_name:Some(if spoiler==0{"Visible character".into()}else{"Secret character".into()}),spoiler})};
  let raw=json!({"vn":{"id":"v1","staff":[],"va":[],"developers":[]},"characters":[],"more":false});
  for mut edits in [vec![voice("hidden",2),voice("safe",0)],vec![voice("safe",0),voice("hidden",2)]]{
   let mut result=project(&raw,"v1",&edits,&Default::default()).unwrap();
   assert_eq!(result["characters"][0]["name"],"Visible character");assert_eq!(result["characters"][0]["vns"][0]["spoiler"],0);
   visible_edges(&mut result,false);assert_eq!(result["vn"]["va"].as_array().unwrap().len(),1);assert!(!result.to_string().contains("Secret"));
   let explicit=Correction{id:"explicit".into(),replaces:None,link:Some(Link{key:Key::Character{id:"c1".into()},name:"Explicit secret".into(),character_name:None,spoiler:2})};
   for first in [false,true]{if first{edits.insert(0,explicit.clone());}else{edits.push(explicit.clone());}
    let projected=project(&raw,"v1",&edits,&Default::default()).unwrap();assert_eq!(projected["characters"][0]["name"],"Explicit secret");assert!(projected["vn"]["va"].as_array().unwrap().iter().all(|v|v["character"]["vns"][0]["spoiler"]==2));edits.retain(|e|e.id!="explicit");
   }
  }
 }
}

#[cfg(test)]mod appearance_payload_tests{
 use super::*;
 #[test]fn work_and_reverse_voice_payloads_never_carry_other_work_appearances(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();let character=json!({"id":"c1","vns":[{"id":"v1","spoiler":0},{"id":"v99","spoiler":2}]});let vn=json!({"id":"v1","staff":[],"va":[{"staff":{"id":"s1","aid":1},"character":character}]});let raw=json!({"vn":vn,"characters":[character],"more":false});let saved=raw.clone();
  assert!(!work_page(&c,&raw,"v1",false).unwrap().to_string().contains("v99"));assert_eq!(raw,saved);
  let profile=json!({"person":{"id":"s1"},"works":[vn],"more":false,"page":1});assert!(!visible_profile_page(&c,&profile,"person","s1",false).unwrap().to_string().contains("v99"));
 }
}

#[cfg(test)]mod reverse_voice_appearance_tests{
 use super::*;
 #[test]fn local_voice_does_not_override_source_appearance_in_person_results(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  crate::relation_correction_store::change(&mut c,"v1",&crate::relation_correction_store::Edit{revision:0,request_id:"voice".into(),corrections:vec![Correction{id:"voice".into(),replaces:None,link:Some(Link{key:Key::Voice{id:"s1".into(),character:"c1".into(),alias:2,note:"".into()},name:"Added voice".into(),character_name:Some("Generated character".into()),spoiler:0})}]}).map_err(|e|e.0).unwrap();
  let raw=json!({"person":{"id":"s1"},"works":[{"id":"v1","staff":[],"va":[{"staff":{"id":"s1","aid":1},"character":{"id":"c1","vns":[{"id":"v1","spoiler":2}]}}]}],"more":false,"page":1});
  assert_eq!(visible_profile_page(&c,&raw,"person","s1",false).unwrap()["works"],json!([]));assert_eq!(visible_profile_page(&c,&raw,"person","s1",true).unwrap()["works"].as_array().unwrap().len(),1);
 }
}

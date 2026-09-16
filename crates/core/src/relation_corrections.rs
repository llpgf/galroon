//! Typed intent for exact local relationship corrections. Persistence/projection is separate.
use serde::{Deserialize,Serialize};
use std::collections::BTreeSet;
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Key {
 Company { id:String },
 Character { id:String },
 Staff { id:String, #[serde(default,skip_serializing_if="Option::is_none")] aid:Option<i64>, role:String, note:String },
 Voice { id:String, character:String, alias:i64, note:String },
 Work { id:String, relation:String },
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Link {pub key:Key,pub name:String,#[serde(default)]pub character_name:Option<String>,pub spoiler:u8}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Correction {
 /// Stable local identity retained through edits and undo.
 pub id:String,
 /// None adds a relationship. Some replaces just this exact source edge.
 pub replaces:Option<Key>,
 /// None hides an exact source edge; Some supplies a local relationship.
 pub link:Option<Link>,
}
fn text(value:&str,limit:usize)->bool{!value.trim().is_empty()&&value.chars().count()<=limit&&!value.chars().any(char::is_control)}
fn key(key:&Key,work:&str)->Result<(),String>{
 let (id,prefix)=match key{Key::Company{id}=>(id,'p'),Key::Character{id}=>(id,'c'),Key::Staff{id,aid,role,note}=>{
  if aid.is_some_and(|value|value<0)||!matches!(role.as_str(),"scenario"|"chardesign"|"art"|"music"|"songs"|"director"|"staff")||note.chars().count()>500||note.chars().any(char::is_control){return Err("Invalid staff role or credit note".into());}(id,'s')
 },Key::Voice{id,character,alias,note}=>{if !crate::exploration::valid_id(character,'c')||*alias<0||note.chars().count()>500||note.chars().any(char::is_control){return Err("Invalid voice character, alias or note".into());}(id,'s')},Key::Work{id,relation}=>{if id==work||!text(relation,80){return Err("Invalid related work".into());}(id,'v')}};
 if !crate::exploration::valid_id(id,prefix){return Err("Invalid relationship identity".into());}Ok(())
}
pub fn validate(work:&str,corrections:&[Correction])->Result<(),String>{
 if !crate::exploration::valid_id(work,'v')||corrections.len()>200{return Err("Invalid work or too many corrections".into());}
 let mut ids=BTreeSet::new();let mut replaced=BTreeSet::new();let mut added=BTreeSet::new();
 for change in corrections{
  if !text(&change.id,128)||!ids.insert(&change.id){return Err("Duplicate or invalid correction identity".into());}
  if change.replaces.is_none()&&change.link.is_none(){return Err("Empty correction".into());}
  if let Some(source)=&change.replaces{key(source,work)?;if !replaced.insert(source){return Err("A source relationship can be replaced only once".into());}}
  if let Some(link)=&change.link{key(&link.key,work)?;
   if !text(&link.name,300)||link.spoiler>2||!added.insert(&link.key){return Err("Invalid or duplicate local relationship".into());}
   match (&link.key,&link.character_name){(Key::Voice{..},Some(name)) if text(name,300)=>{},(Key::Voice{..},_)=>return Err("Voice credit requires its character label".into()),(_,None)=>{},_=>return Err("Character label belongs only to voice credits".into())}
   if let Some(source)=&change.replaces{if std::mem::discriminant(source)!=std::mem::discriminant(&link.key){return Err("Replacement must keep the relationship kind".into());}}
  }
 }Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 fn voice(person:&str,alias:i64)->Key{Key::Voice{id:person.into(),character:"c1".into(),alias,note:"Japanese voice".into()}}
 fn change()->Correction{Correction{id:"edit1".into(),replaces:Some(voice("s1",1)),link:Some(Link{key:voice("s2",2),name:"Actual credit alias".into(),character_name:Some("Character name".into()),spoiler:2})}}
 #[test]fn exact_aliases_and_multiple_roles_remain_distinct(){let a=change();let mut b=a.clone();b.id="edit2".into();b.replaces=Some(voice("s1",3));b.link.as_mut().unwrap().key=voice("s2",4);assert!(validate("v1",&[a,b]).is_ok());let staff=Correction{id:"staff".into(),replaces:None,link:Some(Link{key:Key::Staff{id:"s1".into(),aid:None,role:"scenario".into(),note:"".into()},name:"Writer".into(),character_name:None,spoiler:0})};assert!(validate("v1",&[change(),staff]).is_ok());}
 #[test]fn ambiguous_duplicate_or_cross_kind_replacements_fail(){let a=change();let mut b=a.clone();b.id="edit2".into();assert!(validate("v1",&[a,b]).is_err());let mut a=change();a.link.as_mut().unwrap().key=Key::Company{id:"p1".into()};a.link.as_mut().unwrap().character_name=None;assert!(validate("v1",&[a]).is_err());}
 #[test]fn invalid_labels_identity_spoilers_and_empty_actions_fail(){for field in 0..5{let mut a=change();let l=a.link.as_mut().unwrap();match field{0=>l.name="\n".into(),1=>l.spoiler=3,2=>l.character_name=None,3=>l.key=voice("p1",1),_=>l.key=voice("s1",-1)}assert!(validate("v1",&[a]).is_err());}assert!(validate("v1",&[Correction{id:"empty".into(),replaces:None,link:None}]).is_err());}
 #[test]fn serialized_intent_roundtrips_without_losing_credit_or_spoiler(){let a=change();let raw=serde_json::to_string(&a).unwrap();let b:Correction=serde_json::from_str(&raw).unwrap();assert_eq!(a,b);assert!(validate("v1",&[b]).is_ok());assert!(serde_json::from_str::<Correction>(r#"{"id":"x","replaces":null,"link":null,"unknown":true}"#).is_err());}
}

#[cfg(test)] mod staff_alias_tests {
 use super::*;
 fn staff(aid:Option<i64>)->Key {Key::Staff{id:"s1".into(),aid,role:"scenario".into(),note:"".into()}}
 #[test] fn staff_alias_identity_validation_and_legacy_json() {
  let legacy=r#"{"kind":"staff","id":"s1","role":"scenario","note":""}"#;
  let parsed:Key=serde_json::from_str(legacy).unwrap();
  assert_eq!(parsed,staff(None));
  // Omit None on serialization so pre-aid request receipt digests remain valid.
  assert_eq!(serde_json::to_string(&parsed).unwrap(),legacy);
  for aid in [None,Some(0),Some(17)] {
   let key=staff(aid);assert_eq!(serde_json::from_str::<Key>(&serde_json::to_string(&key).unwrap()).unwrap(),key);
  }
  let edits=vec![Correction{id:"a".into(),replaces:Some(staff(Some(1))),link:None},Correction{id:"b".into(),replaces:Some(staff(Some(2))),link:None}];
  assert!(validate("v1",&edits).is_ok());
  let mut duplicate=edits.clone();duplicate[1].replaces=Some(staff(Some(1)));assert!(validate("v1",&duplicate).is_err());
  let mut invalid=edits;invalid[0].replaces=Some(staff(Some(-1)));assert!(validate("v1",&invalid).is_err());
 }
}

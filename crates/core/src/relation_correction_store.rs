//! Durable exact-edge correction decisions; raw provider snapshots are never modified.
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use crate::{ApiError,relation_corrections::{Correction,validate}};
type R<T>=Result<T,ApiError>;
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Edit{pub revision:i64,pub request_id:String,pub corrections:Vec<Correction>}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS relation_corrections(vndb_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,corrections_json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS relation_correction_history(vndb_id TEXT NOT NULL,revision INTEGER NOT NULL,created INTEGER NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,PRIMARY KEY(vndb_id,revision)); CREATE TABLE IF NOT EXISTS relation_correction_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL);")}
pub fn read(c:&Connection,id:&str)->R<(i64,Vec<Correction>)>{
 validate(id,&[]).map_err(ApiError)?;
 let row:Option<(i64,String)>=c.prepare_cached("SELECT revision,corrections_json FROM relation_corrections WHERE vndb_id=?1")?.query_row([id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 match row{Some((revision,raw))=>Ok((revision,serde_json::from_str(&raw).map_err(|e|ApiError(e.to_string()))?)),None=>Ok((0,vec![]))}
}
/// Read an existing receipt before checking a preview; retries remain valid after later edits.
pub fn replay(c:&Connection,id:&str,input:&Edit)->R<Option<Value>>{
 use sha2::{Digest,Sha256};
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));
 let row:Option<(String,String)>=c.query_row("SELECT digest,result FROM relation_correction_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 match row{None=>Ok(None),Some((old,result))=>{if old!=digest{return Err(ApiError("Request identity already used for another correction".into()));}Ok(Some(serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()))?))}}
}
/// Own the transaction so callers cannot persist a decision without its history/receipt.
pub fn change(c:&mut Connection,id:&str,input:&Edit)->R<Value>{
 use sha2::{Digest,Sha256};validate(id,&input.corrections).map_err(ApiError)?;
 if input.revision<0||input.request_id.trim().is_empty()||input.request_id.len()>128||input.request_id.chars().any(char::is_control){return Err(ApiError("Invalid revision or request identity".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&(id,input)).map_err(|e|ApiError(e.to_string()))?));let tx=c.transaction()?;
 if let Some((old,result))=tx.query_row("SELECT digest,result FROM relation_correction_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{
  if old!=digest{return Err(ApiError("Request identity already used for another correction".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));
 }
 let(revision,before)=read(&tx,id)?;if revision!=input.revision{return Err(ApiError("Relationship corrections changed. Reload before editing.".into()));}
 let next=revision.checked_add(1).ok_or_else(||ApiError("Revision exhausted".into()))?;
 let before=serde_json::to_string(&before).map_err(|e|ApiError(e.to_string()))?;let after=serde_json::to_string(&input.corrections).map_err(|e|ApiError(e.to_string()))?;
 tx.execute("INSERT INTO relation_corrections(vndb_id,revision,corrections_json) VALUES(?1,?2,?3) ON CONFLICT(vndb_id) DO UPDATE SET revision=excluded.revision,corrections_json=excluded.corrections_json",params![id,next,after])?;
 tx.execute("INSERT INTO relation_correction_history(vndb_id,revision,created,before_json,after_json) VALUES(?1,?2,?3,?4,?5)",params![id,next,crate::db::now(),before,after])?;
 crate::relation_correction_index::replace(&tx,id,&input.corrections)?;
 tx.execute("UPDATE relation_correction_state SET revision=revision+1 WHERE singleton=1",[])?;
 let result=json!({"vndb_id":id,"revision":next});tx.execute("INSERT INTO relation_correction_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;tx.commit()?;Ok(result)
}
pub fn history(c:&Connection,id:&str,before:Option<i64>)->R<Value>{
 validate(id,&[]).map_err(ApiError)?;if before.is_some_and(|v|v<=0){return Err(ApiError("Invalid history cursor".into()));}
 let mut q=c.prepare("SELECT revision,created,before_json,after_json FROM relation_correction_history WHERE vndb_id=?1 AND (?2 IS NULL OR revision<?2) ORDER BY revision DESC LIMIT 51")?;
 let rows=q.query_map(params![id,before],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;let more=rows.len()>50;let mut items=vec![];
 for(revision,created,before,after)in rows.into_iter().take(50){items.push(json!({"revision":revision,"created":created,"before":serde_json::from_str::<Vec<Correction>>(&before).map_err(|e|ApiError(e.to_string()))?,"after":serde_json::from_str::<Vec<Correction>>(&after).map_err(|e|ApiError(e.to_string()))?}));}
 let next=if more{items.last().map(|v|v["revision"].clone())}else{None};Ok(json!({"items":items,"next":next}))
}
#[cfg(test)]mod tests{
 use super::*;use crate::relation_corrections::{Key,Link};
 fn edit(revision:i64,request:&str)->Edit{Edit{revision,request_id:request.into(),corrections:vec![Correction{id:"company-add".into(),replaces:None,link:Some(Link{key:Key::Company{id:"p1".into()},name:"Generated studio".into(),character_name:None,spoiler:0})}]}}
 #[test]fn receipts_conflicts_and_history_survive_reopen(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();{let mut c=db.lock().unwrap();let a=edit(0,"receipt");assert_eq!(change(&mut c,"v1",&a).ok().unwrap()["revision"],1);assert_eq!(change(&mut c,"v1",&a).ok().unwrap()["revision"],1);assert!(change(&mut c,"v2",&a).is_err());assert!(change(&mut c,"v1",&edit(0,"conflict")).is_err());assert_eq!(history(&c,"v1",None).ok().unwrap()["items"].as_array().unwrap().len(),1);}drop(db);
  let db=crate::db::open(&path).unwrap();let mut c=db.lock().unwrap();assert_eq!(read(&c,"v1").ok().unwrap().0,1);let before:Vec<Correction>=serde_json::from_value(history(&c,"v1",None).ok().unwrap()["items"][0]["before"].clone()).unwrap();change(&mut c,"v1",&Edit{revision:1,request_id:"undo".into(),corrections:before}).ok().unwrap();assert!(read(&c,"v1").ok().unwrap().1.is_empty());assert_eq!(history(&c,"v1",None).ok().unwrap()["items"].as_array().unwrap().len(),2);
 }
 #[test]fn receipt_failure_rolls_back_decision_and_history(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON relation_correction_requests BEGIN SELECT RAISE(ABORT,'generated failure'); END;").unwrap();assert!(change(&mut c,"v1",&edit(0,"fail")).is_err());assert_eq!(read(&c,"v1").ok().unwrap().0,0);assert_eq!(c.query_row("SELECT revision FROM relation_correction_state WHERE singleton=1",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM relation_correction_targets",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(history(&c,"v1",None).ok().unwrap()["items"].as_array().unwrap().len(),0);}
 #[test]fn history_cursor_is_stable_and_legacy_decisions_untouched(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();c.execute("INSERT INTO relation_overrides VALUES('v1',3,'{}')",[]).unwrap();for n in 0..52{change(&mut c,"v1",&edit(n,&format!("r{n}"))).ok().unwrap();}let first=history(&c,"v1",None).ok().unwrap();assert_eq!(first["next"],3);let second=history(&c,"v1",Some(3)).ok().unwrap();assert_eq!(second["items"].as_array().unwrap().len(),2);assert_eq!(second["items"][0]["revision"],2);assert!(second["next"].is_null());assert_eq!(c.query_row("SELECT revision FROM relation_overrides WHERE vndb_id='v1'",[],|r|r.get::<_,i64>(0)).unwrap(),3);}
}



#[cfg(test)] mod staff_alias_compatibility_tests {
 use super::*;
 #[test] fn legacy_staff_receipt_replays_and_aliases_survive_history_and_reopen() {
  use sha2::{Digest,Sha256};
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
  let legacy=r#"{"revision":0,"request_id":"legacy-staff","corrections":[{"id":"hide","replaces":{"kind":"staff","id":"s1","role":"scenario","note":""},"link":null}]}"#;
  let old:Edit=serde_json::from_str(legacy).unwrap();
  {
   let mut c=db.lock().unwrap();
   let digest=hex::encode(Sha256::digest(format!("[\"v1\",{legacy}]").as_bytes()));
   c.execute("INSERT INTO relation_corrections VALUES('v1',1,?1)",[serde_json::to_string(&old.corrections).unwrap()]).unwrap();
   c.execute("INSERT INTO relation_correction_requests VALUES('legacy-staff',?1,?2)",params![digest,json!({"vndb_id":"v1","revision":1}).to_string()]).unwrap();
   assert_eq!(change(&mut c,"v1",&old).ok().unwrap()["revision"],1);
   let mut edits=old.corrections.clone();
   edits[0].replaces=Some(crate::relation_corrections::Key::Staff{id:"s1".into(),aid:Some(12),role:"scenario".into(),note:"".into()});
   change(&mut c,"v1",&Edit{revision:1,request_id:"with-alias".into(),corrections:edits}).ok().unwrap();
   let history=history(&c,"v1",None).ok().unwrap();assert_eq!(history["items"][0]["after"][0]["replaces"]["aid"],12);
   assert!(history["items"][0]["before"][0]["replaces"].get("aid").is_none());
  }
  drop(db);let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();
  assert_eq!(serde_json::to_value(&read(&c,"v1").ok().unwrap().1).unwrap()[0]["replaces"]["aid"],12);
 }
}

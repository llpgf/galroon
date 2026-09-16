//! Read-only projection of persisted matching evidence. Candidate prose/images are omitted.
use rusqlite::{Connection,params};
use serde_json::{Value,json};
pub fn page(c:&Connection,id:&str,before:Option<i64>)->Result<Value,String>{
 history(c,id,before,false)
}
pub fn attempts(c:&Connection,id:&str,before:Option<i64>)->Result<Value,String>{history(c,id,before,true)}
fn history(c:&Connection,id:&str,before:Option<i64>,attempts:bool)->Result<Value,String>{
 let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
 if !exists{return Err("Resource not found".into());}
 if before.is_some_and(|n|n<=0){return Err("Invalid matching history cursor".into());}
 let query=if attempts{"SELECT e.seq,e.job_id,json_extract(e.payload,'$.revision'),json_extract(e.payload,'$.state'),json_extract(e.payload,'$.evidence'),json_extract(e.payload,'$.work_id'),e.created,j.state FROM events e JOIN jobs j ON j.id=e.job_id WHERE e.code='match.attempt' AND json_extract(e.payload,'$.resource_id')=?1 AND e.seq<?2 ORDER BY e.seq DESC LIMIT 21"}else{"SELECT m.rowid,m.job_id,m.revision,m.state,m.evidence,m.work_id,j.created,j.state FROM match_items m JOIN jobs j ON j.id=m.job_id WHERE m.resource_id=?1 AND m.rowid<?2 ORDER BY m.rowid DESC LIMIT 21"};
 let mut s=c.prepare(query).map_err(|e|e.to_string())?;
 let rows=s.query_map(params![id,before.unwrap_or(i64::MAX)],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,i64>(6)?,r.get::<_,String>(7)?))).map_err(|e|e.to_string())?;
 let mut items=Vec::new();for row in rows{let(seq,job,revision,state,raw,work,created,job_state)=row.map_err(|e|e.to_string())?;let evidence:Value=serde_json::from_str(&raw).map_err(|_|"Stored matching evidence is unreadable")?;
  let candidates:Vec<Value>=evidence["results"].as_array().into_iter().flatten().take(50).map(|v|json!({"id":v["id"],"title":v["title"],"original_title":v["alttitle"],"strength":v["match"]["strength"],"reason":v["match"]["reason"],"ambiguous":v["match"]["ambiguous"]})).collect();
  items.push(json!({"seq":seq,"job_id":job,"revision":revision,"state":state,"job_state":job_state,"work_id":work,"created":created,"recorded_at":evidence["recorded_at"],"algorithm_version":evidence["algorithm_version"],"source":evidence["source"],"titles":evidence["titles"],"reason":evidence["error"].as_str().or_else(||evidence["reason"].as_str()).map(str::to_owned).or_else(||if state=="review"{crate::automatch::review_reason(&evidence)}else{None}),"candidates":candidates}));
 }
 let more=items.len()>20;items.truncate(20);let next=if more{items.last().map(|v|v["seq"].clone()).unwrap_or(Value::Null)}else{Value::Null};Ok(json!({"items":items,"next":next}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn cursor_projection_is_bounded_and_survives_reopen(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
  {let c=db.lock().unwrap();c.execute_batch("INSERT INTO roots(id,path,label) VALUES('r','generated','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('a','r','a','Alpha','archive');").unwrap();
   for i in 1..=23{let job=format!("j{i}");c.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES(?1,'match','completed','{}',1,1)",[&job]).unwrap();let evidence=json!({"titles":["Alpha"],"results":[{"id":"v1","title":"Alpha","description":"omit candidate prose","image":{"url":"omit image"},"match":{"reason":"exact_title","strength":"strong"}}]});c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,evidence) VALUES(?1,'a',1,'review',?2)",params![job,evidence.to_string()]).unwrap();}
   let first=page(&c,"a",None).unwrap();assert_eq!(first["items"].as_array().unwrap().len(),20);assert_eq!(first["next"],4);assert!(!first.to_string().contains("omit"));assert!(first["items"][0]["source"].is_null());
   assert!(page(&c,"missing",None).is_err());assert!(page(&c,"a",Some(0)).is_err());
  }drop(db);let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();let second=page(&c,"a",Some(4)).unwrap();assert_eq!(second["items"].as_array().unwrap().len(),3);assert_eq!(second["items"][0]["seq"],3);assert!(second["next"].is_null());
 }
}

use axum::{extract::{State,Path},http::HeaderMap,Json};
use rusqlite::{Connection,params,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use crate::{App,ApiError,Result,issues,scan};
#[derive(Deserialize)]pub struct Request{revision:i64}
#[derive(Deserialize)]pub struct Confirm{revision:i64,digest:String}
#[derive(Serialize)]pub struct Preview{root_id:String,label:String,path:String,exclude:Vec<String>,digest:String}
fn desktop(a:&App,h:&HeaderMap)->std::result::Result<(),ApiError>{if crate::access::authenticate(a,h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(())}
pub fn inspect(c:&Connection,id:i64,revision:i64)->std::result::Result<Preview,String>{
 issues::reconcile(c)?;
 let(root,label,path,key):(String,String,String,String)=c.query_row("SELECT rt.id,rt.label,rt.path,i.evidence_key FROM issues i LEFT JOIN resources r ON r.id=i.resource_id JOIN roots rt ON rt.id=coalesce(i.root_id,r.root_id) WHERE i.seq=?1 AND i.revision=?2 AND i.category IN ('availability','source') AND i.state IN ('open','deferred')",params![id,revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_|"Issue changed. Refresh before reviewing the source.")?;
 let raw:Option<String>=c.query_row("SELECT exclude FROM source_watch WHERE root_id=?1",[&root],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 let exclude:Vec<String>=serde_json::from_str(raw.as_deref().unwrap_or("[]")).map_err(|_|"Source exclusions are invalid")?;
 let digest=hex::encode(Sha256::digest(json!([id,revision,root,path,exclude,key]).to_string().as_bytes()));Ok(Preview{root_id:root,label,path,exclude,digest})
}
pub async fn preview(State(a):State<App>,h:HeaderMap,Path(id):Path<i64>,Json(r):Json<Request>)->Result<Value>{desktop(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let p=inspect(&tx,id,r.revision)?;tx.commit()?;Ok(Json(json!(p)))}
pub fn enqueue(c:&Connection,id:i64,revision:i64,digest:&str)->std::result::Result<(String,bool),String>{
 let old:Option<String>=c.query_row("SELECT id FROM jobs WHERE kind='scan' AND json_extract(spec,'$.issue_id')=?1 AND json_extract(spec,'$.issue_revision')=?2 AND json_extract(spec,'$.issue_digest')=?3 ORDER BY rowid LIMIT 1",params![id,revision,digest],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 if let Some(job)=old{return Ok((job,false));}
 let p=inspect(c,id,revision)?;if p.digest!=digest{return Err("Source or exclusions changed. Review the scan again.".into());}
 let job=scan::enqueue(c,scan::ScanSpec{root_id:p.root_id,scope:String::new(),exclude:p.exclude},true,false)?;
 c.execute("UPDATE jobs SET spec=json_set(spec,'$.issue_id',?2,'$.issue_revision',?3,'$.issue_digest',?4) WHERE id=?1",params![job,id,revision,digest]).map_err(|e|e.to_string())?;
 issues::change(c,id,revision,"open")?;Ok((job,true))
}
pub async fn confirm(State(a):State<App>,h:HeaderMap,Path(id):Path<i64>,Json(r):Json<Confirm>)->Result<Value>{
 desktop(&a,&h)?;let(job,created)={let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=enqueue(&tx,id,r.revision,&r.digest)?;tx.commit()?;result};
 if created{let db=a.db.clone();let run=job.clone();tokio::task::spawn_blocking(move||scan::run(db,run));}Ok(Json(json!({"job_id":job})))
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->(tempfile::TempDir,crate::db::Db,i64){let t=tempfile::tempdir().unwrap();let root=t.path().join("source");std::fs::create_dir(&root).unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let id={let c=db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('root',?1,'Fixture')",[root.to_str().unwrap()]).unwrap();c.execute_batch("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('r','root','a','Alpha','archive'); INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES('f','root','a','a',1,'1','zip','fixture','missing'); INSERT INTO resource_files(resource_id,file_id) VALUES('r','f'); INSERT INTO source_watch(root_id,path,exclude) VALUES('root','source','[\"skip\"]');").unwrap();issues::reconcile(&c).unwrap();c.query_row("SELECT seq FROM issues WHERE category='availability'",[],|r|r.get(0)).unwrap()};(t,db,id)}
 #[test]fn scan_uses_reviewed_source_and_replays_one_job(){let(_t,db,id)=fixture();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let p=inspect(&tx,id,1).unwrap();assert_eq!(p.exclude,vec!["skip"]);assert_eq!(tx.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);let(job,created)=enqueue(&tx,id,1,&p.digest).unwrap();assert!(created);let replay=enqueue(&tx,id,1,&p.digest).unwrap();assert_eq!(replay,(job.clone(),false));let raw:String=tx.query_row("SELECT spec FROM jobs WHERE id=?1",[job],|r|r.get(0)).unwrap();let spec:Value=serde_json::from_str(&raw).unwrap();assert_eq!(spec["root_id"],"root");assert_eq!(spec["scope"],"");assert_eq!(spec["exclude"],json!(["skip"]));assert_eq!(spec["automatic"],false);assert_eq!(spec["recursive"],true);tx.commit().unwrap();}
 #[test]fn stale_exclusions_evidence_and_busy_source_reject_without_jobs(){let(_t,db,id)=fixture();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let p=inspect(&tx,id,1).unwrap();tx.execute("UPDATE source_watch SET exclude='[]'",[]).unwrap();assert!(enqueue(&tx,id,1,&p.digest).is_err());let p=inspect(&tx,id,1).unwrap();tx.execute("UPDATE files SET availability='unverified'",[]).unwrap();assert!(enqueue(&tx,id,1,&p.digest).is_err());let p=inspect(&tx,id,2).unwrap();tx.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('busy','scan','running','{}',1,1)",[]).unwrap();assert!(enqueue(&tx,id,2,&p.digest).is_err());assert_eq!(tx.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
 #[test]fn transaction_rolls_back_queue_if_issue_update_fails(){let(_t,db,id)=fixture();let mut c=db.lock().unwrap();let p=inspect(&c,id,1).unwrap();c.execute_batch("CREATE TRIGGER reject_triage BEFORE UPDATE ON issues BEGIN SELECT RAISE(ABORT,'generated failure'); END;").unwrap();{let tx=c.transaction().unwrap();assert!(enqueue(&tx,id,1,&p.digest).is_err());}assert_eq!(c.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM frontier",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
 #[test]fn reviewed_scan_runs_and_preserves_generated_source_bytes(){let(t,db,id)=fixture();let root=t.path().join("source");let game=root.join("game.zip");std::fs::write(&game,b"generated archive bytes").unwrap();let job={let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();let p=inspect(&tx,id,1).unwrap();let(job,_)=enqueue(&tx,id,1,&p.digest).unwrap();tx.commit().unwrap();job};scan::run(db.clone(),job.clone());let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");assert_eq!(std::fs::read(&game).unwrap(),b"generated archive bytes");assert_eq!(c.query_row("SELECT count(*) FROM files WHERE relative_path='game.zip' AND availability='present'",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
}


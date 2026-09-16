use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
use rusqlite::{Connection,params};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{Value,json};
use crate::{App,ApiError,Result,db};
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 const CREATE:&str="CREATE TABLE IF NOT EXISTS issues(seq INTEGER PRIMARY KEY AUTOINCREMENT,resource_id TEXT,root_id TEXT,category TEXT NOT NULL DEFAULT 'matching',kind TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'open',reason TEXT NOT NULL,evidence_key TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,created INTEGER NOT NULL,updated INTEGER NOT NULL,UNIQUE(resource_id,category),UNIQUE(root_id,category));";
 c.execute_batch(CREATE)?;
 let modern:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('issues') WHERE name='category')",[],|r|r.get(0))?;
 if !modern{c.execute_batch("ALTER TABLE issues RENAME TO issues_v14;")?;c.execute_batch(CREATE)?;c.execute_batch("INSERT INTO issues(seq,resource_id,kind,state,reason,evidence_key,revision,created,updated) SELECT seq,resource_id,kind,state,reason,evidence_key,revision,created,updated FROM issues_v14; DROP TABLE issues_v14;")?;}
 let rooted:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('issues') WHERE name='root_id')",[],|r|r.get(0))?;
 if !rooted{c.execute_batch("ALTER TABLE issues RENAME TO issues_v15;")?;c.execute_batch(CREATE)?;c.execute_batch("INSERT INTO issues(seq,resource_id,category,kind,state,reason,evidence_key,revision,created,updated) SELECT seq,resource_id,category,kind,state,reason,evidence_key,revision,created,updated FROM issues_v15; DROP TABLE issues_v15;")?;}
 let jobbed:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('issues') WHERE name='job_id')",[],|r|r.get(0))?;if !jobbed{c.execute_batch("ALTER TABLE issues ADD COLUMN job_id TEXT;")?;}
 c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS issue_job ON issues(job_id,category); CREATE INDEX IF NOT EXISTS issue_state ON issues(state,seq);")
}
// Reconcile inside the caller's transaction. Repeated observations preserve deferrals.
pub fn reconcile(c:&Connection)->std::result::Result<(),String>{
 let run=||->rusqlite::Result<()>{
  let mut q=c.prepare("SELECT r.id,r.revision,m.state,m.evidence FROM resources r LEFT JOIN match_items m ON m.rowid=(SELECT max(rowid) FROM match_items WHERE resource_id=r.id AND evidence!='{}') WHERE r.work_id IS NULL AND NOT EXISTS(SELECT 1 FROM resource_bindings b WHERE b.resource_id=r.id) AND EXISTS(SELECT 1 FROM resource_files f WHERE f.resource_id=r.id)")?;
  let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
  for(id,revision,state,raw)in rows{let e:Value=raw.as_deref().and_then(|s|serde_json::from_str(s).ok()).unwrap_or_default();let(kind,reason)=if let Some(error)=e["error"].as_str(){("query_failed",error.to_string())}else if state.as_deref()==Some("skipped"){("skipped",e["reason"].as_str().unwrap_or("Source changed during matching").into())}else{("unmatched",crate::automatch::review_reason(&e).unwrap_or_else(||"No confirmed work".into()))};
   let key=json!([revision,kind,reason,e["algorithm_version"],e["results"]]).to_string();
   c.execute("INSERT INTO issues(resource_id,kind,reason,evidence_key,created,updated) VALUES(?1,?2,?3,?4,?5,?5) ON CONFLICT(resource_id,category) DO UPDATE SET kind=excluded.kind,reason=excluded.reason,evidence_key=excluded.evidence_key,state='open',revision=issues.revision+1,updated=excluded.updated WHERE issues.evidence_key!=excluded.evidence_key OR issues.state='resolved'",params![id,kind,reason,key,db::now()])?;
  }
  c.execute("UPDATE issues SET state='resolved',revision=revision+1,updated=?1 WHERE category='matching' AND state!='resolved' AND NOT EXISTS(SELECT 1 FROM resources r WHERE r.id=issues.resource_id AND r.work_id IS NULL AND NOT EXISTS(SELECT 1 FROM resource_bindings b WHERE b.resource_id=r.id) AND EXISTS(SELECT 1 FROM resource_files f WHERE f.resource_id=r.id))",[db::now()])?;Ok(())
 };run().map_err(|e|e.to_string())?;availability(c).map_err(|e|e.to_string())?;crate::source_issues::reconcile(c).map_err(|e|e.to_string())?;crate::acquire_issues::reconcile(c).map_err(|e|e.to_string())
}
fn availability(c:&Connection)->rusqlite::Result<()>{
 let mut statement=c.prepare("SELECT rf.resource_id,f.id,f.availability,f.relative_path,f.size,f.mtime FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE f.availability IN ('missing','unverified') ORDER BY rf.resource_id,f.id")?;
 let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,json!([r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,String>(5)?]))))?;
 let mut evidence=std::collections::BTreeMap::<String,Vec<Value>>::new();for row in rows{let(id,file)=row?;evidence.entry(id).or_default().push(file);}
 for(id,files)in evidence{let missing=files.iter().filter(|f|f[1]=="missing").count();let unverified=files.len()-missing;let kind=if missing>0{"missing"}else{"unverified"};let reason=format!("{missing} files missing after enumeration; {unverified} files not yet verified. Review the source and scan before changing files.");
  c.execute("INSERT INTO issues(resource_id,category,kind,reason,evidence_key,created,updated) VALUES(?1,'availability',?2,?3,?4,?5,?5) ON CONFLICT(resource_id,category) DO UPDATE SET kind=excluded.kind,reason=excluded.reason,evidence_key=excluded.evidence_key,state='open',revision=issues.revision+1,updated=excluded.updated WHERE issues.evidence_key!=excluded.evidence_key OR issues.state='resolved'",params![id,kind,reason,json!(files).to_string(),db::now()])?;
 }
 c.execute("UPDATE issues SET state='resolved',revision=revision+1,updated=?1 WHERE category='availability' AND state!='resolved' AND NOT EXISTS(SELECT 1 FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=issues.resource_id AND f.availability IN ('missing','unverified'))",[db::now()])?;Ok(())
}
fn desktop(a:&App,h:&HeaderMap)->std::result::Result<(),ApiError>{if crate::access::authenticate(a,h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(())}
#[derive(Deserialize)]pub struct List{#[serde(default="open")]state:String,before:Option<i64>}
fn open()->String{"open".into()}
pub async fn list(State(a):State<App>,h:HeaderMap,Query(q):Query<List>)->Result<Value>{
 desktop(&a,&h)?;if !["open","deferred","resolved"].contains(&q.state.as_str())||q.before.is_some_and(|n|n<=0){return Err(ApiError("Invalid issue filter".into()));}
 let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;reconcile(&tx)?;
 let rows={let mut s=tx.prepare("SELECT i.seq,i.resource_id,i.kind,i.state,i.reason,i.revision,coalesce(r.title,rt.label,j.id),coalesce(r.relative_path,rt.path,json_extract(j.spec,'$.final_folder')),rt.label,EXISTS(SELECT 1 FROM jobs j WHERE j.kind IN ('match','scan') AND json_extract(j.spec,'$.issue_id')=i.seq AND j.state IN ('queued','running','pausing','cancelling')),i.category,coalesce(i.root_id,r.root_id),i.job_id FROM issues i LEFT JOIN jobs j ON j.id=i.job_id LEFT JOIN resources r ON r.id=coalesce(i.resource_id,json_extract(j.spec,'$.resource_id')) LEFT JOIN roots rt ON rt.id=coalesce(i.root_id,r.root_id) WHERE i.state=?1 AND i.seq<?2 ORDER BY i.seq DESC LIMIT 51")?;let rows=s.query_map(params![q.state,q.before.unwrap_or(i64::MAX)],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"resource_id":r.get::<_,Option<String>>(1)?,"kind":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?,"reason":r.get::<_,String>(4)?,"revision":r.get::<_,i64>(5)?,"title":r.get::<_,Option<String>>(6)?,"path":r.get::<_,Option<String>>(7)?,"source":r.get::<_,Option<String>>(8)?,"retry_pending":r.get::<_,bool>(9)?,"category":r.get::<_,String>(10)?,"root_id":r.get::<_,Option<String>>(11)?,"job_id":r.get::<_,Option<String>>(12)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};tx.commit()?;
 let more=rows.len()>50;let mut rows=rows;rows.truncate(50);let next=if more{rows.last().map(|r|r["id"].clone()).unwrap_or(Value::Null)}else{Value::Null};Ok(Json(json!({"items":rows,"next":next})))
}
#[derive(Deserialize)]pub struct Edit{revision:i64,state:String}
pub async fn edit(State(a):State<App>,h:HeaderMap,Path(id):Path<i64>,Json(e):Json<Edit>)->Result<Value>{desktop(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;reconcile(&tx)?;change(&tx,id,e.revision,&e.state)?;tx.commit()?;Ok(Json(json!({"id":id})))}
pub fn change(c:&Connection,id:i64,revision:i64,state:&str)->std::result::Result<(),String>{
 if !["open","deferred"].contains(&state){return Err("Choose open or deferred".into());}
 let n=c.execute("UPDATE issues SET state=?3,revision=revision+1,updated=?4 WHERE seq=?1 AND revision=?2 AND state IN ('open','deferred')",params![id,revision,state,db::now()]).map_err(|e|e.to_string())?;if n!=1{return Err("Issue changed. Refresh before trying again.".into());}crate::db::event(c,"","issue.triage",&json!({"issue_id":id,"revision":revision,"state":state})).map_err(|e|e.to_string())?;Ok(())
}
#[derive(Deserialize)]pub struct Retry{revision:i64}
pub async fn retry(State(a):State<App>,h:HeaderMap,Path(id):Path<i64>,Json(e):Json<Retry>)->Result<Value>{
 desktop(&a,&h)?;let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;
 let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let job=queue_retry(&tx,id,e.revision)?;tx.commit()?;Ok(Json(json!({"job_id":job})))
}
pub fn queue_retry(c:&Connection,id:i64,revision:i64)->std::result::Result<String,String>{
 let sql=|e:rusqlite::Error|e.to_string();
 // The reviewed issue revision is the durable request key; retries do not enqueue twice.
 let prior:Option<String>=c.query_row("SELECT id FROM jobs WHERE kind='match' AND json_extract(spec,'$.issue_id')=?1 AND json_extract(spec,'$.issue_revision')=?2 ORDER BY rowid LIMIT 1",params![id,revision],|r|r.get(0)).optional().map_err(sql)?;
 if let Some(job)=prior{return Ok(job);}
 reconcile(c)?;
 let source:Option<(String,i64)>=c.query_row("SELECT r.id,r.revision FROM issues i JOIN resources r ON r.id=i.resource_id WHERE i.seq=?1 AND i.revision=?2 AND i.state IN ('open','deferred') AND i.category='matching'",params![id,revision],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
 let(resource,source_revision)=source.ok_or("Issue changed. Refresh before retrying.")?;crate::roots::require_idle(c)?;
 let job=db::id();c.execute("INSERT INTO jobs(id,kind,state,spec,discovered,created,updated,message) VALUES(?1,'match','queued',?2,1,?3,?3,'Retrying one matching issue')",params![job,json!({"issue_id":id,"issue_revision":revision,"resource_id":resource,"automatic":false}).to_string(),db::now()]).map_err(sql)?;
 c.execute("INSERT INTO match_items(job_id,resource_id,revision) VALUES(?1,?2,?3)",params![job,resource,source_revision]).map_err(sql)?;
 change(c,id,revision,"open")?;Ok(job)
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->(tempfile::TempDir,db::Db){let t=tempfile::tempdir().unwrap();let db=db::open(&t.path().join("library.sqlite")).unwrap();db.lock().unwrap().execute_batch("INSERT INTO roots(id,path,label) VALUES('root','generated','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('r','root','a','Alpha','archive'); INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES('f','root','a','a',1,'1','zip','fixture'); INSERT INTO resource_files(resource_id,file_id) VALUES('r','f');").unwrap();(t,db)}
 #[test]fn deferral_survives_refresh_restore_and_reopens_on_new_evidence(){
  let(t,db)=fixture();{let c=db.lock().unwrap();reconcile(&c).unwrap();change(&c,1,1,"deferred").unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"deferred");assert!(change(&c,1,1,"open").is_err());}
  let backup=crate::backup::export(&db,t.path()).unwrap();let dest=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=db::open(&dest.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"deferred");
  c.execute("UPDATE resources SET revision=revision+1",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"open");assert!(change(&c,1,2,"deferred").is_err());
  c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Alpha','Alpha'); UPDATE resources SET work_id='w';").unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"resolved");assert!(change(&c,1,4,"open").is_err());
  c.execute("UPDATE resources SET work_id=NULL",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"open");
 }
 #[test]fn schema_thirteen_upgrade_preserves_resources_and_creates_protection(){
  let(t,db)=fixture();db.lock().unwrap().execute_batch("DROP TABLE issues; PRAGMA user_version=13;").unwrap();drop(db);let db=db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM issues",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert!(std::fs::read_dir(t.path().join("migration-backups")).unwrap().next().is_some());
 }
 #[test]fn retry_rejects_new_evidence_and_busy_work_without_queuing(){
  let(_t,db)=fixture();let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();reconcile(&tx).unwrap();tx.execute("UPDATE resources SET revision=revision+1",[]).unwrap();assert!(queue_retry(&tx,1,1).is_err());
  tx.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('busy','scan','running','{}',1,1)",[]).unwrap();assert!(queue_retry(&tx,1,2).is_err());assert_eq!(tx.query_row("SELECT count(*) FROM jobs WHERE kind='match'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn availability_is_independent_of_matching_and_preserves_deferral(){
  let(_t,db)=fixture();let c=db.lock().unwrap();c.execute("UPDATE files SET availability='missing'",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM issues",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  let id:i64=c.query_row("SELECT seq FROM issues WHERE category='availability'",[],|r|r.get(0)).unwrap();change(&c,id,1,"deferred").unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE seq=?1",[id],|r|r.get::<_,String>(0)).unwrap(),"deferred");assert!(queue_retry(&c,id,2).is_err());
  c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Alpha','Alpha'); UPDATE resources SET work_id='w';").unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE category='matching'",[],|r|r.get::<_,String>(0)).unwrap(),"resolved");assert_eq!(c.query_row("SELECT state FROM issues WHERE category='availability'",[],|r|r.get::<_,String>(0)).unwrap(),"deferred");
  c.execute("UPDATE files SET availability='unverified'",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT kind||':'||state FROM issues WHERE category='availability'",[],|r|r.get::<_,String>(0)).unwrap(),"unverified:open");
  c.execute("UPDATE files SET availability='present'",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM issues WHERE state='resolved'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn schema_fourteen_upgrade_preserves_issue_identity_and_deferral(){
  let(t,db)=fixture();{let c=db.lock().unwrap();reconcile(&c).unwrap();change(&c,1,1,"deferred").unwrap();c.execute_batch("ALTER TABLE issues RENAME TO modern_fixture; CREATE TABLE issues(seq INTEGER PRIMARY KEY AUTOINCREMENT,resource_id TEXT NOT NULL UNIQUE,kind TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'open',reason TEXT NOT NULL,evidence_key TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,created INTEGER NOT NULL,updated INTEGER NOT NULL); INSERT INTO issues SELECT seq,resource_id,kind,state,reason,evidence_key,revision,created,updated FROM modern_fixture; DROP TABLE modern_fixture; PRAGMA user_version=14;").unwrap();}drop(db);
  let db=db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT seq||':'||category||':'||state||':'||revision FROM issues",[],|r|r.get::<_,String>(0)).unwrap(),"1:matching:deferred:2");c.execute("UPDATE files SET availability='missing'",[]).unwrap();reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM issues",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
}






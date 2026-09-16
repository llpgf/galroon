//! Durable catalog-only automatic matching. No filesystem mutations.
use crate::{db::{self,Db},editions,matching};
use rusqlite::{Connection,OptionalExtension,params};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
type R<T>=Result<T,String>;
fn sql<T>(v:rusqlite::Result<T>)->R<T>{v.map_err(|e|e.to_string())}
pub fn migrate(c:&Connection)->R<()> {
    sql(c.execute_batch("CREATE TABLE IF NOT EXISTS match_items(job_id TEXT NOT NULL REFERENCES jobs(id),resource_id TEXT NOT NULL REFERENCES resources(id),revision INTEGER NOT NULL,state TEXT NOT NULL DEFAULT 'pending',evidence TEXT NOT NULL DEFAULT '{}',work_id TEXT,PRIMARY KEY(job_id,resource_id)); CREATE INDEX IF NOT EXISTS match_pending ON match_items(job_id,state); CREATE INDEX IF NOT EXISTS match_resource ON match_items(resource_id,revision,state); CREATE INDEX IF NOT EXISTS intake_events ON events(code,job_id,seq); CREATE INDEX IF NOT EXISTS match_attempt_resource ON events(json_extract(payload,'$.resource_id'),seq) WHERE code='match.attempt';"))
}
fn scan_key(c:&Connection)->R<String>{Ok(sql(c.query_row("SELECT id||':'||updated FROM jobs WHERE kind='scan' AND state='completed' ORDER BY updated DESC,rowid DESC LIMIT 1",[],|r|r.get(0)).optional())?.unwrap_or_default())}
fn set(c:&Connection,key:&str,value:&str)->R<()>{sql(c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]))?;Ok(())}
pub fn enabled(c:&Connection)->R<bool>{Ok(sql(c.query_row("SELECT value FROM settings WHERE key='auto_match_enabled'",[],|r|r.get::<_,String>(0)).optional())?.as_deref()==Some("1"))}
fn active(c:&Connection)->R<Option<String>>{sql(c.query_row("SELECT id FROM jobs WHERE kind='match' AND state IN ('queued','running','pausing','cancelling') ORDER BY rowid DESC LIMIT 1",[],|r|r.get(0)).optional())}
/// Idempotent bootstrap; explicit refresh retries all remaining unmatched resources.
pub fn kick(db:&Db,refresh:bool)->R<Option<String>> {
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;
    if let Some(id)=active(&tx)? {return Ok(Some(id));}
    let first=!enabled(&tx)?;set(&tx,"auto_match_enabled","1")?;
    let version=sql(tx.query_row("SELECT value FROM settings WHERE key='auto_match_version'",[],|r|r.get::<_,String>(0)).optional())?;
    if !refresh && !first && version.as_deref()==Some(&matching::ALGORITHM_VERSION.to_string()) {sql(tx.commit())?;return Ok(None);}
    if !refresh {
        let stopped:bool=sql(tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE kind='match' AND state IN ('paused','interrupted'))",[],|r|r.get(0)))?;
        if stopped {sql(tx.commit())?;return Ok(None);}
    }
    if refresh {sql(tx.execute("UPDATE jobs SET state='cancelled',message='Replaced by a refreshed matching sequence',updated=?1 WHERE kind='match' AND state IN ('paused','interrupted')",[db::now()]))?;}
    let id=queue(&tx,refresh)?;sql(tx.commit())?;Ok(id)
}
fn queue(c:&Connection,refresh:bool)->R<Option<String>> {
    let busy:bool=sql(c.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE kind!='match' AND state IN ('queued','running','pausing','cancelling')) OR EXISTS(SELECT 1 FROM plans WHERE state='executing')",[],|r|r.get(0)))?;
    if busy {set(c,"auto_match_requested","1")?;return Ok(None);}
    let filter=format!("r.work_id IS NULL AND NOT EXISTS(SELECT 1 FROM resource_bindings b WHERE b.resource_id=r.id) AND EXISTS(SELECT 1 FROM resource_files f WHERE f.resource_id=r.id) AND (?1 OR NOT EXISTS(SELECT 1 FROM match_items m WHERE m.resource_id=r.id AND m.revision=r.revision AND m.state='review' AND json_extract(m.evidence,'$.algorithm_version')={}))",matching::ALGORITHM_VERSION);
    let count:i64=sql(c.query_row(&format!("SELECT count(*) FROM resources r WHERE {filter}"),[refresh],|r|r.get(0)))?;
    set(c,"auto_match_scan",&scan_key(c)?)?;set(c,"auto_match_requested","0")?;set(c,"auto_match_version",&matching::ALGORITHM_VERSION.to_string())?;
    if count==0{return Ok(None);}
    let id=db::id();sql(c.execute("INSERT INTO jobs(id,kind,state,spec,discovered,created,updated,message) VALUES(?1,'match','queued',?2,?3,?4,?4,'Matching sequence queued')",params![id,json!({"automatic":!refresh}).to_string(),count,db::now()]))?;
    sql(c.execute(&format!("INSERT INTO match_items(job_id,resource_id,revision) SELECT ?2,r.id,r.revision FROM resources r WHERE {filter}"),params![refresh,id]))?;Ok(Some(id))
}
pub fn status(db:&Db)->R<Value>{let c=db.lock().map_err(|e|e.to_string())?;
    let latest=sql(c.query_row("SELECT id,state,processed,discovered,current_path,message FROM jobs WHERE kind='match' ORDER BY rowid DESC LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).optional())?;
    let job=if let Some((id,state,processed,total,current,message))=latest{let counts: (i64,i64,i64)=sql(c.query_row("SELECT coalesce(sum(state='matched'),0),coalesce(sum(state='review'),0),coalesce(sum(state='skipped'),0) FROM match_items WHERE job_id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))))?;json!({"id":id,"state":state,"processed":processed,"total":total,"current":current,"message":message,"matched":counts.0,"review":counts.1,"skipped":counts.2})}else{Value::Null};
    let waiting=sql(c.query_row("SELECT value FROM settings WHERE key='auto_match_requested'",[],|r|r.get::<_,String>(0)).optional())?.as_deref()==Some("1");
    let watch:Value=sql(c.query_row("SELECT count(*),coalesce(sum(enabled=1 AND status='watching'),0),coalesce(sum(enabled=1 AND status IN ('offline','error')),0),coalesce(sum(enabled=1 AND needs_scan=1),0),coalesce(sum(json_array_length(pending)),0) FROM source_watch",[],|r|Ok(json!({"total":r.get::<_,i64>(0)?,"watching":r.get::<_,i64>(1)?,"unavailable":r.get::<_,i64>(2)?,"reconciling":r.get::<_,i64>(3)?,"pending":r.get::<_,i64>(4)?}))))?;
    let mut s=sql(c.prepare("SELECT max(e.seq),e.job_id,e.code,count(*),max(e.created),coalesce(r.label,''),max(e.payload) FROM events e LEFT JOIN jobs j ON j.id=e.job_id LEFT JOIN roots r ON r.id=json_extract(j.spec,'$.root_id') WHERE e.code IN ('resource.discovered','match.completed') GROUP BY e.job_id,e.code ORDER BY max(e.seq) DESC LIMIT 20"))?;
    let updates=sql(sql(s.query_map([],|r|Ok(json!({"seq":r.get::<_,i64>(0)?,"job_id":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"count":r.get::<_,i64>(3)?,"created":r.get::<_,i64>(4)?,"source":r.get::<_,String>(5)?,"data":serde_json::from_str::<Value>(&r.get::<_,String>(6)?).unwrap_or_default()}))))?.collect::<rusqlite::Result<Vec<_>>>())?;
    Ok(json!({"enabled":enabled(&c)?,"waiting":waiting,"job":job,"watch":watch,"updates":updates}))
}
pub struct Worker(tokio::task::JoinHandle<()>,std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Worker{pub fn request_stop(&self){self.1.store(true,std::sync::atomic::Ordering::SeqCst);}}
impl Drop for Worker{fn drop(&mut self){self.0.abort();}}
pub fn start(db:Db)->Worker {let stop=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));let stopped=stop.clone();Worker(tokio::spawn(async move{loop{
    let next={let result=(||->R<Option<String>>{let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;
        if stopped.load(std::sync::atomic::Ordering::SeqCst){return Ok(None);}
        if !enabled(&tx)?{return explicit_retry(&tx);}
        if let Some(id)=active(&tx)?{return Ok(Some(id));}
        let stopped:bool=sql(tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE kind='match' AND state IN ('paused','interrupted'))",[],|r|r.get(0)))?;
        let key=scan_key(&tx)?;let old=sql(tx.query_row("SELECT value FROM settings WHERE key='auto_match_scan'",[],|r|r.get::<_,String>(0)).optional())?.unwrap_or_default();
        let requested=sql(tx.query_row("SELECT value FROM settings WHERE key='auto_match_requested'",[],|r|r.get::<_,String>(0)).optional())?.as_deref()==Some("1");
        let version=sql(tx.query_row("SELECT value FROM settings WHERE key='auto_match_version'",[],|r|r.get::<_,String>(0)).optional())?;
        let id=if requested||(!stopped&&(key!=old||version.as_deref()!=Some(&matching::ALGORITHM_VERSION.to_string()))){queue(&tx,false)?}else{None};sql(tx.commit())?;Ok(id)
    })();match result{Ok(v)=>v,Err(e)=>{crate::diagnostics::record("error","matching.scheduler",&e,None);eprintln!("Matching scheduler: {e}");None}}};
    if let Some(id)=next {if let Err(e)=run(&db,&id).await {crate::diagnostics::record("error","matching.failed",&e,Some(&id));if let Ok(c)=db.lock(){let _=c.execute("UPDATE jobs SET state='failed',message=?2,updated=?3 WHERE id=?1",params![id,e,db::now()]);}}}
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
}}),stop)}
fn explicit_retry(c:&Connection)->R<Option<String>>{sql(c.query_row("SELECT id FROM jobs WHERE kind='match' AND state IN ('queued','running','pausing','cancelling') AND json_extract(spec,'$.issue_id') IS NOT NULL ORDER BY rowid LIMIT 1",[],|r|r.get(0)).optional())}
struct Snapshot{revision:i64,fingerprint:String,input:matching::Input}
fn snapshot(c:&Connection,rid:&str)->R<Option<Snapshot>>{
    let (title,path,revision,primary):(String,String,i64,Option<String>)=sql(c.query_row("SELECT title,relative_path,revision,work_id FROM resources WHERE id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))))?;
    if primary.is_some()||!editions::bindings(c,rid)?.is_empty(){return Ok(None);}
    let mut hash=Sha256::new();hash.update(path.as_bytes());hash.update(revision.to_le_bytes());let mut hints=vec![path];
    let mut s=sql(c.prepare("SELECT f.id,f.relative_path,f.size,f.mtime FROM files f JOIN resource_files rf ON rf.file_id=f.id WHERE rf.resource_id=?1 ORDER BY length(f.relative_path) DESC,f.relative_path,f.id"))?;
    let mut rows=sql(s.query([rid]))?;let mut n=0;
    while let Some(r)=sql(rows.next())?{let id:String=sql(r.get(0))?;let path:String=sql(r.get(1))?;let size:i64=sql(r.get(2))?;let mtime:String=sql(r.get(3))?;hash.update(json!([id,path,size,mtime]).to_string().as_bytes());if n<31{hints.push(path);}n+=1;}
    if n==0{return Ok(None);}Ok(Some(Snapshot{revision,fingerprint:hex::encode(hash.finalize()),input:matching::prepare(&title,&hints)?}))
}
/// Strong evidence is necessary; conflicting meaningful source names prevent admission.
fn eligible(response:&Value)->Option<&Value>{
    if review_reason(response).is_some(){None}else{response["results"].as_array()?.first()}
}
pub fn review_reason(response:&Value)->Option<String>{
    if response["incomplete"]==true{return Some("Search incomplete. Refresh when VNDB is available.".into());}
    let Some(candidate)=response["results"].as_array().and_then(|v|v.first())else{return Some(if response["needs_title"]==true{"No usable work title in this resource."}else{"No VNDB candidate found."}.into());};
    if candidate["match"]["ambiguous"]==true{return Some("Multiple works have equally strong evidence.".into());}
    if candidate["match"]["strength"]!="strong" {return Some(match candidate["match"]["reason"].as_str(){Some("different_subtitle")=>"Only a partial title matches; the subtitle or edition needs confirmation.",Some("number_conflict")=>"The title and candidate have different sequel numbers.",_=>"The name is too short or the candidate evidence is not exact."}.into());}
    let Some(titles)=response["titles"].as_array()else{return Some("Source title evidence is missing.".into());};
    for title in titles.iter().filter_map(Value::as_str){
        let Ok(input)=matching::prepare(title,&[])else{return Some("Source title cannot be interpreted.".into());};let row=matching::rank(&input,vec![candidate.clone()]);let reason=row[0]["match"]["reason"].as_str().unwrap_or("");
        if !["exact_brand_title","exact_title","exact_alias","exact_release","direct_id","confirmed_title"].contains(&reason)&&!matching::corroborates_filename(title,candidate){return Some(format!("A different source name needs review: {title}"));}
    }None
}
fn brand(path:&str)->String{
    use unicode_normalization::UnicodeNormalization;
    let s:String=path.nfkc().collect();let prefix=regex::Regex::new(r"^(?:\([^)]*\)\s*)?((?:\[[^\]]+\]\s*)+)").unwrap();
    let Some(c)=prefix.captures(&s)else{return String::new();};let labels=regex::Regex::new(r"\[([^\]]+)\]").unwrap();
    labels.captures_iter(&c[1]).map(|v|v[1].to_string()).filter(|v|!v.chars().all(|c|c.is_ascii_digit())&&!v.to_uppercase().starts_with("VJ")&&!v.to_uppercase().starts_with("RJ")).last().map(|v|matching::normalized(&v)).unwrap_or_default()
}
/// Reuse an explicit manual identity only within the same source, brand and full
/// cleaned label. Never learn recursively from automatic matches.
fn with_confirmation(c:&Connection,rid:&str,input:&matching::Input,response:&Value)->R<Value>{
    let mut result=response.clone();let mut candidates=result["results"].as_array().cloned().unwrap_or_default();
    for row in &mut candidates{row.as_object_mut().map(|o|o.remove("confirmed_titles"));}
    let (root,path):(String,String)=sql(c.query_row("SELECT root_id,relative_path FROM resources WHERE id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?))))?;
    let label=matching::clean_name(&path);let key=matching::normalized(&label);let origin=brand(&path);
    if !origin.is_empty()&&key.chars().count()>=4&&input.titles.iter().any(|t|matching::normalized(t)==key){
        let mut s=sql(c.prepare("SELECT r.id,r.relative_path,r.revision,w.vndb_id FROM resources r JOIN works w ON w.id=r.work_id WHERE r.root_id=?1 AND r.id!=?2 AND w.merged_into IS NULL AND w.vndb_id IS NOT NULL AND (SELECT count(*) FROM resource_bindings b WHERE b.resource_id=r.id)=1 AND NOT EXISTS(SELECT 1 FROM match_items m WHERE m.resource_id=r.id AND m.state='matched')"))?;
        let rows=sql(sql(s.query_map(params![root,rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;
        let proofs:Vec<_>=rows.into_iter().filter(|(_,p,_,_)|brand(p)==origin&&matching::normalized(&matching::clean_name(p))==key).collect();
        let identities:std::collections::HashSet<_>=proofs.iter().map(|(_,_,_,id)|id).collect();
        if identities.len()==1 {for candidate in &mut candidates{let proof:Vec<_>=proofs.iter().filter(|(_,_,_,id)|candidate["id"]==id.as_str()).map(|(id,_,rev,_)|json!({"title":label,"resource_id":id,"revision":rev,"basis":"manual_binding_same_source_and_brand"})).collect();if !proof.is_empty(){candidate["confirmed_titles"]=json!(proof);}}}
    }
    result["algorithm_version"]=json!(matching::ALGORITHM_VERSION);result["results"]=json!(matching::rank(input,candidates));result["titles"]=json!(input.titles);result["decision"]=json!({"reason":review_reason(&result)});Ok(result)
}
pub fn explain(c:&Connection,rid:&str,response:&Value)->R<Value>{let snap=snapshot(c,rid)?.ok_or("Resource already matched")?;with_confirmation(c,rid,&snap.input,response)}

#[cfg(test)] mod tests {
    use super::*;
    fn fixture(names:&[&str])->(tempfile::TempDir,Db){
        let t=tempfile::tempdir().unwrap();let db=db::open(&t.path().join("library.sqlite")).unwrap();
        {let c=db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('root',?1,'Fixture')",[t.path().to_string_lossy()]).unwrap();
        for (i,name) in names.iter().enumerate(){let id=format!("r{i}");let path=t.path().join(format!("{i}.rar"));std::fs::write(&path,b"original fixture").unwrap();
            c.execute("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES(?1,'root',?2,?3,'archive')",params![id,format!("{i}/{name}.rar"),name]).unwrap();
            c.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES(?1,'root',?2,?3,16,'1','rar','fixture')",params![id,path.to_string_lossy(),format!("{i}/{name}.rar")]).unwrap();
            c.execute("INSERT INTO resource_files(resource_id,file_id) VALUES(?1,?1)",[&id]).unwrap();
        }}(t,db)
    }
    fn response(input:matching::Input,rows:Vec<Value>)->Value {json!({"results":matching::rank(&input,rows),"titles":input.titles,"incomplete":false})}
    fn exact(input:matching::Input)->Value{response(input,vec![json!({"id":"v4","title":"Clannad"})])}
    fn scalar(db:&Db,query:&str)->i64{db.lock().unwrap().query_row(query,[],|r|r.get(0)).unwrap()}
    #[tokio::test]async fn scoped_issue_retry_runs_without_enabling_global_matching(){
        let(_t,db)=fixture(&["Clannad","Kanon"]);let job={let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();crate::issues::reconcile(&tx).unwrap();let id:i64=tx.query_row("SELECT seq FROM issues WHERE resource_id='r0'",[],|r|r.get(0)).unwrap();let job=crate::issues::queue_retry(&tx,id,1).unwrap();assert_eq!(crate::issues::queue_retry(&tx,id,1).unwrap(),job);assert!(!enabled(&tx).unwrap());assert_eq!(explicit_retry(&tx).unwrap(),Some(job.clone()));tx.commit().unwrap();job};
        assert_eq!(scalar(&db,"SELECT count(*) FROM match_items"),1);run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();
        let c=db.lock().unwrap();assert!(!enabled(&c).unwrap());assert_eq!(explicit_retry(&c).unwrap(),None);assert_eq!(c.query_row("SELECT work_id IS NULL FROM resources WHERE id='r1'",[],|r|r.get::<_,bool>(0)).unwrap(),true);
        crate::issues::reconcile(&c).unwrap();assert_eq!(c.query_row("SELECT state FROM issues WHERE resource_id='r0'",[],|r|r.get::<_,String>(0)).unwrap(),"resolved");
    }
    #[tokio::test]async fn repeated_attempts_survive_success_backup_and_restore(){
        let(t,db)=fixture(&["Clannad"]);let job=kick(&db,true).unwrap().unwrap();
        for message in ["First failure","Second failure"]{db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run_with(&db,&job,|_|async{Err(message.into())}).await.unwrap();}
        db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();
        let before={let c=db.lock().unwrap();let p=crate::match_history::attempts(&c,"r0",None).unwrap();assert_eq!(p["items"].as_array().unwrap().len(),3);assert_eq!(p["items"][0]["state"],"matched");assert_eq!(p["items"][1]["reason"],"Second failure");assert_eq!(p["items"][2]["reason"],"First failure");let older=crate::match_history::attempts(&c,"r0",p["items"][1]["seq"].as_i64()).unwrap();assert_eq!(older["items"].as_array().unwrap().len(),1);p};
        let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();assert_eq!(crate::match_history::attempts(&restored.lock().unwrap(),"r0",None).unwrap(),before);
    }
    #[tokio::test]async fn failed_attempt_record_rolls_back_binding(){
        let(_t,db)=fixture(&["Clannad"]);let job=kick(&db,true).unwrap().unwrap();db.lock().unwrap().execute_batch("CREATE TRIGGER fixture_reject_attempt BEFORE INSERT ON events WHEN NEW.code='match.attempt' BEGIN SELECT RAISE(ABORT,'Generated journal failure'); END;").unwrap();
        assert!(run_with(&db,&job,|i|async{Ok(exact(i))}).await.is_err());assert_eq!(scalar(&db,"SELECT count(*) FROM works"),0);assert_eq!(scalar(&db,"SELECT count(*) FROM match_items WHERE state='pending'"),1);assert_eq!(scalar(&db,"SELECT count(*) FROM resource_bindings"),0);
    }
    #[tokio::test]async fn history_records_source_and_provider_failure_without_binding(){
        let(_t,db)=fixture(&["Clannad"]);let job=kick(&db,true).unwrap().unwrap();
        run_with(&db,&job,|_|async{Err("Generated provider unavailable".into())}).await.unwrap();
        {let c=db.lock().unwrap();let page=crate::match_history::page(&c,"r0",None).unwrap();let item=&page["items"][0];assert_eq!(item["reason"],"Generated provider unavailable");assert_eq!(item["job_state"],"paused");assert_eq!(item["titles"][0],"Clannad");assert!(item["source"]["listing_fingerprint"].is_string());assert!(item["recorded_at"].is_number());assert!(item["work_id"].is_null());}
        let retry=kick(&db,true).unwrap().unwrap();run_with(&db,&retry,|i|async{Ok(exact(i))}).await.unwrap();
        let c=db.lock().unwrap();let page=crate::match_history::page(&c,"r0",None).unwrap();assert_eq!(page["items"].as_array().unwrap().len(),2);assert_eq!(page["items"][0]["state"],"matched");assert_eq!(page["items"][1]["reason"],"Generated provider unavailable");
    }
    fn confirmed_fixture()->(tempfile::TempDir,Db){
        let(t,db)=fixture(&["Short Name","Short Name","Short Name"]);
        db.lock().unwrap().execute_batch("UPDATE resources SET relative_path=CASE id WHEN 'r0' THEN '[240101][Studio] Short Name DL版' WHEN 'r1' THEN '[240101][Studio] Short Name 通常版' ELSE '[240101][Studio] Short Name 豪華版' END;INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Short Name ~Long Subtitle~','Original','v4'),('other','Short Name ~Other Story~','Other','v5');").unwrap();
        editions::bind_primary(&db,"r1","w","Manual edition",1).unwrap();(t,db)
    }
    fn short_response()->Value{let input=matching::prepare("Short Name",&[]).unwrap();response(input,vec![json!({"id":"v4","title":"Short Name ~Long Subtitle~"})])}
    #[test] fn manual_same_source_brand_and_label_can_confirm_an_abbreviated_title(){
        let(_t,db)=confirmed_fixture();let c=db.lock().unwrap();let result=explain(&c,"r0",&short_response()).unwrap();
        assert_eq!(result["results"][0]["match"]["reason"],"confirmed_title");assert!(eligible(&result).is_some());
        c.execute("UPDATE resources SET relative_path='[240101][Another Studio] Short Name DL版' WHERE id='r0'",[]).unwrap();
        assert!(eligible(&explain(&c,"r0",&short_response()).unwrap()).is_none());
    }
    #[test] fn conflicting_manual_bindings_and_automatic_precedents_are_not_learned(){
        let(_t,db)=confirmed_fixture();editions::bind_primary(&db,"r2","other","Manual edition",1).unwrap();
        assert!(eligible(&explain(&db.lock().unwrap(),"r0",&short_response()).unwrap()).is_none());
        editions::set_bindings(&db,"r2",editions::EditBindings{revision:2,primary_work_id:None,bindings:vec![]}).unwrap();
        let job=kick(&db,true).unwrap().unwrap();let c=db.lock().unwrap();
        c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,work_id) VALUES(?1,'r1',1,'matched','w')",[job]).unwrap();
        assert!(eligible(&explain(&c,"r0",&short_response()).unwrap()).is_none());
    }
    #[test] fn manual_confirmation_is_rechecked_atomically_before_admission(){
        let(_t,db)=confirmed_fixture();let job=kick(&db,true).unwrap().unwrap();db.lock().unwrap().execute("UPDATE jobs SET state='running'",[]).unwrap();
        let snap=snapshot(&db.lock().unwrap(),"r0").unwrap().unwrap();let enriched=explain(&db.lock().unwrap(),"r0",&short_response()).unwrap();assert!(eligible(&enriched).is_some());
        editions::set_bindings(&db,"r1",editions::EditBindings{revision:2,primary_work_id:None,bindings:vec![]}).unwrap();
        apply(&db,&job,"r0",&snap,&enriched).unwrap();assert!(editions::bindings(&db.lock().unwrap(),"r0").unwrap().is_empty());
        assert_eq!(scalar(&db,"SELECT count(*) FROM match_items WHERE state='review'"),1);
    }
    #[tokio::test] async fn binds_unique_results_once_reuses_work_and_preserves_originals(){
        let(t,db)=fixture(&["Clannad","Clannad","Unknown title"]);let job=kick(&db,false).unwrap().unwrap();assert_eq!(kick(&db,true).unwrap(),Some(job.clone()));
        run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();
        assert_eq!(scalar(&db,"SELECT count(*) FROM works"),1);assert_eq!(scalar(&db,"SELECT count(*) FROM resource_bindings"),2);
        let status=status(&db).unwrap();assert_eq!(status["job"]["matched"],2);assert_eq!(status["job"]["review"],1);assert_eq!(status["job"]["state"],"completed");let summary=crate::job_summary::read(&db.lock().unwrap(),&job).unwrap();assert_eq!(summary["new_works"],1);assert_eq!(summary["new_editions"],0);assert_eq!(summary["needs_review"],1);assert_eq!(summary["unmatched_resources"],1);
        assert!(kick(&db,false).unwrap().is_none());let second=kick(&db,true).unwrap().unwrap();run_with(&db,&second,|i|async{Ok(exact(i))}).await.unwrap();
        assert_eq!(scalar(&db,"SELECT count(*) FROM resource_history"),2);assert_eq!(scalar(&db,"SELECT count(*) FROM plans"),0);assert_eq!(scalar(&db,"SELECT count(*) FROM operations"),0);
        for i in 0..3{assert_eq!(std::fs::read(t.path().join(format!("{i}.rar"))).unwrap(),b"original fixture");}
    }
    #[test] fn uncertainty_partial_results_and_conflicting_member_titles_stay_unmatched(){
        let input=matching::prepare("Clannad",&[]).unwrap();let mut result=exact(input.clone());assert!(eligible(&result).is_some());result["incomplete"]=json!(true);assert!(eligible(&result).is_none());
        let tied=response(input,vec![json!({"id":"v4","title":"Clannad"}),json!({"id":"v5","title":"Clannad"})]);assert!(eligible(&tied).is_none());
        for other in ["Clannad 2","Clannad -After Story-","Other game"] {let input=matching::prepare("Clannad",&[other.into()]).unwrap();assert!(eligible(&exact(input)).is_none(),"{other}");}
        let input=matching::prepare("AIR",&[]).unwrap();assert!(eligible(&response(input,vec![json!({"id":"v36","title":"AIR"})])).is_none());
    }
    #[tokio::test] async fn manual_metadata_and_binding_changes_win_over_inflight_search(){
        let(_t,db)=fixture(&["Clannad","Clannad"]);db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title,vndb_id,notes,overrides) VALUES('w','My title','Original','v4','Private notes','{\"title\":\"My title\"}');").unwrap();
        let job=kick(&db,false).unwrap().unwrap();let writes=db.clone();
        run_with(&db,&job,move|i|{let writes=writes.clone();async move{if scalar(&writes,"SELECT count(*) FROM resource_bindings")==0{editions::bind_primary(&writes,"r0","w","Manual edition",1).unwrap();}Ok(exact(i))}}).await.unwrap();
        let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT title,notes FROM works WHERE id='w'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).unwrap(),("My title".into(),"Private notes".into()));
        assert_eq!(c.query_row("SELECT release_label FROM resources WHERE id='r0'",[],|r|r.get::<_,String>(0)).unwrap(),"Manual edition");drop(c);
        assert_eq!(status(&db).unwrap()["job"]["skipped"],1);assert_eq!(status(&db).unwrap()["job"]["matched"],1);
    }
    #[tokio::test] async fn changed_membership_or_revision_is_never_admitted(){
        for update in ["UPDATE files SET mtime='2'","DELETE FROM resource_files","UPDATE resources SET revision=revision+1"] {
            let(_t,db)=fixture(&["Clannad"]);let job=kick(&db,false).unwrap().unwrap();let writes=db.clone();
            run_with(&db,&job,move|i|{writes.lock().unwrap().execute_batch(update).unwrap();async move{Ok(exact(i))}}).await.unwrap();
            assert_eq!(scalar(&db,"SELECT count(*) FROM works"),0);assert_eq!(status(&db).unwrap()["job"]["skipped"],1);
        }
    }
    #[tokio::test] async fn pause_cancel_and_provider_failures_keep_pending_items_for_resume(){
        for state in ["pausing","cancelling","provider_error"] {
            let(_t,db)=fixture(&["Clannad"]);let job=kick(&db,false).unwrap().unwrap();let writes=db.clone();
            run_with(&db,&job,move|i|{if state!="provider_error"{writes.lock().unwrap().execute("UPDATE jobs SET state=?1",[state]).unwrap();}async move{if state=="provider_error"{Err("VNDB rate limit".into())}else{Ok(exact(i))}}}).await.unwrap();
            assert_eq!(scalar(&db,"SELECT count(*) FROM works"),0);assert_eq!(scalar(&db,"SELECT count(*) FROM match_items WHERE state='pending'"),1);
            assert_eq!(status(&db).unwrap()["job"]["state"],if state=="cancelling"{"cancelled"}else{"paused"});
            db.lock().unwrap().execute("UPDATE jobs SET state='queued'",[]).unwrap();run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();assert_eq!(scalar(&db,"SELECT count(*) FROM resource_bindings"),1);let summary=crate::job_summary::read(&db.lock().unwrap(),&job).unwrap();assert_eq!(summary["new_works"],1);assert_eq!(summary["unmatched_resources"],0);assert_eq!(summary["failed"],if state=="provider_error"{1}else{0});
        }
    }
    #[tokio::test] async fn interrupted_sequence_resumes_without_replaying_committed_matches(){
        let(t,db)=fixture(&["Clannad","Clannad"]);let job=kick(&db,false).unwrap().unwrap();
        db.lock().unwrap().execute("UPDATE jobs SET state='running'",[]).unwrap();let snap=snapshot(&db.lock().unwrap(),"r0").unwrap().unwrap();apply(&db,&job,"r0",&snap,&exact(snap.input.clone())).unwrap();drop(db);
        let db=db::open(&t.path().join("library.sqlite")).unwrap();assert_eq!(status(&db).unwrap()["job"]["state"],"interrupted");assert!(kick(&db,false).unwrap().is_none());
        // Backup restore clears settings, but bootstrap must not replace its interrupted queue.
        db.lock().unwrap().execute("DELETE FROM settings WHERE key LIKE 'auto_match_%'",[]).unwrap();assert!(kick(&db,false).unwrap().is_none());assert_eq!(scalar(&db,"SELECT count(*) FROM jobs"),1);
        db.lock().unwrap().execute("UPDATE jobs SET state='queued'",[]).unwrap();run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();assert_eq!(scalar(&db,"SELECT count(*) FROM resource_history"),2);
    }
    #[tokio::test] async fn algorithm_upgrade_retries_old_reviews_once_without_touching_matches(){
        let(_t,db)=fixture(&["Clannad","Unknown title"]);let first=kick(&db,false).unwrap().unwrap();
        run_with(&db,&first,|i|async{Ok(exact(i))}).await.unwrap();
        {let c=db.lock().unwrap();c.execute("UPDATE match_items SET evidence=json_set(evidence,'$.algorithm_version',1) WHERE state='review'",[]).unwrap();set(&c,"auto_match_version","1").unwrap();}
        let upgraded=kick(&db,false).unwrap().unwrap();assert_ne!(upgraded,first);assert_eq!(status(&db).unwrap()["job"]["total"],1);
        run_with(&db,&upgraded,|i|async{Ok(exact(i))}).await.unwrap();assert!(kick(&db,false).unwrap().is_none());
        assert_eq!(scalar(&db,"SELECT count(*) FROM resource_history"),1);assert_eq!(scalar(&db,"SELECT count(*) FROM jobs"),2);
    }
    #[test] fn refresh_defers_during_scans_and_replaces_paused_runs(){
        let(_t,db)=fixture(&["Clannad"]);db.lock().unwrap().execute_batch("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('scan','scan','running','{}',1,1)").unwrap();
        assert!(kick(&db,false).unwrap().is_none());assert_eq!(status(&db).unwrap()["waiting"],true);assert_eq!(scalar(&db,"SELECT count(*) FROM match_items"),0);
        db.lock().unwrap().execute("UPDATE jobs SET state='completed' WHERE id='scan'",[]).unwrap();let first=kick(&db,true).unwrap().unwrap();assert_eq!(status(&db).unwrap()["waiting"],false);
        db.lock().unwrap().execute("UPDATE jobs SET state='paused' WHERE id=?1",[&first]).unwrap();let second=kick(&db,true).unwrap().unwrap();assert_ne!(first,second);assert_eq!(scalar(&db,"SELECT count(*) FROM jobs WHERE state='cancelled'"),1);
    }
}
fn complete_item(c:&Connection,job:&str,rid:&str,state:&str,response:&Value,work:Option<&str>)->R<()>{
    sql(crate::job_summary::resource(c,job,rid))?;
    for (dimension,active) in [("needs_review",state=="review"),("skipped",state=="skipped")]{sql(crate::job_summary::record(c,job,dimension,rid,if active{"yes"}else{"no"}))?;}
    let mut response=response.clone();response["recorded_at"]=json!(db::now());
    sql(c.execute("UPDATE match_items SET state=?3,evidence=?4,work_id=?5 WHERE job_id=?1 AND resource_id=?2",params![job,rid,state,response.to_string(),work]))?;
    let revision:i64=sql(c.query_row("SELECT revision FROM match_items WHERE job_id=?1 AND resource_id=?2",params![job,rid],|r|r.get(0)))?;
    sql(db::event(c,job,"match.attempt",&json!({"resource_id":rid,"revision":revision,"state":state,"evidence":response,"work_id":work})))?;
    sql(c.execute("UPDATE jobs SET processed=(SELECT count(*) FROM match_items WHERE job_id=?1 AND state IN ('matched','review','skipped')),updated=?2 WHERE id=?1",params![job,db::now()]))?;Ok(())
}
fn apply(db:&Db,job:&str,rid:&str,expected:&Snapshot,response:&Value)->R<()> {
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;
    let state:String=sql(tx.query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get(0)))?;if state!="running"{return Ok(());}
    let pending:bool=sql(tx.query_row("SELECT state='pending' FROM match_items WHERE job_id=?1 AND resource_id=?2",params![job,rid],|r|r.get(0)))?;if !pending{return Ok(());}
    let current=snapshot(&tx,rid)?;
    if !current.is_some_and(|s|s.revision==expected.revision&&s.fingerprint==expected.fingerprint){complete_item(&tx,job,rid,"skipped",&json!({"reason":"Resource changed while matching"}),None)?;sql(tx.commit())?;return Ok(());}
    let mut result=with_confirmation(&tx,rid,&expected.input,response)?;
    result["source"]=json!({"revision":expected.revision,"listing_fingerprint":expected.fingerprint,"queries":expected.input.queries});let response=&result;
    if let Some(candidate)=eligible(response){
        let external=candidate["id"].as_str().ok_or("Candidate has no VNDB ID")?;
        let existing:Option<String>=sql(tx.query_row("SELECT id FROM works WHERE vndb_id=?1",[external],|r|r.get(0)).optional())?;
        let mut wid=existing.clone().unwrap_or_else(db::id);
        if existing.is_some(){let mut seen=std::collections::HashSet::new();loop{if !seen.insert(wid.clone()){return Err("Work merge history contains a cycle".into());}let merged:Option<String>=sql(tx.query_row("SELECT merged_into FROM works WHERE id=?1",[&wid],|r|r.get(0)))?;if let Some(next)=merged{wid=next;}else{break;}}}
        else{
            let title=candidate["title"].as_str().filter(|s|!s.is_empty()).ok_or("Candidate has no title")?;
            let source=json!({"title":title,"original_title":candidate["alttitle"].as_str().unwrap_or(title),"description":candidate["description"].as_str().unwrap_or(""),"cover":candidate["image"]["url"].as_str().unwrap_or(""),"developer":candidate["developers"].as_array().map(|a|a.iter().filter_map(|d|d["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),"released":candidate["released"].as_str().unwrap_or(""),"tags":candidate["tags"].as_array().map(|a|a.iter().filter_map(|d|d["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),"aliases":candidate["aliases"].as_array().cloned().unwrap_or_default()});
            sql(tx.execute("INSERT INTO works(id,title,original_title,vndb_id,description,cover,developer,released,tags,aliases,source_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![wid,source["title"].as_str(),source["original_title"].as_str(),external,source["description"].as_str(),source["cover"].as_str(),source["developer"].as_str(),source["released"].as_str(),source["tags"].to_string(),source["aliases"].to_string(),source.to_string()]))?;
        }
        if existing.is_none(){sql(crate::job_summary::record(&tx,job,"new_works",&wid,"yes"))?;}
        let release=editions::legacy_release(&tx,&wid,"Unclassified edition")?;
        editions::set_bindings_in(&tx,rid,editions::EditBindings{revision:expected.revision,primary_work_id:Some(wid.clone()),bindings:vec![editions::Binding{work_id:wid.clone(),release_id:release,role:"unknown".into()}]})?;
        complete_item(&tx,job,rid,"matched",response,Some(&wid))?;
    }else{complete_item(&tx,job,rid,"review",response,None)?;}
    sql(tx.commit())?;Ok(())
}
async fn run(db:&Db,job:&str)->R<()>{run_with(db,job,matching::search).await}
async fn run_with<F,Fut>(db:&Db,job:&str,search:F)->R<()> where F:Fn(matching::Input)->Fut,Fut:std::future::Future<Output=R<Value>> {
    {let c=db.lock().map_err(|e|e.to_string())?;sql(c.execute("UPDATE jobs SET state='running',message='Matching resources',updated=?2 WHERE id=?1 AND state='queued'",params![job,db::now()]))?;}
    loop{
        let next={let c=db.lock().map_err(|e|e.to_string())?;let state:String=sql(c.query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get(0)))?;
            if ["pausing","cancelling"].contains(&state.as_str()){sql(c.execute("UPDATE jobs SET state=?2,current_path='',updated=?3 WHERE id=?1",params![job,if state=="pausing"{"paused"}else{"cancelled"},db::now()]))?;return Ok(());}
            if state!="running"{return Ok(());}
            let next:Option<(String,i64)>=sql(c.query_row("SELECT resource_id,revision FROM match_items WHERE job_id=?1 AND state='pending' ORDER BY rowid LIMIT 1",[job],|r|Ok((r.get(0)?,r.get(1)?))).optional())?;
            if next.is_none(){
                sql(c.execute("UPDATE jobs SET state='completed',current_path='',message='Matching complete. Uncertain resources remain Unmatched.',updated=?2 WHERE id=?1",params![job,db::now()]))?;
                let counts:(i64,i64)=sql(c.query_row("SELECT coalesce(sum(state='matched'),0),coalesce(sum(state='review'),0) FROM match_items WHERE job_id=?1",[job],|r|Ok((r.get(0)?,r.get(1)?))))?;
                sql(db::event(&c,job,"match.completed",&json!({"matched":counts.0,"review":counts.1})))?;return Ok(());
            }next.unwrap()
        };
        let (rid,revision)=next;
        let snap={let mut connection=db.lock().map_err(|e|e.to_string())?;let c=sql(connection.transaction())?;let snap=snapshot(&c,&rid)?;if snap.as_ref().is_none_or(|s|s.revision!=revision){complete_item(&c,job,&rid,"skipped",&json!({"reason":"Resource already changed or matched"}),None)?;sql(c.commit())?;None}else{sql(c.execute("UPDATE jobs SET current_path=(SELECT title FROM resources WHERE id=?2),updated=?3 WHERE id=?1",params![job,rid,db::now()]))?;sql(c.commit())?;snap}};
        let Some(snap)=snap else{continue;};
        match search(snap.input.clone()).await {
            Ok(result) if !result["warning"].is_string()=>apply(db,job,&rid,&snap,&result)?,
            result=>{let message=match result{Err(e)=>e,Ok(v)=>v["warning"].as_str().unwrap_or("VNDB search incomplete. Refine the name or refresh later.").to_string()};
                let mut connection=db.lock().map_err(|e|e.to_string())?;let c=sql(connection.transaction())?;
                crate::diagnostics::record("error","matching.provider",&message,Some(job));
                sql(crate::job_summary::resource(&c,job,&rid))?;
                sql(crate::job_summary::record(&c,job,"failed",&rid,"yes"))?;
                let evidence=json!({"error":message,"recorded_at":db::now(),"algorithm_version":matching::ALGORITHM_VERSION,"titles":snap.input.titles,"source":{"revision":snap.revision,"listing_fingerprint":snap.fingerprint,"queries":snap.input.queries}});
                sql(db::event(&c,job,"match.attempt",&json!({"resource_id":rid,"revision":snap.revision,"state":"failed","evidence":evidence,"work_id":null})))?;
                sql(c.execute("UPDATE match_items SET evidence=?3 WHERE job_id=?1 AND resource_id=?2 AND state='pending'",params![job,rid,evidence.to_string()]))?;
                sql(c.execute("UPDATE jobs SET state=CASE WHEN state='cancelling' THEN 'cancelled' ELSE 'paused' END,errors=errors+1,message=?2,current_path='',updated=?3 WHERE id=?1",params![job,message,db::now()]))?;sql(c.commit())?;return Ok(());
            }
        }
    }
}


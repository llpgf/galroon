//! Durable catalog-only automatic matching. No filesystem mutations.
use crate::{db::{self,Db},editions,matching};
use rusqlite::{Connection,OptionalExtension,params};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use unicode_normalization::UnicodeNormalization;
type R<T>=Result<T,String>;
fn sql<T>(v:rusqlite::Result<T>)->R<T>{v.map_err(|e|e.to_string())}
pub fn migrate(c:&Connection)->R<()> {
    sql(c.execute_batch("CREATE TABLE IF NOT EXISTS match_items(job_id TEXT NOT NULL REFERENCES jobs(id),resource_id TEXT NOT NULL REFERENCES resources(id),revision INTEGER NOT NULL,state TEXT NOT NULL DEFAULT 'pending',evidence TEXT NOT NULL DEFAULT '{}',work_id TEXT,PRIMARY KEY(job_id,resource_id)); CREATE INDEX IF NOT EXISTS match_pending ON match_items(job_id,state); CREATE INDEX IF NOT EXISTS match_resource ON match_items(resource_id,revision,state); CREATE INDEX IF NOT EXISTS intake_events ON events(code,job_id,seq); CREATE INDEX IF NOT EXISTS match_attempt_resource ON events(json_extract(payload,'$.resource_id'),seq) WHERE code='match.attempt';"))
}
fn scan_key(c:&Connection)->R<String>{Ok(sql(c.query_row("SELECT id||':'||updated FROM jobs WHERE kind='scan' AND state='completed' ORDER BY updated DESC,rowid DESC LIMIT 1",[],|r|r.get(0)).optional())?.unwrap_or_default())}
fn set(c:&Connection,key:&str,value:&str)->R<()>{sql(c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]))?;Ok(())}
/// On unless explicitly turned off, so a fresh library matches after its first scan.
pub fn enabled(c:&Connection)->R<bool>{Ok(sql(c.query_row("SELECT value FROM settings WHERE key='auto_match_enabled'",[],|r|r.get::<_,String>(0)).optional())?.as_deref()!=Some("0"))}
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
// A name that cleans to nothing (only a date and brand) is left for review rather than failing the whole job.
fn snapshot(c:&Connection,rid:&str)->R<Option<Snapshot>>{
    catalog_snapshot(c,rid,false)
}
fn member_hints(paths:Vec<String>)->Vec<String>{
    let mut paths:Vec<String>=paths.into_iter().map(|p|p.nfc().collect()).collect();
    paths.sort_by_cached_key(|p|(std::cmp::Reverse(p.chars().count()),p.clone()));paths.truncate(31);paths
}
fn catalog_snapshot(c:&Connection,rid:&str,include_bound:bool)->R<Option<Snapshot>>{
    let (title,path,revision,primary):(String,String,i64,Option<String>)=sql(c.query_row("SELECT title,relative_path,revision,work_id FROM resources WHERE id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))))?;
    if !include_bound&&(primary.is_some()||!editions::bindings(c,rid)?.is_empty()){return Ok(None);}
    // Catalogs scanned before system entries were skipped can still hold macOS metadata such as ._DAV.
    if std::path::Path::new(&path).components().any(|p|crate::scan::system_entry(&p.as_os_str().to_string_lossy())){return Ok(None);}
    let mut hash=Sha256::new();hash.update(path.as_bytes());hash.update(revision.to_le_bytes());
    let path:String=path.nfc().collect();let title:String=title.nfc().collect();
    // A bundle member (scan split "Bundle/child") is named by the child, never by the collection folder.
    let parent=std::path::Path::new(&path).parent().map(|p|p.to_string_lossy().to_string()).filter(|p|!p.is_empty());
    let local=|p:&str|parent.as_deref().and_then(|top|std::path::Path::new(p).strip_prefix(top).ok()).map(|p|p.to_string_lossy().to_string()).unwrap_or_else(||p.to_string());
    let mut hints=vec![local(&path)];
    let mut s=sql(c.prepare("SELECT f.id,f.relative_path,f.size,f.mtime FROM files f JOIN resource_files rf ON rf.file_id=f.id WHERE rf.resource_id=?1 ORDER BY length(f.relative_path) DESC,f.relative_path,f.id"))?;
    // Preserve the raw SQL order and values for existing listing fingerprints. Hint selection is NFC-only.
    let mut rows=sql(s.query([rid]))?;let mut members=Vec::new();
    while let Some(r)=sql(rows.next())?{let id:String=sql(r.get(0))?;let path:String=sql(r.get(1))?;let size:i64=sql(r.get(2))?;let mtime:String=sql(r.get(3))?;hash.update(json!([id,path,size,mtime]).to_string().as_bytes());members.push(path);}
    if members.is_empty(){return Ok(None);}hints.extend(member_hints(members).iter().map(|p|local(p)));
    Ok(Some(Snapshot{revision,fingerprint:hex::encode(hash.finalize()),input:if title.trim().is_empty(){matching::Input{titles:vec![],queries:vec![],direct_id:None,brands:vec![],dates:vec![]}}else{matching::prepare(&local(&title),&hints)?}}))
}
/// Strong evidence is necessary; conflicting meaningful source names prevent admission.
fn eligible(response:&Value)->Option<&Value>{
    if review_reason(response).is_some(){None}else{response["results"].as_array()?.first()}
}
/// The complete proposed binding, in primary-first order; admission still requires review_reason.
pub fn bound_candidates(response:&Value)->Vec<&Value>{
    let Some(rows)=response["results"].as_array()else{return vec![];};let Some(first)=rows.first()else{return vec![];};
    if let Some(ids)=first["match"]["compilation"]["vndb_ids"].as_array(){ids.iter().filter_map(|id|rows.iter().find(|c|c["id"]==*id)).collect()}else{vec![first]}
}
fn strings(v:&Value)->Vec<String>{v.as_array().map(|a|a.iter().filter_map(|s|s.as_str().map(str::to_owned)).collect()).unwrap_or_default()}
pub fn review_reason(response:&Value)->Option<String>{
    // Incomplete without a provider warning means VNDB had more results than one page: retrying will not help.
    if response["incomplete"]==true{return Some(if response["warning"].is_string(){"Search incomplete. Refresh when VNDB is available."}else{"VNDB has too many works with this name to be sure. Choose the work manually."}.into());}
    // No recorded search yet (queued, or matching paused before reaching it): never claim VNDB has no candidate.
    if response.get("results").is_none(){return Some("Not checked against VNDB yet.".into());}
    let Some(candidate)=response["results"].as_array().and_then(|v|v.first())else{return Some(if response["needs_title"]==true{"No usable work title in this resource."}else{"No VNDB candidate found."}.into());};
    if candidate["match"]["ambiguous"]==true{return Some("Multiple works have equally strong evidence.".into());}
    if candidate["match"]["strength"]!="strong" {return Some(match candidate["match"]["reason"].as_str(){Some("release_conflict")=>"The folder's brand and release date belong to a different work with the same name.",Some("different_subtitle")=>"Only a partial title matches; the subtitle or edition needs confirmation.",Some("number_conflict")=>"The title and candidate have different sequel numbers.",_=>"The name is too short or the candidate evidence is not exact."}.into());}
    let Some(titles)=response["titles"].as_array()else{return Some("Source title evidence is missing.".into());};
    let bound=bound_candidates(response);
    let release_id=&candidate["match"]["compilation"]["release_id"];
    let compilation_names:Vec<_>=candidate["release_matches"].as_array().into_iter().flatten().filter(|r|r["id"]==*release_id).filter_map(|r|r["title"].as_str()).collect();
    let known:Vec<_>=bound.iter().flat_map(|c|matching::candidate_names(c)).collect();let mut names=known.clone();names.extend(titles.first().and_then(Value::as_str));
    let mut fallbacks=vec![None;bound.len()];
    'members: for (index,title) in titles.iter().filter_map(Value::as_str).enumerate(){
        // Bonus, crack and artwork members of the matched work file into its card. The resource's own name only
        // passes as a VNDB name plus such words, never as a code name or bonus words alone.
        if if index==0{matching::names_with_extras(title,&known)}else{matching::ancillary_member(title,&names)}{continue;}
        for (work_index,&candidate) in bound.iter().enumerate(){
        let Ok(mut input)=matching::prepare(title,&[])else{return Some("Source title cannot be interpreted.".into());};
        // Re-ranking one cleaned title keeps the resource's own brand/date corroboration.
        input.brands=strings(&response["brands"]);input.dates=strings(&response["dates"]);let row=matching::rank(&input,vec![candidate.clone()]);let reason=row[0]["match"]["reason"].as_str().unwrap_or("");
        if !["exact_brand_title","exact_title","exact_alias","exact_release","direct_id","confirmed_title","exact_main_title"].contains(&reason)&&!matching::corroborates_filename(title,candidate){
            let (fallback_names,brands,additional_names)=fallbacks[work_index].get_or_insert_with(||{
                let mut brands:Vec<&str>=response["brands"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                for maker in candidate["developers"].as_array().into_iter().flatten(){brands.extend(["name","original"].iter().filter_map(|k|maker[*k].as_str()));}
                let mut additional_names=Vec::new();
                // A release spelling is evidence only when it names this folder, not another
                // bundled release of the same VNDB work.
                for release in candidate["release_matches"].as_array().into_iter().flatten(){
                    for key in ["title","alttitle"]{if let Some(name)=release[key].as_str(){
                        if titles.first().and_then(Value::as_str).is_some_and(|folder|matching::normalized(folder)==matching::normalized(name)){additional_names.push(name);}
                    }}
                }
                let mut fallback_names=names.clone();
                // Quoted bonus labels sometimes omit the subtitle. Reuse the existing corroborated-main-title
                // rule rather than letting an arbitrary shortened name vouch for itself.
                for name in &known{
                    let main=matching::main_title(name);if main==*name||matching::normalized(main).chars().count()<4{continue;}
                    let Ok(mut input)=matching::prepare(main,&[])else{continue;};input.brands=strings(&response["brands"]);input.dates=strings(&response["dates"]);
                    let row=matching::rank(&input,vec![candidate.clone()]);if row[0]["match"]["reason"]=="exact_main_title"&&row[0]["match"]["strength"]=="strong"{fallback_names.push(main);additional_names.push(main);}
                }
                (fallback_names,brands,additional_names)
            });
            if index>0&&response["results"][0]["match"]["compilation"].is_object()&&matching::compilation_member(title,&compilation_names,fallback_names){continue 'members;}
            if matching::title_anchored_member(title,if index==0{&known}else{fallback_names},brands)||matching::title_anchored_member(title,additional_names,brands){continue 'members;}
            if index>0&&matching::corroborated_member(title,candidate,response["results"].as_array().unwrap()){continue 'members;}
            if index>0&&matching::fan_release_member(title,candidate){continue 'members;}
        }else{continue 'members;}
        }
        return Some(format!("A different source name needs review: {title}"));
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
    result["algorithm_version"]=json!(matching::ALGORITHM_VERSION);result["results"]=json!(matching::rank(input,candidates));result["titles"]=json!(input.titles);result["brands"]=json!(input.brands);result["dates"]=json!(input.dates);result["decision"]=json!({"reason":review_reason(&result)});Ok(result)
}
pub fn explain(c:&Connection,rid:&str,response:&Value)->R<Value>{let snap=snapshot(c,rid)?.ok_or("Resource already matched")?;with_confirmation(c,rid,&snap.input,response)}
/// Read-only input for live diagnostics, including bound resources; shares automatch's catalog builder.
pub fn catalog_input(c:&Connection,rid:&str)->R<Option<matching::Input>>{Ok(catalog_snapshot(c,rid,true)?.map(|s|s.input))}
/// Re-evaluate recorded candidates, including bound resources, using catalog evidence only. Never writes.
pub fn replay(c:&Connection,rid:&str,response:&Value)->R<Option<Value>>{
    let Some(snap)=catalog_snapshot(c,rid,true)?else{return Ok(None);};let mut stored=response.clone();
    if !stored.is_object(){return Err("Recorded response must be an object".into());}
    if let Some(rows)=stored.get_mut("results"){
        for row in rows.as_array_mut().ok_or("Recorded results must be an array")?{if let Some(o)=row.as_object_mut(){o.remove("match");}}
    }
    let mut result=with_confirmation(c,rid,&snap.input,&stored)?;
    // An absent search is different from a completed search with no candidates.
    if stored.get("results").is_none(){result.as_object_mut().unwrap().remove("results");result["decision"]=json!({"reason":review_reason(&result)});}
    Ok(Some(result))
}

#[cfg(test)] mod tests {
    use super::*;
    #[tokio::test]async fn completed_auto_match_publishes_catalog_sequence(){
        let(_t,db)=fixture(&["Clannad"]);let job=kick(&db,false).unwrap().unwrap();let before=crate::catalog_changes::seq(&db.lock().unwrap()).unwrap();
        run_with(&db,&job,|i|async{Ok(exact(i))}).await.unwrap();
        assert!(crate::catalog_changes::seq(&db.lock().unwrap()).unwrap()>before);assert_eq!(scalar(&db,"SELECT count(*) FROM resources WHERE work_id IS NOT NULL"),1);
    }
    #[test]fn unsearched_resources_are_not_reported_as_missing_from_vndb(){
        assert_eq!(review_reason(&json!({})).as_deref(),Some("Not checked against VNDB yet."));
        assert_eq!(review_reason(&json!({"results":[]})).as_deref(),Some("No VNDB candidate found."));
    }
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
    fn compilation_response(input:matching::Input)->Value{response(input,vec![json!({"id":"v1","title":"Alpha Story","release_matches":[{"id":"r123","title":"Collection Box"}]}),json!({"id":"v2","title":"Beta Story","release_matches":[{"id":"r123","title":"Collection Box"}]})])}
    #[test]fn compilation_member_review_uses_all_bound_names_and_incomplete_search_stays_review(){
        let mut result=compilation_response(matching::prepare("Collection Box",&[]).unwrap());assert!(eligible(&result).is_some());
        result["titles"]=json!(["Collection Box","Beta Story Soundtrack","Alpha Story Patch"]);assert_eq!(review_reason(&result),None);
        for member in ["Collection Box 「Beta Story OST」 (2wav+cue+log)","Collection Box 「Alpha Story」 Disc-1","Collection Box 「Beta Story Disk」","Collection Box 同梱 Disk"]{result["titles"]=json!(["Collection Box",member]);assert_eq!(review_reason(&result),None,"{member}");}
        for member in ["Collection Box 「Unrelated Game」 Disc-1","Collection Box 「Alpha Story 2」","Collection Box 「Beta Story Other Subtitle」","Collection Box 「Other Box」","Collection Box 「Beta Story OST」 Unknown Game"]{result["titles"]=json!(["Collection Box",member]);assert!(review_reason(&result).is_some(),"{member}");}
        result["titles"]=json!(["Collection Box"]);
        result["titles"].as_array_mut().unwrap().push(json!("Unrelated Game"));assert_eq!(review_reason(&result),Some("A different source name needs review: Unrelated Game".into()));
        result["titles"]=json!(["Collection Box"]);result["incomplete"]=json!(true);assert!(eligible(&result).is_none());result["warning"]=json!("Provider unavailable");assert_eq!(review_reason(&result),Some("Search incomplete. Refresh when VNDB is available.".into()));
    }
    #[tokio::test]async fn compilation_admits_all_works_atomically_as_automatic_main_bindings(){
        let(_t,db)=fixture(&["Collection Box","Collection Box"]);
        db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title,vndb_id,notes) VALUES('existing','My Beta','Original Beta','v2','Keep notes');").unwrap();
        let job=kick(&db,true).unwrap().unwrap();run_with(&db,&job,|i|async move{Ok(compilation_response(i))}).await.unwrap();
        {let c=db.lock().unwrap();for rid in ["r0","r1"]{let bindings=editions::bindings(&c,rid).unwrap();assert_eq!(bindings.len(),2);assert!(bindings.iter().all(|b|b.role=="main"&&b.release_id.is_none()));
            assert_eq!(c.query_row("SELECT w.vndb_id FROM resources r JOIN works w ON w.id=r.work_id WHERE r.id=?1",[rid],|r|r.get::<_,String>(0)).unwrap(),"v1");assert!(snapshot(&c,rid).unwrap().is_none());
            let evidence:String=c.query_row("SELECT evidence FROM match_items WHERE resource_id=?1",[rid],|r|r.get(0)).unwrap();let evidence:Value=serde_json::from_str(&evidence).unwrap();assert_eq!(evidence["results"][0]["match"]["compilation"]["vndb_ids"],json!(["v1","v2"]));
            assert_eq!(bound_candidates(&replay(&c,rid,&evidence).unwrap().unwrap()).len(),2);
        }assert_eq!(c.query_row("SELECT title||':'||notes FROM works WHERE id='existing'",[],|r|r.get::<_,String>(0)).unwrap(),"My Beta:Keep notes");}
        assert_eq!(scalar(&db,"SELECT count(*) FROM works"),2);assert_eq!(scalar(&db,"SELECT count(*) FROM releases"),0);assert_eq!(scalar(&db,"SELECT count(*) FROM resource_history"),2);
        assert_eq!(scalar(&db,"SELECT count(*) FROM events WHERE code='manual.bind'"),0);assert_eq!(scalar(&db,"SELECT count(*) FROM events WHERE code='match.attempt' AND json_extract(payload,'$.state')='matched'"),2);assert!(kick(&db,true).unwrap().is_none());
    }
    #[tokio::test]async fn compilation_history_and_attempt_failures_roll_back_every_work_and_binding(){
        for trigger in ["CREATE TRIGGER reject_history BEFORE INSERT ON resource_history BEGIN SELECT RAISE(ABORT,'History failure'); END;","CREATE TRIGGER reject_attempt BEFORE INSERT ON events WHEN NEW.code='match.attempt' BEGIN SELECT RAISE(ABORT,'Attempt failure'); END;"]{
            let(_t,db)=fixture(&["Collection Box"]);let job=kick(&db,true).unwrap().unwrap();db.lock().unwrap().execute_batch(trigger).unwrap();assert!(run_with(&db,&job,|i|async move{Ok(compilation_response(i))}).await.is_err());
            for table in ["works","resource_bindings","resource_history"]{assert_eq!(scalar(&db,&format!("SELECT count(*) FROM {table}")),0);}assert_eq!(scalar(&db,"SELECT count(*) FROM match_items WHERE state='pending'"),1);
        }
    }
    #[test]fn catalog_inputs_and_hint_order_are_unicode_independent_but_fingerprints_stay_raw(){
        let title="[240101][ブランド] サンプルタイトル";let path=format!("親ブランド/{title}");
        let members:Vec<String>=(0..40).map(|i|format!("{path}/[メーカー{i:02}] {}{i:02}.rar",if i%2==0{"カキクケコカ"}else{"ガギグゲゴ"})).collect();
        let nfd=|s:&str|s.nfd().collect::<String>();let hints=member_hints(members.clone());
        assert_eq!(hints,member_hints(members.iter().rev().map(|p|nfd(p)).collect()));assert_eq!(hints.len(),31);
        assert!(hints.windows(2).all(|p|p[0].chars().count()>p[1].chars().count()||(p[0].chars().count()==p[1].chars().count()&&p[0]<=p[1])));
        let(_t,db)=fixture(&[title]);let c=db.lock().unwrap();c.execute("DELETE FROM resource_files",[]).unwrap();c.execute("DELETE FROM files",[]).unwrap();
        for (i,p) in members.iter().enumerate(){let id=format!("f{i:02}");
            c.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES(?1,'root',?2,?2,16,'1','rar','fixture')",params![id,p]).unwrap();
            c.execute("INSERT INTO resource_files(resource_id,file_id) VALUES('r0',?1)",[&id]).unwrap();
        }
        let mut inputs=Vec::new();let mut fingerprints=Vec::new();
        for decomposed in [false,true]{
            let form=|s:&str|if decomposed{nfd(s)}else{s.to_string()};let raw_path=form(&path);
            c.execute("UPDATE resources SET relative_path=?1,title=?2 WHERE id='r0'",params![raw_path,form(title)]).unwrap();
            for (i,p) in members.iter().enumerate(){c.execute("UPDATE files SET relative_path=?1 WHERE id=?2",params![form(p),format!("f{i:02}")]).unwrap();}
            let snap=snapshot(&c,"r0").unwrap().unwrap();let mut hash=Sha256::new();hash.update(raw_path.as_bytes());hash.update(snap.revision.to_le_bytes());
            let mut s=c.prepare("SELECT id,relative_path,size,mtime FROM files ORDER BY length(relative_path) DESC,relative_path,id").unwrap();
            for row in s.query_map([],|r|Ok(json!([r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?]))).unwrap(){hash.update(row.unwrap().to_string().as_bytes());}
            assert_eq!(snap.fingerprint,hex::encode(hash.finalize()));fingerprints.push(snap.fingerprint);
            let input=snap.input;inputs.push(json!({"titles":input.titles,"queries":input.queries,"brands":input.brands,"dates":input.dates,"direct_id":input.direct_id}));
        }
        assert_eq!(inputs[0],inputs[1]);assert_ne!(fingerprints[0],fingerprints[1]);assert_eq!(inputs[0]["titles"][0],"サンプルタイトル");
        assert!(!inputs[0]["titles"].as_array().unwrap().iter().any(|t|t=="親ブランド"));
    }
    #[test]fn exact_title_file_stems_are_ancillary_with_any_short_extension(){
        let title="女装お嬢様への異常な愛情";let candidate=json!({"id":"v13508","title":title});
        for ext in ["cf","exe","ini","cfg","dat","a","X1234"]{
            let member=format!("{title}.{ext}");assert!(matching::title_anchored_member(&member,&[title],&[]));
            assert_eq!(review_reason(&response(matching::prepare(title,&[format!("RJ119660/{member}")]).unwrap(),vec![candidate.clone()])),None);
        }
        let folder="[Key] CLANNAD";let input=matching::prepare(folder,&[format!("{folder}.cf")]).unwrap();
        assert_eq!(review_reason(&response(input,vec![json!({"id":"v4","title":"CLANNAD","developers":[{"name":"Key"}]})])),None);
        for member in [format!("{title}2.cf"),format!("{title}別作品.cf"),format!("{title}.abcdef"),format!("{title}.a-b"),"別の作品タイトル.cf".into()]{
            assert!(!matching::title_anchored_member(&member,&[title],&[]),"{member}");
            assert!(review_reason(&response(matching::prepare(title,&[member.clone()]).unwrap(),vec![candidate.clone()])).is_some(),"{member}");
        }
    }
    #[test]fn replay_includes_bound_resources_and_shares_live_decisions_without_writes(){
        let(_t,db)=fixture(&["Clannad","Unknown title","Clannad Anniversary Collection"]);
        db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Clannad','Clannad','v4')",[]).unwrap();
        editions::bind_primary(&db,"r0","w","Fixture",1).unwrap();let c=db.lock().unwrap();
        // A split bundle child must not inherit the collection's title.
        c.execute("UPDATE resources SET relative_path='Unrelated Bundle/Clannad.rar' WHERE id='r0'",[]).unwrap();
        c.execute("UPDATE files SET relative_path='Unrelated Bundle/Clannad.rar' WHERE id='r0'",[]).unwrap();
        c.execute_batch("PRAGMA query_only=ON").unwrap();let changes=c.total_changes();
        let stale=json!({"results":[{"id":"v4","title":"Clannad","match":{"strength":"review","ambiguous":true}}],"titles":["Stale title"],"incomplete":false});
        assert!(snapshot(&c,"r0").unwrap().is_none());assert!(explain(&c,"r0",&stale).is_err());
        let bound=replay(&c,"r0",&stale).unwrap().unwrap();assert_eq!(eligible(&bound).unwrap()["id"],"v4");assert_eq!(bound["titles"],json!(["Clannad"]));
        let unmatched=replay(&c,"r1",&stale).unwrap().unwrap();assert_eq!(unmatched,explain(&c,"r1",&stale).unwrap());assert!(eligible(&unmatched).is_none());
        let compilation=json!({"results":[{"id":"v5","title":"Other Work","release_matches":[{"id":"r1","title":"Clannad Anniversary Collection"}]},{"id":"v4","title":"Clannad","release_matches":[{"id":"r1","title":"Clannad Anniversary Collection"}]}],"incomplete":false});
        let result=replay(&c,"r2",&compilation).unwrap().unwrap();assert_eq!(result,explain(&c,"r2",&compilation).unwrap());assert_eq!(eligible(&result).unwrap()["id"],"v4");
        assert_eq!(replay(&c,"r1",&json!({})).unwrap().unwrap()["decision"]["reason"],"Not checked against VNDB yet.");
        let mut partial=stale.clone();partial["incomplete"]=json!(true);assert!(eligible(&replay(&c,"r0",&partial).unwrap().unwrap()).is_none());
        assert_eq!(c.total_changes(),changes);
    }
    #[test]fn automatic_matching_is_on_until_turned_off(){
        let(_t,db)=fixture(&["Clannad"]);let c=db.lock().unwrap();assert!(enabled(&c).unwrap());
        c.execute("INSERT INTO settings(key,value) VALUES('auto_match_enabled','0')",[]).unwrap();assert!(!enabled(&c).unwrap());
    }
    #[tokio::test]async fn scoped_issue_retry_runs_without_enabling_global_matching(){
        let(_t,db)=fixture(&["Clannad","Kanon"]);db.lock().unwrap().execute("INSERT INTO settings(key,value) VALUES('auto_match_enabled','0')",[]).unwrap();let job={let mut c=db.lock().unwrap();let tx=c.transaction().unwrap();crate::issues::reconcile(&tx).unwrap();let id:i64=tx.query_row("SELECT seq FROM issues WHERE resource_id='r0'",[],|r|r.get(0)).unwrap();let job=crate::issues::queue_retry(&tx,id,1).unwrap();assert_eq!(crate::issues::queue_retry(&tx,id,1).unwrap(),job);assert!(!enabled(&tx).unwrap());assert_eq!(explicit_retry(&tx).unwrap(),Some(job.clone()));tx.commit().unwrap();job};
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
    #[test]fn incomplete_search_says_whether_a_retry_can_help(){
        let outage=json!({"incomplete":true,"warning":"VNDB returned 503 Service Unavailable. Try again later.","results":[]});
        assert_eq!(review_reason(&outage).as_deref(),Some("Search incomplete. Refresh when VNDB is available."));
        let crowded=json!({"incomplete":true,"warning":null,"results":[]});
        assert_eq!(review_reason(&crowded).as_deref(),Some("VNDB has too many works with this name to be sure. Choose the work manually."));
    }
    #[test]fn bonus_members_file_into_the_matched_card_but_another_work_still_needs_review(){
        let input=matching::prepare("アマツツミ",&[]).unwrap();let ranked=matching::rank(&input,vec![json!({"id":"v18852","title":"Amatsutsumi","alttitle":"アマツツミ"})]);
        let response=|members:&[&str]|{let mut titles=vec!["アマツツミ"];titles.extend(members);json!({"results":ranked,"titles":titles,"incomplete":false})};
        assert_eq!(review_reason(&response(&["アマツツミ 同梱特典 サウンドトラック","Bonus","Boku Sensei Crak"])),Some("A different source name needs review: Boku Sensei Crak".into()));
        assert_eq!(review_reason(&response(&["アマツツミ 同梱特典 サウンドトラック","Bonus","AmatsuCrak","APT-183"])),None);
        assert_eq!(review_reason(&response(&["Bonus","エッチな皇女バニーのご奉仕"])),Some("A different source name needs review: エッチな皇女バニーのご奉仕".into()));
        // A bonus-only folder name never vouches for itself.
        let bonus=json!({"results":ranked,"titles":["Bonus","アマツツミ"],"incomplete":false});assert!(review_reason(&bonus).is_some());
    }
    #[test]fn legacy_quoted_bonus_members_still_file_into_the_matched_card(){
        for (title,member) in [
            ("宿りし乙女の誓いと魔法","宿りし乙女の誓いと魔法 豪華版特典「オリジナルサウンドトラック」"),
            ("アマカノ3","アマカノ3 FANZA特典「恋人メイドのご奉仕お目覚めボイス」"),
            ("トリノライン","トリノライン 豪華版 サウンドトラック 「feathered」"),
            ("SPIRAL!!-Limited Edition-","SPIRAL!!-Limited Edition- 同梱特典 『SHUFFLE!』本編"),
            ("奴隷姫騎士と奴隷侍女とのスローライフ","[241025][1305021][WAFFLE] 奴隷姫騎士と奴隷侍女とのスローライフ アニメ追加版 オフィシャル通販特典 「奴隷たちとの畑づくり」 (files).rar"),
            ("人間不信の錬金術師と元兵士のホムンクルス","[241025][1305022][WAFFLE] 人間不信の錬金術師と元兵士のホムンクルス アニメ追加版 オフィシャル通販特典 「錬金術師の過去」 (files).rar"),
            ("恋愛弱者な幼馴染少女と恋愛強者な彼女","[241025][1297034][WAFFLE] 恋愛弱者な幼馴染少女と恋愛強者な彼女 Webアンケート特典 (files).rar"),
            ("恋愛弱者な幼馴染少女と恋愛強者な彼女","[241025][1297034][WAFFLE] 恋愛弱者な幼馴染少女と恋愛強者な彼女 エッチな音声データ (ogg).rar"),
            ("恋愛弱者な幼馴染少女と恋愛強者な彼女","[241025][1297034][WAFFLE] 恋愛弱者な幼馴染少女と恋愛強者な彼女 オフィシャル通販特典 (files).rar"),
        ]{
            assert_eq!(matching::clean_name(member),title,"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
            // Archive labels reach admission after clean_name; bare member labels also pass directly.
            for name in [member.to_string(),matching::clean_name(member)].into_iter().filter(|n|!n.starts_with('[')){
                assert!(matching::names_with_extras(&name,&[title]),"{member}");
                assert!(matching::ancillary_member(&name,&[title]),"{member}");
                for titles in [json!([title,name]),json!([name,title])]{
                    assert_eq!(review_reason(&json!({"results":ranked,"titles":titles,"incomplete":false})),None,"{member}");
                }
            }
        }
    }
    #[test]fn legacy_irotoridori_hd_compilation_1181108_stays_accepted(){
        let title="いろとりどりのセカイ HD Re:GENERATION";
        let hints:Vec<String>=["いろとりどりのセカイHD","いろとりどりのヒカリHD","紅い瞳に映るセカイHD","10th Anniversary メモリアルDIARY"].iter()
            .map(|bonus|format!("1181108/[220930][1181108][FAVORITE] {title} DL版「{bonus}」 (files).rar")).collect();
        let input=matching::prepare("1181108",&hints).unwrap();assert_eq!(input.titles,vec![title]);
        let ranked=matching::rank(&input,vec![json!({"id":"v5834","title":"Irotoridori no Sekai","alttitle":"いろとりどりのセカイ","developers":[{"name":"FAVORITE"}],
            "release_matches":[{"title":"いろとりどりのセカイ HD Re:GENERATION パッケージ版","released":"2022-09-30"}]})]);
        assert_eq!(review_reason(&json!({"results":ranked,"titles":input.titles,"brands":input.brands,"dates":input.dates,"incomplete":false})),None);
    }
    #[test]fn title_anchored_media_and_distribution_members_are_ancillary(){
        for (title,member) in [
            ("千恋*万花","千恋*万花 ドラマCD『「Blue sky」の日』『after』"),
            ("アマナツ+","アマナツ+ SPECIAL SOUNDTRACK (flac+cue)"),
            ("アマナツ","アマナツ + SPECIAL SOUNDTRACK (flac+cue)"),
            ("金色ラブリッチェ","金色ラブリッチェ SPECIAL SOUNDTRACK (flac+cue+rr3)"),
            ("千恋*万花","千恋*万花 キャラクターソング Vol.4 「Blue sky」/レナ・リヒテナウアー (flac+cue+rr3)"),
            ("よめがみ My Sweet Goddess!","よめがみ My Sweet Goddess! オープニング主題歌 (flac+cue+rr3)"),
            ("さくらの雲*スカアレットの恋","さくらの雲*スカアレットの恋 コンプリートサウンドアルバム (flac+cue+rr3)"),
            ("アインシュタインより愛を込めて","アインシュタインより愛を込めて COMPLETE VOCAL COLLECTION (flac+cue+rr3)"),
            ("千の刃濤、桃花染の皇姫‐花あかり‐","千の刃濤、桃花染の皇姫‐花あかり‐ マキシシングル feat. Airots"),
            ("キミと恋するハッピーサマー","キミと恋するハッピーサマー ミニサウンドトラック"),
            ("将軍様はお年頃ふぁんでぃすく -御三家だヨ! 全員集合-","将軍様はお年頃ふぁんでぃすく -御三家だヨ! 全員集合- 演出強化パッチ"),
            ("二人と始める打算的なラブコメ","二人と始める打算的なラブコメ PlayDRM認証回避パッチ"),
            ("MUSICUS!","MUSICUS! R18版演出調整パッチ Ver1.0.1"),
            ("白昼夢の青写真","白昼夢の青写真 とらのあな特典ドラマCD"),
            ("出会って5分は俺のもの!時間停止と不可避な運命","出会って5分は俺のもの!時間停止と不可避な運命 特別限定版 オリジナルサウンドトラック"),
            ("千の刃濤、桃花染の皇姫","千の刃濤、桃花染の皇姫 菜摘テーマファイル"),
            ("ピュア×コネクト","ピュア×コネクト 凹んだときに聴きたい励ましCD"),
            ("喫茶ステラと死神の蝶","(PD輸入) (18禁ゲーム) [190927] [ゆずソフト] 喫茶ステラと死神の蝶 キャラクターソング vol.1 (flac+cue)"),
            ("リアライブ","18禁ゲーム) [190329] [パープルソフトウェア] リアライブ"),
            ("星織ユメミライ","水樹瞳@568087-[140725] [tone work’s] 星織ユメミライ"),
            ("恋愛×ロワイアル","ゲーム「恋愛×ロワイアル」オリジナルサウンドトラック"),
            ("スタディ§ステディ2","ま~まれぇど「スタディ§ステディ2」メロンブックス特典DLC"),
            ("はじめるセカイの理想論","Whirlpool「はじめるセカイの理想論」メロンブックス特典ボイスドラマDLC「…」"),
            ("戦国†恋姫EX壱","『戦国†恋姫EX壱』ボイス特典版_龍巳システムボイス"),
            ("ワールド・エレクション","[Whirlpool]+ワールド・エレクション+10週年スペシャルコンテンツDVD++免认证补丁(認証回避Patch)"),
            ("ワールド・エレクション","[Whirlpool] ワールド・エレクション 10週年スペシャルコンテンツDVD"),
        ]{
            assert!(matching::title_anchored_member(member,&[title],&["ま～まれぇど","Whirlpool"]),"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
            for name in [member.to_string(),matching::clean_name(member)]{
                let r=json!({"results":ranked,"titles":[title,name],"brands":["ま～まれぇど","Whirlpool"],"incomplete":false});assert_eq!(review_reason(&r),None,"{member}");
            }
        }
    }
    #[test]fn quoted_main_title_members_need_brand_or_date_corroboration(){
        for (title,member,maker) in [
            ("戦国†恋姫EX壱～奥州の独眼竜編～","『戦国†恋姫EX壱』ボイス特典版_龍巳システムボイス","BaseSon"),
            ("はじめるセカイの理想論 -goodbye world index-","Whirlpool「はじめるセカイの理想論」メロンブックス特典ボイスドラマDLC「シスター・ティアの懺悔のお時間」","Whirlpool"),
        ]{
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title,"developers":[{"name":maker}]})]);
            let mut r=json!({"results":ranked,"titles":[title,member],"brands":[maker],"incomplete":false});assert_eq!(review_reason(&r),None,"{member}");
            r["brands"]=json!(["Unrelated maker"]);assert!(review_reason(&r).is_some(),"{member}");
        }
    }
    #[test]fn other_products_and_unlicensed_quotes_still_require_review(){
        for (title,member) in [
            ("ハピメア REGRET END","ハピメア REGRET END 「ハピメアFD」"),
            ("素晴らしき日々 ~不連続存在~ 15th Anniversary edition","素晴らしき日々 ~不連続存在~ 15th Anniversary edition 「終ノ空remake -2025ver-」"),
            ("アンラベル・トリガー","エッチな皇女バニーのご奉仕"),
            ("ジュエリー・ハーツ・アカデミア","オリジナルデジタルコンテンツ『美少女探偵アリアンナ ~レインボーダイヤを守り抜け~』"),
            ("Secret Agent","Secret Agent ミニゲーム3本パック"),
            ("妻みぐい3","妻みぐい3 スピンオフ 純愛編 (認証回避済) (files) ※自炊"),
            ("相思相愛ロリータ","(同人音声)[RJ153712][夜のひつじ]安眠添い寝ロリータ(R18)"),
            ("放課後シンデレラ","放課後シンデレラFD"),
            ("アマナツ","アマナツ+ SPECIAL SOUNDTRACK (flac+cue)"),("MUSICUS!","MUSICUS!+ SPECIAL SOUNDTRACK (flac+cue)"),
            ("アマツツミ","アマツツミ2 特典"),("アマツツミ","アマツツミ ファンディスク"),
            ("アマツツミ","Song of Saya"),("アマツツミ","Boku Sensei Crak"),
            ("恋する乙女と守護の楯 Reboot The“SHIELD-9”","恋する乙女と守護の楯 Reboot The“SHIELD-9” 「ORIGIN Disk」"),
            ("アマツツミ","Disk「アマツツミ」"),("アマツツミ","アマツツミ2「アマツツミ」"),("アマツツミ","別の作品「アマツツミ」"),
        ]{assert_member_review(title,member);}
        for tail in ["2 特典","FD","ファンディスク","アペンド","スピンオフ","after","外伝","remake","リメイク","ミニゲーム","「別の作品」"]{
            for title in ["アマツツミ","MUSICUS!"]{assert_member_review(title,&format!("{title} {tail}"));}
        }
    }
    #[test]fn fallback_exclusions_do_not_veto_legacy_bonus_acceptance(){
        for tail in ["特典 「別の作品」","特典FD","特典 ミニゲーム","特典 remake","特典 スピンオフ","特典 Disk「別の作品」","特典ディスク『別の作品』","(ベータ)"]{
            let title="アマツツミ";let member=format!("{title} {tail}");
            assert!(!matching::title_anchored_member(&member,&[title],&[]),"{member}");
            assert!(matching::ancillary_member(&member,&[title]),"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,member],"incomplete":false})),None);
        }
    }
    #[test]fn edition_and_descriptive_bonus_members_are_anchored(){
        for (title,member) in [
            ("Eスクールライフ","Eスクールライフ E初回版"),
            ("どっちのiが好きですか?","どっちのiが好きですか? i初回版"),
            ("キスから始まるギャルの恋","キスから始まるギャルの恋 プレミアムエディション"),
            ("プリンセスハートリンク 〜剣姫たちの艶舞〜","プリンセスハートリンク 〜剣姫たちの艶舞〜 プレミアム版"),
            ("宿星のガールフレンド2 ―the destiny star of girlfriend―","宿星のガールフレンド2 ―the destiny star of girlfriend― パッケージ販売限定版"),
            ("僕の未来は、恋と課金と。","僕の未来は、恋と課金と。 特別限定版"),
            ("ウチはもう、延期できない","ウチはもう、延期できない。 特別限定版"),
            ("響野さん家はエロゲ屋さん!","響野さん家はエロゲ屋さん! 特版 オリジナルサウンドトラック"),
            ("出会って5分は俺のもの!時間停止と不可避な運命","出会って5分は俺のもの!時間停止と不可避な運命 特別限定版 (iso+mds+マニュアル+rr3)"),
            ("虜ノ姉妹 ~淫らに弄ばれる運命の迷い子~ if エロパッチ対応豪華版","虜ノ姉妹 ~淫らに弄ばれる運命の迷い子~ if エロパッチ対応豪華版 (SF認証 iso+mds+rr3)"),
            ("SISTERS","SISTERS ULTRA EDITION"),
            ("仄暗き時の果てより","仄暗き時の果てより R18"),
            ("ANIM Mother＆Wife","ANIM Mother&Wife"),
            ("Princess x Princess","Princess×Princess"),
            ("堕ちるパート妻 年下男との情事に溺れた果てに…","堕ちるパート妻 年下男との情事に溺れた果てに... [D_280070]"),
            ("はぴねす!2 Sakura Celebration","はぴねす!2 Sakura Celebration + Digital Art Works"),
            ("DeepOne -領界侵犯-","DeepOne -領界侵犯- 特別限定版 + Soundtrack + Artwork"),
            ("ハジラブ -Making*Lovers-","ハジラブ -Making*Lovers-&Making*LoversフルHDリマスター同梱限定セット版 (OST & Maxi & スケパッチ付)"),
            ("千恋*万花","千恋*万花 ほこほこぬくぬくあったまろCD (flac+cue+rr3)"),
            ("桜ひとひら恋もよう","桜ひとひら恋もよう げっちゅ屋店舗特典ヂスク (flac+cue+rr3)"),
            ("まほ×ろば","まほ×ろば 衣服が透けちゃうマジックROM"),
            ("かりぐらし恋愛","かりぐらし恋愛 ASa Project 同梱10周年ドラマCD"),
            ("恋はそっと咲く花のように","恋はそっと咲く花のように ずっと兄さまに寄り添っていたいパッチ"),
            ("若葉色のカルテット","若葉色のカルテット OP/EDフルコーラスマキシシングルCD (flac+cue+rr3)"),
            ("俺の姿が、透明に!?不可視の薬と数奇な運命","俺の姿が、透明に!?不可視の薬(インビジブル)と数奇な運命 全店舗特典ドラマCD"),
            ("ガラス姫と鏡の従者","ガラス姫と鏡の従者 サンドリヨン学園公式ボーカルソングCD (wav+cue+log)"),
            ("カスタムオーダーメイド3D2","カスタムオーダーメイド3D2 カスタムメイド3D2互換アップデートパッチ(純真・ツンデレ・クーデレ)"),
            ("ゆらめく心に満ちた世界で、君の夢と欲望は叶うか","ゆらめく心に満ちた世界で、君の夢と欲望は叶うか 「クロ&シロ」 エッチ解放パッチ"),
            ("虜ノ鎖 ~処女たちを穢す淫らな楔~","虜ノ鎖 ~処女たちを穢す淫らな楔~ 期間限定パッチDLC 「処女にかる男たちパッチ」& 「從順な堕女にされる処女パッチ」"),
            ("乙女が彩る恋のエッセンス","乙女が彩る恋のエッセンス DLC02 サイト内コンテンツボイス集その1&スティックポスターデータ&メッセージペーパー"),
            ("その花びらにくちづけを 初めて出逢ったあの日から","その花びらにくちづけを 初めて出逢ったあの日から Getchu屋特典ドラマCD 「その花びらにくちづけを いれかえまいんど」 (mdf+mds)※自炊"),
            ("オトメ*ドメイン","オトメ*ドメイン メディオ!特典ドラマCD 「柚子さんの裸エプロンは新妻の特訓です!」 (mds+mdf) ※自炊"),
            ("未来ラジオと人工鳩","未来ラジオと人工鳩 とらのあな限定 「未来ラジオラジオ特別編」 水月編"),
            ("キュリオディーラー","キュリオディーラー カウントダウン漫画&ボイス"),
            ("見上げてごらん、夜空の星を FINE DAYS","見上げてごらん夜空の星をFINEDAYSminiSoundtrack「and Little Planets」"),
            ("制服カノジョ3","制服カノジョ3 めいか初恋BOX同梱特典ボイスドラマCD「ある日のカノジョ」"),
        ]{
            assert!(matching::title_anchored_member(member,&[title],&[]),"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
            for name in [member.to_string(),matching::clean_name(member)]{
                assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,name],"incomplete":false})),None,"{member}");
            }
        }
        for (title,alias,member) in [
            ("アオイトリ","AOITORI","アオイトリ -AOITORI-"),
            ("ノラと皇女と野良猫ハート2","Nora, Princess, and Crying Cat. 2","ノラと皇女と野良猫ハート2 -Nora,Princess,and Crying Cat.-"),
        ]{
            assert!(matching::title_anchored_member(member,&[title,alias],&[]),"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title,"aliases":[alias]})]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":[member,title]})),None);
            assert!(!matching::title_anchored_member(member,&[title],&[]),"unknown translation: {member}");
        }
    }
    #[test]fn edition_labels_do_not_license_other_games(){
        for (title,member) in [
            ("恋する乙女と守護の楯 Reboot The“SHIELD-9”","恋する乙女と守護の楯 Reboot The“SHIELD-9” 「ORIGIN Disk」"),
            ("放課後シンデレラ","放課後シンデレラ 20周年記念初回豪華版 SPECIAL DISC"),
            ("百千の定にかわたれし剋","百千の定にかわたれし剋 AP01 「もしもその時出会っていたならDATA」"),
            ("抜きゲーみたいな島に住んでる貧乳(わたし)はどうすりゃいいですか?","抜きゲーみたいな島に住んでる貧乳(わたし)はどうすりゃいいですか? 1+2 Remaster + Append"),
            ("虜ノ焔~淫欲の炎に満ちた地下闘技場~","虜ノ焔~淫欲の炎に満ちた地下闘技場~ if エロパッチ"),
            ("Secret Agent","Secret Agent ミニゲーム3本パック"),
            ("妻みぐい3","妻みぐい3 スピンオフ 純愛編"),
            ("放課後シンデレラ","放課後シンデレラFD"),
            ("ハピメア REGRET END","ハピメア REGRET END 「ハピメアFD」"),
            ("アンラベル・トリガー","エッチな皇女バニーのご奉仕"),
            ("ジュエリー・ハーツ・アカデミア","オリジナルデジタルコンテンツ『美少女探偵アリアンナ…』"),
            ("まおかつ!","Maokatsu Crk"),("ぼくと先生の乳淫せいかつ","Boku Sensei Crak"),
            ("星の乙女と六華の姉妹","hoshioto_new stage2"),("蒼の彼方のフォーリズム","4rhythm WP & 4koma"),
            ("アマツツミ","OP主題歌『ずっとそばで...』short ver"),
        ]{assert_member_review(title,member);}
        for tail in ["Disk","DISC","ディスク","ヂスク","SPECIAL DISC","別作品 DISC","2 ドラマCD","2 特典CD","if 特典CD","FD ドラマCD","スピンオフ ドラマCD","SPECIAL DISC 特典CD"]{
            assert_member_review("アマツツミ",&format!("アマツツミ {tail}"));
        }
        // An if folder may vouch for its own patch; the real 1330597 folder is the base work.
        let title="虜ノ焔~淫欲の炎に満ちた地下闘技場~ if";
        assert!(matching::title_anchored_member(&format!("{title} エロパッチ"),&[title],&[]));
    }
    #[test]fn folder_identity_does_not_vouch_for_a_maker_or_an_unknown_subtitle(){
        assert_eq!(matching::normalized("ANIM Mother&Wife"),matching::normalized("ANIM Mother＆Wife"));
        for (folder,title) in [
            ("ANIM Mother&Wife","いつまでも息子のままじゃいられない!6 ~巨乳でクールな母さんのおっぱいに抱かれながらたっぷり甘えたいっ!~"),
            ("DeepOne -領界侵犯- 特別限定版 + Soundtrack + Artwork","Deep One -ディープワン-"),
        ]{
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":[folder,title]})),Some(format!("A different source name needs review: {folder}")));
        }
    }
    #[test]fn edition_fallback_uses_work_titles_and_the_folders_release(){
        for (folder,member,title,maker,release) in [
            ("SISTERS~夏の最後の日~ Ultra Edition","SISTERS ULTRA EDITION","SISTERS ～夏の最後の日～","Jellyfish","SISTERS ～夏の最後の日～ Ultra Edition"),
            ("虜ノ姉妹 ~淫らに弄ばれる運命の迷い子~ if エロパッチ対応豪華版","虜ノ姉妹 ~淫らに弄ばれる運命の迷い子~ if エロパッチ対応豪華版 (SF認証 iso+mds+rr3)","虜ノ姉妹 ～淫らに弄ばれる運命の迷い子～","Guilty","虜ノ姉妹 ～淫らに弄ばれる運命の迷い子～ if エロパッチ対応豪華版"),
            ("僕の未来は、恋と課金と。 特別限定版","僕の未来は、恋と課金と。~Charge To The Future~ 特別限定版","僕の未来は、恋と課金と。～Charge To The Future～","Sonora","僕の未来は、恋と課金と。～Charge To The Future～ 特別限定版"),
            ("プリンセスハートリンク","プリンセスハートリンク 〜剣姫たちの艶舞〜 プレミアム版","プリンセスハートリンク 〜剣姫たちの艶舞〜","裸足少女",""),
            ("キスから始まるギャルの恋 〜くるみのウワサとホントのキモチ〜","キスから始まるギャルの恋 プレミアムエディション","キスから始まるギャルの恋 〜くるみのウワサとホントのキモチ〜","プレカノ",""),
        ]{
            let mut input=matching::prepare(folder,&[]).unwrap();input.titles.push(member.into());input.brands.push(maker.into());
            let ranked=matching::rank(&input,vec![json!({"id":"v1","title":title,"developers":[{"name":maker}],"release_matches":[{"title":release}]})]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":input.titles,"brands":input.brands})),None,"{member}");
        }
        // Merely knowing about an if release cannot make the base folder its if edition.
        let title="虜ノ焔~淫欲の炎に満ちた地下闘技場~";let member=format!("{title} if エロパッチ");
        let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v56239","title":title,"release_matches":[{"title":format!("{title} if")}]})]);
        assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,member]})),Some(format!("A different source name needs review: {member}")));
        for (title,alias,member) in [
            ("ゆびきり婚約ロリイタ","与你拉钩起誓的婚约","ゆびきり婚約ロリイタ-与你拉钩起誓的婚约"),
            ("相思相愛ロリータの生活","与小萝莉相思相爱的生活","相思相愛ロリータの生活-与小萝莉相思相爱的生活"),
            ("お泊まり恋人ロリータ","お泊まり恋人ロリータ","お泊まり恋人ロリータ-お泊まり恋人ロリータ"),
            ("愛欲姉妹ロリータ","愛欲姉妹ロリータ","愛欲姉妹ロリータ-愛欲姉妹ロリータ"),
        ]{
            assert!(!matching::title_anchored_member(member,&[title,alias],&[]),"{member}");
        }
        // Admitting the named bundle label does not admit a separate bundled game.
        let title="ハジラブ -Making*Lovers-";let other="Making*LoversフルHDリマスター";
        let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v30456","title":title})]);
        assert_eq!(review_reason(&json!({"results":ranked,"titles":["ハジラブ -Making*Lovers-&Making*LoversフルHDリマスター同梱限定セット版 (OST & Maxi & スケパッチ付)",title,other]})),Some(format!("A different source name needs review: {other}")));
    }
    #[test]fn candidate_owned_chinese_and_fan_release_members(){
        for (id,title,translation,member) in [
            ("v10569","孤独に効く百合","孤独少女的百合物语","PSP 孤独少女的百合物语"),
            ("v10568","恋妻くずし","恋妻くずし","【脸肿汉化组】《恋妻くずし》汉化补丁v1.0"),
            ("v24202","堕落ロイヤル聖処女","堕落皇家聖處女","【變態協會漢化組】[ADV][夜のひつじ]墮落皇家聖處女 漢化補丁 V1.0"),
            ("v22436","ハーレム双子ロリータ","双子洛丽塔后宫","双子洛丽塔后宫KRKR版"),
            ("v18874","ゆびきり婚約ロリイタ","与你拉钩起誓的婚约","ゆびきり婚約ロリイタ-与你拉钩起誓的婚约"),
            ("v19771","相思相愛ロリータの生活","与小萝莉相思相爱的生活","相思相愛ロリータの生活-与小萝莉相思相爱的生活"),
            ("v20624","お泊まり恋人ロリータ","同居恋人洛丽塔","お泊まり恋人ロリータ-お泊まり恋人ロリータ"),
            ("v25127","愛欲姉妹ロリータ","爱欲姐妹洛丽塔","愛欲姉妹ロリータ-愛欲姉妹ロリータ"),
            ("v14850","妹「お姉ちゃんクソビッチなんで私にしませんか」","妹「お姉ちゃんクソビッチなんで私にしませんか」","お姉ちゃんクソビッチなんで私にしませんか"),
            ("v15976","幼馴染の心が読めたらどうするか?","幼馴染の心が読めたらどうするか?","(C86) (同人ゲーム) [140906] [夜のひつじ] 幼馴染の心が読めたらどうするか? [RJ140827]"),
            ("v27389","誘惑なまいきロリータ","诱惑、自大、洛丽塔","誘惑なまいきロリータ [RJ275492]"),
        ]{
            // Exercise both VNDB title fields and aliases without adding any folder/release spellings.
            for field in ["titles","aliases"]{
                let mut candidate=json!({"id":id,"title":title,"developers":[{"name":"Yoru no Hitsuji","original":"夜のひつじ"}]});
                candidate[field]=if field=="titles"{json!([{"title":translation}])}else{json!([translation])};
                let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![candidate.clone()]);
                for name in [member.to_string(),matching::clean_name(member)]{
                    assert!(matching::fan_release_member(&name,&candidate),"{id}: {name}");
                    assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,name]})),None,"{id}: {member}");
                }
                assert_eq!(review_reason(&response(matching::prepare(title,&[member.into()]).unwrap(),vec![candidate])),None,"{id}: prepared member");
            }
        }
        // These observed forms survive NFKC and therefore need the explicit local map.
        assert_ne!(matching::normalized("墮落皇家聖處女"),matching::normalized("堕落皇家聖処女"));
        let candidate=json!({"id":"v24202","title":"堕落ロイヤル聖処女","titles":[{"title":"堕落皇家聖處女"}]});
        assert!(matching::fan_release_member("墮落皇家聖処女 漢化版",&candidate));
        let candidate=json!({"id":"v10569","title":"孤独に効く百合","titles":[{"title":"孤独少女的百合物语"}]});
        for platform in ["PSP","PSV","NS","Switch","Android","KRKR版","吉里吉里版"]{
            assert!(matching::fan_release_member(&format!("{platform} 孤独少女的百合物语"),&candidate));
            assert!(matching::fan_release_member(&format!("孤独少女的百合物语 {platform}"),&candidate));
        }
        for group in ["【脸肿汉化组】","【變態協會漢化組】","【翻译社】"]{
            for tail in ["汉化","漢化","汉化补丁","漢化補丁","汉化版","漢化版"]{
                assert!(matching::fan_release_member(&format!("{group}[AVG]《孤独少女的百合物语》{tail} Ver.1.0"),&candidate));
            }
        }
    }
    #[test]fn fan_release_members_require_a_complete_own_title(){
        for (id,title,member) in [
            ("v16626","相思相愛ロリータ","(同人音声)[RJ153712][夜のひつじ]安眠添い寝ロリータ(R18)"),
            ("v24279","世界の始まりゴッコ","世界の始まりゴッコ worldbegin_v1.0.0_win"),
            ("v18874","ゆびきり婚約ロリイタ","ゆびきり婚約ロリイタ-与小萝莉相思相爱的生活"),
            ("v18874","ゆびきり婚約ロリイタ","与小萝莉相思相爱的生活-ゆびきり婚約ロリイタ"),
            ("v14850","妹「お姉ちゃんクソビッチなんで私にしませんか」","お姉ちゃん"),
            ("v10568","恋妻くずし","【脸肿汉化组】"),
            ("v10568","恋妻くずし","【脸肿汉化组】汉化补丁v1.0"),
            ("v10568","恋妻くずし","【脸肿汉化组】《恋妻くずし2》汉化补丁v1.0"),
            ("v10568","恋妻くずし","【脸肿汉化组】《恋妻くずし》未知补丁v1.0"),
        ]{
            let candidate=json!({"id":id,"title":title,"developers":[{"original":"夜のひつじ"}]});
            assert!(!matching::fan_release_member(member,&candidate),"{member}");
            let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![candidate]);
            for name in [member.to_string(),matching::clean_name(member)]{
                assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,name]})),Some(format!("A different source name needs review: {name}")));
            }
        }
        // Synthetic boundary variants of the real quoted title isolate the eight-character guard.
        for (body,allowed) in [("お姉ちゃんクソ",false),("お姉ちゃんクソビ",true)]{
            let candidate=json!({"id":"v14850","title":format!("妹「{body}」")});
            assert_eq!(matching::fan_release_member(body,&candidate),allowed,"{body}");
        }
        let own=json!({"id":"v10568","title":"恋妻くずし"});
        let other=json!({"id":"v10569","title":"孤独に効く百合","titles":[{"title":"孤独少女的百合物语"}]});
        for member in ["PSP 孤独少女的百合物语","恋妻くずし-孤独少女的百合物语"]{
            let ranked=matching::rank(&matching::prepare("恋妻くずし",&[]).unwrap(),vec![own.clone(),other.clone()]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":["恋妻くずし",member]})),Some(format!("A different source name needs review: {member}")));
        }
        let mut release=own.clone();release["release_matches"]=json!([{"title":"孤独少女的百合物语"}]);
        assert!(!matching::fan_release_member("PSP 孤独少女的百合物语",&release));
    }
    #[test]fn candidate_corroborated_code_and_generic_members(){
        for (candidate,member) in [
            (json!({"id":"v27365","title":"Maokatsu! ~Maou to Yuusha no Idol Seikatsu~","alttitle":"まおかつ！ ～魔王と勇者のアイドル生活～"}),"Maokatsu Crk"),
            (json!({"id":"v31700","title":"Hoshi no Otome to Rikka no Shimai","alttitle":"星の乙女と六華の姉妹","aliases":["Hoshioto","星乙"]}),"hoshioto_new stage2"),
            (json!({"id":"v12849","title":"Ao no Kanata no Four Rhythm","alttitle":"蒼の彼方のフォーリズム","aliases":["Beyond the sky, into the firmament.","あおかな","Four Rhythm Across the Blue","苍彼","4Rhythm","Aokana","아오카나"]}),"4rhythm WP & 4koma"),
            (json!({"id":"v28131","title":"Boku to Sensei no Nyuuin Seikatsu","alttitle":"ぼくと先生の乳淫せいかつ","aliases":["ぼくと先生の乳淫生活","nyuuin","bokunyuu","ぼく乳"]}),"Boku Sensei Crak"),
            (json!({"id":"v21529","title":"Motto! Haramase! Honoo no Oppai Isekai Ero Mahou Gakuen!","alttitle":"もっと！孕ませ！炎のおっぱい異世界エロ魔法学園！"}),"全CG存档"),
            (json!({"id":"v19974","title":"Honoguraki Toki no Hate Yori","alttitle":"仄暗き時の果てより"}),"仄暗き時の果てより 中文"),
            (json!({"id":"v26410","title":"Dansei o Iyashi Nuku Kikai","alttitle":"男性を癒やし抜く機械","developers":[{"name":"Yoru no Hitsuji","original":"夜のひつじ"}]}),"cg[190829][夜のひつじ]男性を癒やし抜く機械"),
        ]{
            assert!(matching::corroborated_member(member,&candidate,&[]),"{member}");
            let title=candidate["alttitle"].as_str().unwrap();let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![candidate.clone()]);
            for name in [member.to_string(),matching::clean_name(member)]{
                assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,name]})),None,"{member}");
            }
        }
        let candidate=json!({"id":"v19974","title":"Honoguraki Toki no Hate Yori","alttitle":"仄暗き時の果てより"});
        for word in ["全CG存档","存档","存檔","CG","中文","中文版","汉化","漢化","补丁","補丁"]{
            for member in [word.to_string(),format!("仄暗き時の果てより {word}")]{assert!(matching::corroborated_member(&member,&candidate,&[]),"{member}");}
        }
        // All suffix spellings stay local to this rule, including versions and separators.
        let candidate=json!({"id":"v31700","title":"Hoshi no Otome to Rikka no Shimai","aliases":["Hoshioto"]});
        for suffix in ["crack","crk","crak","patch","new","stage2","1.0","v1.0.0","win","wp","4koma","cg","save","dlc"]{
            assert!(matching::corroborated_member(&format!("Hoshioto_{suffix}"),&candidate,&[]),"{suffix}");
        }
        let latin=json!({"id":"v27365","titles":[{"title":"まおかつ！ ～魔王と勇者のアイドル生活～","latin":"Maokatsu! ~Maou to Yuusha no Idol Seikatsu~"}]});
        assert!(matching::corroborated_member("Maokatsu Crk",&latin,&[]));
    }
    #[test]fn uncorroborated_codes_and_chinese_titles_stay_review(){
        for (candidate,members) in [
            (json!({"id":"v24279","title":"Sekai no Hajimari Gokko","alttitle":"世界の始まりゴッコ"}),vec!["worldbegin_v1.0.0_win","世界の始まりゴッコ worldbegin_v1.0.0_win"]),
            (json!({"id":"v18852","title":"Amatsutsumi","alttitle":"アマツツミ"}),vec!["Boku Sensei Crak","Song of Saya"]),
            (json!({"id":"v28131","title":"Boku to Sensei no Nyuuin Seikatsu","alttitle":"ぼくと先生の乳淫せいかつ","aliases":["nyuuin","bokunyuu"]}),vec!["Sensei Crk","Bok Crk","Sensei Boku Crk","Boku Sens Crk","Boku to Crk","Boku Sensei mystery","Boku new Sensei Crk"]),
            (json!({"id":"v10569","title":"Kodoku ni Kiku Yuri","alttitle":"孤独に効く百合","aliases":["こどゆり","kodoyuri"]}),vec!["PSP 孤独少女的百合物语","PSP 孤独少女的百合物语 汉化补丁"]),
            (json!({"id":"v22436","title":"Harem Futago Lolita","alttitle":"ハーレム双子ロリータ","aliases":["The Harem of Lolita Twins","lol4"]}),vec!["双子洛丽塔后宫KRKR版"]),
            (json!({"id":"v10568","title":"Koi Tsuma Kuzushi","alttitle":"恋妻くずし","aliases":["Love Wife Demolish"]}),vec!["cg[脸肿汉化组]恋妻くずし 汉化补丁"]),
            (json!({"id":"v19974","title":"Honoguraki Toki no Hate Yori","alttitle":"仄暗き時の果てより","titles":[{"title":"来自昏暗的时间尽头","latin":"Laizi Hun'an de Shijian Jintou"}]}),vec!["来自昏暗的时间尽头 中文","仄暗き時の果てより 中文2","仄暗き時の果てより+ 中文"]),
        ]{
            let title=candidate["alttitle"].as_str().unwrap();let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![candidate.clone()]);
            for member in members{
                assert!(!matching::corroborated_member(member,&candidate,&[]),"{member}");
                // The old ASCII-only code path already admits bare worldbegin; do not veto it.
                if member=="worldbegin_v1.0.0_win"{assert!(matching::ancillary_member(member,&[title]));continue;}
                assert_eq!(review_reason(&json!({"results":ranked,"titles":[title,member]})),Some(format!("A different source name needs review: {member}")));
            }
        }
    }
    #[test]fn code_corroboration_cannot_come_from_folders_or_other_candidates(){
        let own=json!({"id":"v27365","title":"Maokatsu! ~Maou to Yuusha no Idol Seikatsu~","alttitle":"まおかつ！ ～魔王と勇者のアイドル生活～"});
        let other=json!({"id":"v31700","title":"Hoshi no Otome to Rikka no Shimai","alttitle":"星の乙女と六華の姉妹","aliases":["Hoshioto"]});
        assert!(!matching::corroborated_member("hoshioto_new stage2",&own,&[other.clone()]));
        // Collisions are synthetic additions to real VNDB candidates; only this new path is vetoed.
        for key in ["aliases","title","titles"]{
            let mut collision=other.clone();collision[key]=match key{"aliases"=>json!(["Maokatsu"]),"titles"=>json!([{"title":"Maokatsu"}]),_=>json!("Maokatsu")};
            assert!(!matching::corroborated_member("Maokatsu Crk",&own,&[collision.clone()]));
            let ranked=matching::rank(&matching::prepare(own["alttitle"].as_str().unwrap(),&[]).unwrap(),vec![own.clone(),collision]);
            assert_eq!(review_reason(&json!({"results":ranked,"titles":[own["alttitle"],"Maokatsu Crk"]})),Some("A different source name needs review: Maokatsu Crk".into()));
        }
        assert!(matching::corroborated_member("Maokatsu Crk",&own,&[own.clone()]));
        let ranked=matching::rank(&matching::prepare(own["alttitle"].as_str().unwrap(),&[]).unwrap(),vec![own.clone(),other]);
        assert_eq!(review_reason(&json!({"results":ranked,"titles":["hoshioto",own["alttitle"],"hoshioto_new stage2"]})),Some("A different source name needs review: hoshioto".into()));
        // Generic-only and code labels never supply the resource's own identity.
        for folder in ["全CG存档","Maokatsu Crk"]{assert!(review_reason(&json!({"results":ranked,"titles":[folder,own["alttitle"]]})).is_some());}
        let short=json!({"id":"v12849","title":"Ao no Kanata no Four Rhythm","aliases":["Ao"]});
        assert!(!matching::corroborated_member("Ao Crk",&short,&[]));
    }
    fn assert_member_review(title:&str,member:&str){
        assert!(!matching::title_anchored_member(member,&[title],&[]),"{member}");
        let ranked=matching::rank(&matching::prepare(title,&[]).unwrap(),vec![json!({"id":"v1","title":title})]);
        for name in [member.to_string(),matching::clean_name(member)]{
            let r=json!({"results":ranked,"titles":[title,name],"incomplete":false});
            assert_eq!(review_reason(&r),Some(format!("A different source name needs review: {name}")),"{member}");
        }
    }
    #[tokio::test]async fn resource_without_a_usable_title_is_left_for_review(){
        let(_t,db)=fixture(&["","Clannad"]);let job=kick(&db,true).unwrap().unwrap();
        run_with(&db,&job,|i|async move{if i.queries.is_empty()&&i.direct_id.is_none(){Ok(json!({"results":[],"titles":[],"incomplete":false,"needs_title":true}))}else{Ok(exact(i))}}).await.unwrap();
        let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
        assert_eq!(c.query_row("SELECT state FROM match_items WHERE resource_id='r0'",[],|r|r.get::<_,String>(0)).unwrap(),"review");
        assert_eq!(c.query_row("SELECT state FROM match_items WHERE resource_id='r1'",[],|r|r.get::<_,String>(0)).unwrap(),"matched");
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
        let compilation=candidate["match"]["compilation"].is_object();let mut bindings=Vec::new();
        for candidate in bound_candidates(response){
        let external=candidate["id"].as_str().ok_or("Candidate has no VNDB ID")?;
        // A remake filed by VNDB as a release of the original work gets its own card for that release.
        let remake=if compilation{None}else{matching::remake_release(&expected.input,candidate)};let release_key=remake.as_ref().and_then(|r|r["id"].as_str()).unwrap_or("").to_string();
        let existing:Option<String>=sql(tx.query_row("SELECT id FROM works WHERE vndb_id=?1 AND coalesce(vndb_release,'')=?2",params![external,release_key],|r|r.get(0)).optional())?;
        let mut wid=existing.clone().unwrap_or_else(db::id);
        if existing.is_some(){let mut seen=std::collections::HashSet::new();loop{if !seen.insert(wid.clone()){return Err("Work merge history contains a cycle".into());}let merged:Option<String>=sql(tx.query_row("SELECT merged_into FROM works WHERE id=?1",[&wid],|r|r.get(0)))?;if let Some(next)=merged{wid=next;}else{break;}}}
        else{
            let title=candidate["title"].as_str().filter(|s|!s.is_empty()).ok_or("Candidate has no title")?;
            let source=json!({"title":title,"original_title":candidate["alttitle"].as_str().unwrap_or(title),"description":candidate["description"].as_str().unwrap_or(""),"cover":candidate["image"]["url"].as_str().unwrap_or(""),"developer":candidate["developers"].as_array().map(|a|a.iter().filter_map(|d|d["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),"released":candidate["released"].as_str().unwrap_or(""),"tags":candidate["tags"].as_array().map(|a|a.iter().filter_map(|d|d["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),"aliases":candidate["aliases"].as_array().cloned().unwrap_or_default()});
            let mut source=source;if let Some(release)=&remake{for (k,v) in matching::release_card(candidate,release).as_object().into_iter().flatten(){source[k]=v.clone();}}
            sql(tx.execute("INSERT INTO works(id,title,original_title,vndb_id,description,cover,developer,released,tags,aliases,source_json,vndb_release) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![wid,source["title"].as_str(),source["original_title"].as_str(),external,source["description"].as_str(),source["cover"].as_str(),source["developer"].as_str(),source["released"].as_str(),source["tags"].to_string(),source["aliases"].to_string(),source.to_string(),remake.as_ref().and_then(|r|r["id"].as_str())]))?;
        }
        if existing.is_none(){sql(crate::job_summary::record(&tx,job,"new_works",&wid,"yes"))?;}
        let release=if compilation{None}else{editions::legacy_release(&tx,&wid,"Unclassified edition")?};
        // Bonus archives split out of a bundle folder are extras of their work, not the game itself.
        let path:String=sql(tx.query_row("SELECT relative_path FROM resources WHERE id=?1",[rid],|r|r.get(0)))?;
        let role=if compilation{"main"}else if path.contains(['/','\\'])&&matching::bonus_name(&path){"extra"}else{"unknown"};
        if !bindings.iter().any(|b:&editions::Binding|b.work_id==wid){bindings.push(editions::Binding{work_id:wid,release_id:release,role:role.into()});}
        }
        let primary=bindings.first().map(|b|b.work_id.clone());
        // Same binding/history primitive as manual_bindings_in, without a person's timeline event.
        editions::set_bindings_in(&tx,rid,editions::EditBindings{revision:expected.revision,primary_work_id:primary.clone(),bindings})?;
        complete_item(&tx,job,rid,"matched",response,primary.as_deref())?;
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


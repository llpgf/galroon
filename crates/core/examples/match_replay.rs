//! Offline, read-only replay of the latest stored candidates for every resource.
use galroon_core::{automatch,matching};
use rusqlite::{Connection,OpenFlags};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{collections::BTreeMap,path::{Path,PathBuf}};
type R<T>=Result<T,Box<dyn std::error::Error>>;
const LIMITATION:&str="Replay only sees candidates stored in the earlier run. Changes to VNDB queries require a live run to confirm.";
const USAGE:&str="match_replay <library.sqlite> --out <report.json> [--baseline <report.json>] [--answers <answers.json>]";
#[derive(Deserialize,Serialize)]
struct Stored{state:Option<String>,work_id:Option<String>,vndb_id:Option<String>}
#[derive(Deserialize,Serialize)]
struct Row{resource_id:String,relative_path:String,vndb_id:Option<String>,strength:Option<String>,ambiguous:bool,reason:Option<String>,review_reason:Option<String>,stored:Stored}
impl Row{fn strong(&self)->bool{self.vndb_id.is_some()&&self.strength.as_deref()==Some("strong")&&!self.ambiguous&&self.review_reason.is_none()}}
#[derive(Deserialize,Serialize)]
struct Report{algorithm_version:i64,limitation:String,summary:Value,diff:Value,answers:Value,resources:Vec<Row>}
#[derive(Deserialize)]
struct Answer{vndb:String,source:AnswerSource}
#[derive(Deserialize)]
enum AnswerSource{#[serde(rename="user")]User,#[serde(rename="auto-2026-10-11")]Automatic}
struct Args{db:PathBuf,out:PathBuf,baseline:Option<PathBuf>,answers:Option<PathBuf>}
fn args()->R<Args>{
    let mut args=std::env::args_os().skip(1);let db=args.next().ok_or(USAGE)?;let mut options=BTreeMap::new();
    while let Some(key)=args.next(){let key=key.to_str().ok_or(USAGE)?.to_string();if !["--out","--baseline","--answers"].contains(&key.as_str()){return Err(USAGE.into());}let value=args.next().ok_or(USAGE)?;if options.insert(key,value).is_some(){return Err("Duplicate option".into());}}
    Ok(Args{db:db.into(),out:options.remove("--out").ok_or(USAGE)?.into(),baseline:options.remove("--baseline").map(Into::into),answers:options.remove("--answers").map(Into::into)})
}
fn read<T:serde::de::DeserializeOwned>(path:&Path)->R<T>{Ok(serde_json::from_slice(&std::fs::read(path)?)?)}
fn rows(c:&Connection)->R<Vec<Row>>{
    let mut s=c.prepare("SELECT r.id,r.relative_path,m.evidence,m.state,m.work_id,w.vndb_id FROM resources r LEFT JOIN match_items m ON m.rowid=(SELECT mi.rowid FROM match_items mi JOIN jobs j ON j.id=mi.job_id WHERE mi.resource_id=r.id ORDER BY j.rowid DESC LIMIT 1) LEFT JOIN works w ON w.id=m.work_id ORDER BY r.relative_path,r.id")?;
    let mut cursor=s.query([])?;let mut rows=vec![];
    while let Some(r)=cursor.next()?{
        let id:String=r.get(0)?;let evidence:Option<String>=r.get(2)?;let evidence:Value=serde_json::from_str(evidence.as_deref().unwrap_or("{}"))?;
        let replay=automatch::replay(c,&id,&evidence).map_err(|e|format!("Resource {id}: {e}"))?;
        let (candidate,review_reason)=if let Some(result)=replay{let reason=result["decision"]["reason"].as_str().map(str::to_owned);(result["results"][0].clone(),reason)}else{(Value::Null,Some("Resource has no usable catalog input (no indexed files or a system entry).".into()))};
        rows.push(Row{resource_id:id,relative_path:r.get(1)?,vndb_id:candidate["id"].as_str().map(str::to_owned),strength:candidate["match"]["strength"].as_str().map(str::to_owned),ambiguous:candidate["match"]["ambiguous"]==true,reason:candidate["match"]["reason"].as_str().map(str::to_owned),review_reason,stored:Stored{state:r.get(3)?,work_id:r.get(4)?,vndb_id:r.get(5)?}});
    }Ok(rows)
}
fn diff(rows:&[Row],baseline:&Report)->R<Value>{
    let old:BTreeMap<_,_>=baseline.resources.iter().map(|r|(r.resource_id.as_str(),r)).collect();
    if old.len()!=baseline.resources.len()||old.len()!=rows.len()||rows.iter().any(|r|old.get(r.resource_id.as_str()).is_none_or(|b|b.relative_path!=r.relative_path)){return Err("Baseline must contain the same resource ids and relative paths".into());}
    let(mut gained,mut lost,mut changed,mut reason_changed)=(vec![],vec![],vec![],vec![]);
    for row in rows{let before=old[row.resource_id.as_str()];let entry=json!({"resource_id":row.resource_id,"relative_path":row.relative_path,"before":before,"after":row});
        if !before.strong()&&row.strong(){gained.push(entry);}else if before.strong()&&!row.strong(){lost.push(entry);}else if before.strong()&&row.strong()&&before.vndb_id!=row.vndb_id{changed.push(entry);}else if before.reason!=row.reason||before.review_reason!=row.review_reason{reason_changed.push(entry);}
    }
    Ok(json!({"counts":{"gained":gained.len(),"lost":lost.len(),"changed":changed.len(),"reason_changed":reason_changed.len()},"gained":gained,"lost":lost,"changed":changed,"reason_changed":reason_changed}))
}
fn check_answers(rows:&[Row],answers:&BTreeMap<String,Answer>)->R<Value>{
    let mut paths=BTreeMap::new();for row in rows{if paths.insert(row.relative_path.as_str(),row).is_some(){return Err("Answer keys require unique relative paths across roots".into());}}
    let(mut agree,mut false_matches,mut missing)=(0,vec![],vec![]);
    for (path,answer) in answers{
        let _=&answer.source;let id=answer.vndb.strip_prefix('v').ok_or("Answer must contain a VNDB vNNN id")?;
        if id.is_empty()||!id.bytes().all(|c|c.is_ascii_digit()){return Err("Answer must contain a VNDB vNNN id".into());}
        match paths.get(path.as_str()){
            Some(row) if row.strong()=>if row.vndb_id.as_deref()==Some(&answer.vndb){agree+=1;}else{false_matches.push(json!({"relative_path":path,"expected":answer.vndb,"actual":row.vndb_id,"resource_id":row.resource_id}));},
            row=>missing.push(json!({"relative_path":path,"expected":answer.vndb,"resource_id":row.map(|r|&r.resource_id)})),
        }
    }
    Ok(json!({"counts":{"agree":agree,"false":false_matches.len(),"missing":missing.len()},"false":false_matches,"missing":missing}))
}
fn print_diff(diff:&Value){
    for (key,label) in [("gained","gained"),("lost","lost"),("changed","changed (FALSE-MATCH ALARM)"),("reason_changed","reason changed")]{
        println!("{label}: {}",diff["counts"][key]);
        for entry in diff[key].as_array().into_iter().flatten(){println!("  {}: {} -> {} | {} -> {}",entry["relative_path"].as_str().unwrap_or(""),entry["before"]["vndb_id"],entry["after"]["vndb_id"],json!({"reason":entry["before"]["reason"],"review":entry["before"]["review_reason"]}),json!({"reason":entry["after"]["reason"],"review":entry["after"]["review_reason"]}));}
    }
}
fn run()->R<bool>{
    let args=args()?;
    // Refuse to overwrite any input, including a symlink to the read-only catalog.
    if let Ok(out)=args.out.canonicalize(){for input in std::iter::once(&args.db).chain(args.baseline.iter()).chain(args.answers.iter()){if input.canonicalize()?==out{return Err("Output must be separate from every input".into());}}}
    let baseline:Option<Report>=args.baseline.as_deref().map(read).transpose()?;let answers:Option<BTreeMap<String,Answer>>=args.answers.as_deref().map(read).transpose()?;
    let c=Connection::open_with_flags(&args.db,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // Keep all evidence and manual precedents in one consistent read-only snapshot.
    c.execute_batch("BEGIN")?;let resources=rows(&c)?;c.execute_batch("ROLLBACK")?;
    let strong=resources.iter().filter(|r|r.strong()).count();let summary=json!({"resources":resources.len(),"strong":strong,"review":resources.len()-strong});
    let diff=baseline.as_ref().map(|b|diff(&resources,b)).transpose()?.unwrap_or(Value::Null);let answers=answers.as_ref().map(|a|check_answers(&resources,a)).transpose()?.unwrap_or(Value::Null);
    let failed=diff["counts"]["changed"].as_u64().unwrap_or(0)>0||answers["counts"]["false"].as_u64().unwrap_or(0)>0;
    let report=Report{algorithm_version:matching::ALGORITHM_VERSION,limitation:LIMITATION.into(),summary,diff,answers,resources};
    // Atomic replacement also avoids following an output hard link into a catalog.
    let mut output=tempfile::NamedTempFile::new_in(args.out.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")))?;
    serde_json::to_writer_pretty(&mut output,&report)?;output.persist(&args.out)?;
    println!("Algorithm {}. {LIMITATION}\n{}",report.algorithm_version,report.summary);
    if !report.diff.is_null(){print_diff(&report.diff);}if !report.answers.is_null(){println!("answers: {}",report.answers["counts"]);}
    Ok(failed)
}
fn main(){match run(){Ok(false)=>(),Ok(true)=>std::process::exit(1),Err(e)=>{eprintln!("{e}");std::process::exit(2);}}}

#[cfg(test)]mod tests{
    use super::*;
    fn row(id:&str,strong:bool)->Row{Row{resource_id:id.into(),relative_path:id.into(),vndb_id:Some("v4".into()),strength:Some("strong".into()),ambiguous:false,reason:Some("exact_title".into()),review_reason:if strong{None}else{Some("Conflicting member".into())},stored:Stored{state:None,work_id:None,vndb_id:None}}}
    #[test]fn compares_final_admission_and_checks_all_answers(){
        let baseline=Report{algorithm_version:5,limitation:LIMITATION.into(),summary:Value::Null,diff:Value::Null,answers:Value::Null,resources:vec![row("gained",false),row("lost",true),row("changed",true),row("reason",false)]};
        let mut changed=row("changed",true);changed.vndb_id=Some("v5".into());let mut reason=row("reason",false);reason.review_reason=Some("Another reason".into());
        let rows=vec![row("gained",true),row("lost",false),changed,reason];let result=diff(&rows,&baseline).unwrap();assert_eq!(result["counts"],json!({"gained":1,"lost":1,"changed":1,"reason_changed":1}));
        let answers=serde_json::from_value(json!({"gained":{"vndb":"v4","source":"user"},"changed":{"vndb":"v4","source":"auto-2026-10-11"},"lost":{"vndb":"v4","source":"user"},"absent":{"vndb":"v4","source":"user"}})).unwrap();
        assert_eq!(check_answers(&rows,&answers).unwrap()["counts"],json!({"agree":1,"false":1,"missing":2}));assert!(diff(&rows[..2],&baseline).is_err());
    }
    #[test]fn reads_latest_job_even_when_inserted_in_reverse_order_and_keeps_unsearched_resources(){
        let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=galroon_core::db::open(&path).unwrap();
        {let c=db.lock().unwrap();c.execute_batch("INSERT INTO roots(id,path,label) VALUES('root','/unused','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('r0','root','Clannad','Clannad','archive'),('r1','root','Unsearched','Unsearched','archive'),('r2','root','Empty','Empty','archive'); INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES('f0','root','/unused/Clannad','Clannad',1,'1','rar','fixture'),('f1','root','/unused/Unsearched','Unsearched',1,'1','rar','fixture'); INSERT INTO resource_files(resource_id,file_id) VALUES('r0','f0'),('r1','f1'); INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('old','match','completed','{}',1,100),('new','match','completed','{}',1,1); INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Clannad','Clannad','v4');").unwrap();
            let evidence=json!({"results":[{"id":"v4","title":"Clannad"}],"incomplete":false}).to_string();
            c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,evidence,work_id) VALUES('new','r0',1,'matched',?1,'w')",[evidence]).unwrap();
            c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,evidence) VALUES('old','r0',1,'review','{}')",[]).unwrap();}
        drop(db);let c=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();let rows=rows(&c).unwrap();assert_eq!(rows.len(),3);
        let matched=rows.iter().find(|r|r.resource_id=="r0").unwrap();assert!(matched.strong());assert_eq!(matched.stored.state.as_deref(),Some("matched"));assert_eq!(matched.stored.vndb_id.as_deref(),Some("v4"));
        assert_eq!(rows.iter().find(|r|r.resource_id=="r1").unwrap().review_reason.as_deref(),Some("Not checked against VNDB yet."));assert!(!rows.iter().find(|r|r.resource_id=="r2").unwrap().strong());
    }
}

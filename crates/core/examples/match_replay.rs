//! Offline, read-only replay of the latest stored candidates for every resource.
use galroon_core::{automatch,matching};
use rusqlite::{Connection,OpenFlags};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},ffi::OsString,path::{Path,PathBuf}};
use unicode_normalization::UnicodeNormalization;
type R<T>=Result<T,Box<dyn std::error::Error>>;
const LIMITATION:&str="Replay only sees candidates stored in the earlier run. Changes to VNDB queries require a live run to confirm.";
const USAGE:&str="match_replay <library.sqlite> --out <report.json> [--baseline <report.json>] [--answers <answers.json>] [--root-map <old>=<new>]... [--fail-on changed,false,lost,missing,added,removed]";
#[derive(Clone,Deserialize,Serialize)]
struct Stored{state:Option<String>,work_id:Option<String>,vndb_id:Option<String>}
#[derive(Clone,Deserialize,Serialize)]
struct Row{resource_id:String,#[serde(default)]root_path:String,relative_path:String,vndb_id:Option<String>,strength:Option<String>,ambiguous:bool,reason:Option<String>,review_reason:Option<String>,stored:Stored}
impl Row{
    fn strong(&self)->bool{self.vndb_id.is_some()&&self.strength.as_deref()==Some("strong")&&!self.ambiguous&&self.review_reason.is_none()}
    fn identity(&self,section:&str)->Option<&str>{if section=="actual"{self.stored.vndb_id.as_deref().filter(|_|self.stored.state.as_deref()==Some("matched"))}else{self.vndb_id.as_deref().filter(|_|self.strong())}}
}
#[derive(Deserialize,Serialize)]
struct Report{algorithm_version:i64,limitation:String,summary:Value,diff:Value,answers:Value,resources:Vec<Row>,#[serde(default,skip_serializing_if="Option::is_none")]error:Option<String>}
impl Report{fn new()->Self{Self{algorithm_version:matching::ALGORITHM_VERSION,limitation:LIMITATION.into(),summary:Value::Null,diff:Value::Null,answers:Value::Null,resources:vec![],error:None}}}
#[derive(Deserialize)]
struct Answer{vndb:String,source:AnswerSource}
#[derive(Deserialize)]
enum AnswerSource{#[serde(rename="user")]User,#[serde(rename="auto-2026-10-11")]Automatic}
struct Args{db:PathBuf,out:PathBuf,baseline:Option<PathBuf>,answers:Option<PathBuf>,root_maps:Vec<(String,String)>,fail_on:BTreeSet<String>}
fn fail_on(value:&str)->R<BTreeSet<String>>{
    let mut result=BTreeSet::new();for key in value.split(','){if !["changed","false","lost","missing","added","removed"].contains(&key){return Err(format!("Invalid --fail-on category: {key:?}").into());}result.insert(key.into());}Ok(result)
}
fn args(raw:&[OsString])->R<Args>{
    let mut args=raw.iter();let db=args.next().ok_or(USAGE)?;let mut options=BTreeMap::new();let mut root_maps=vec![];
    while let Some(key)=args.next(){let key=key.to_str().ok_or(USAGE)?.to_string();if !["--out","--baseline","--answers","--root-map","--fail-on"].contains(&key.as_str()){return Err(USAGE.into());}let value=args.next().ok_or(USAGE)?;
        if key=="--root-map"{let (old,new)=value.to_str().and_then(|v|v.split_once('=')).ok_or("--root-map requires <old>=<new>")?;if old.is_empty()||new.is_empty()||root_maps.iter().any(|(p,_)|p==old){return Err("Root-map prefixes must be nonempty and unique".into());}root_maps.push((old.to_string(),new.to_string()));}
        else if options.insert(key,value).is_some(){return Err("Duplicate option".into());}
    }
    Ok(Args{db:db.into(),out:options.remove("--out").ok_or(USAGE)?.into(),baseline:options.remove("--baseline").map(Into::into),answers:options.remove("--answers").map(Into::into),root_maps,fail_on:fail_on(options.remove("--fail-on").map(|v|v.to_str().ok_or(USAGE)).transpose()?.unwrap_or("changed,false"))?})
}
fn read<T:serde::de::DeserializeOwned>(path:&Path)->R<T>{Ok(serde_json::from_slice(&std::fs::read(path)?)?)}
fn rows(c:&Connection)->R<Vec<Row>>{
    let mut s=c.prepare("SELECT r.id,r.relative_path,m.evidence,m.state,m.work_id,w.vndb_id,roots.path FROM resources r JOIN roots ON roots.id=r.root_id LEFT JOIN match_items m ON m.rowid=(SELECT mi.rowid FROM match_items mi JOIN jobs j ON j.id=mi.job_id WHERE mi.resource_id=r.id ORDER BY j.rowid DESC LIMIT 1) LEFT JOIN works w ON w.id=m.work_id ORDER BY r.relative_path,r.id")?;
    let mut cursor=s.query([])?;let mut rows=vec![];
    while let Some(r)=cursor.next()?{
        let id:String=r.get(0)?;let evidence:Option<String>=r.get(2)?;let evidence:Value=serde_json::from_str(evidence.as_deref().unwrap_or("{}"))?;
        let replay=automatch::replay(c,&id,&evidence).map_err(|e|format!("Resource {id}: {e}"))?;
        let (candidate,review_reason)=if let Some(result)=replay{let reason=result["decision"]["reason"].as_str().map(str::to_owned);(result["results"][0].clone(),reason)}else{(Value::Null,Some("Resource has no usable catalog input (no indexed files or a system entry).".into()))};
        rows.push(Row{resource_id:id,root_path:r.get(6)?,relative_path:r.get(1)?,vndb_id:candidate["id"].as_str().map(str::to_owned),strength:candidate["match"]["strength"].as_str().map(str::to_owned),ambiguous:candidate["match"]["ambiguous"]==true,reason:candidate["match"]["reason"].as_str().map(str::to_owned),review_reason,stored:Stored{state:r.get(3)?,work_id:r.get(4)?,vndb_id:r.get(5)?}});
    }Ok(rows)
}
fn path_key(row:&Row,maps:&[(String,String)])->String{
    // Map the raw baseline root once, then normalize the entire key (including the relative path).
    let root=maps.iter().filter(|(old,_)|row.root_path.starts_with(old)).max_by_key(|(old,_)|old.len()).map(|(old,new)|format!("{new}{}",&row.root_path[old.len()..])).unwrap_or_else(||row.root_path.clone());
    format!("{}/{}",root.trim_end_matches('/'),row.relative_path).nfc().collect()
}
fn path_index<'a>(rows:&'a [Row],maps:&[(String,String)],side:&str)->R<BTreeMap<String,&'a Row>>{
    let mut groups:BTreeMap<String,Vec<&Row>>=BTreeMap::new();for row in rows{if row.root_path.is_empty(){return Err(format!("{side}: root_path is required for path comparison (resource {})",row.resource_id).into());}groups.entry(path_key(row,maps)).or_default().push(row);}
    let collisions:Vec<_>=groups.iter().filter(|(_,v)|v.len()>1).map(|(key,v)|format!("{key:?}: {}",v.iter().map(|r|format!("{}/{} (id {})",r.root_path,r.relative_path,r.resource_id)).collect::<Vec<_>>().join(", "))).collect();
    if !collisions.is_empty(){return Err(format!("Duplicate path keys in {side}:\n{}",collisions.join("\n")).into());}Ok(groups.into_iter().map(|(k,v)|(k,v[0])).collect())
}
fn diff(rows:&[Row],baseline:&Report,maps:&[(String,String)])->R<Value>{
    if baseline.error.is_some(){return Err("Cannot compare an error report".into());}
    let old_ids:BTreeMap<_,_>=baseline.resources.iter().map(|r|(r.resource_id.as_str(),r)).collect();let new_ids:BTreeMap<_,_>=rows.iter().map(|r|(r.resource_id.as_str(),r)).collect();
    if old_ids.len()!=baseline.resources.len()||new_ids.len()!=rows.len(){return Err("Duplicate resource ids in report".into());}
    let id_mode=old_ids.len()==new_ids.len()&&rows.iter().all(|r|old_ids.get(r.resource_id.as_str()).is_some_and(|b|b.relative_path==r.relative_path));
    // Older reports without roots remain usable in ID mode. New reports reject path collisions even in ID mode.
    let new_paths=if !id_mode||rows.iter().all(|r|!r.root_path.is_empty()){Some(path_index(rows,&[],"new report")?)}else{None};
    let old_paths=if !id_mode||baseline.resources.iter().all(|r|!r.root_path.is_empty()){Some(path_index(&baseline.resources,maps,"baseline")?)}else{None};
    let (new,old)=if id_mode{(new_ids.into_iter().map(|(k,v)|(k.to_string(),v)).collect(),old_ids.into_iter().map(|(k,v)|(k.to_string(),v)).collect())}else{(new_paths.unwrap(),old_paths.unwrap())};
    let added:Vec<_>=new.iter().filter(|(k,_)|!old.contains_key(*k)).map(|(_,r)|*r).collect();let removed:Vec<_>=old.iter().filter(|(k,_)|!new.contains_key(*k)).map(|(_,r)|*r).collect();
    let mut result=json!({"mode":if id_mode{"id"}else{"path"},"added":added,"removed":removed,"counts":{"added":added.len(),"removed":removed.len()}});
    for section in ["actual","replay"]{
        let(mut gained,mut lost,mut changed,mut reason_changed)=(vec![],vec![],vec![],vec![]);
        for (key,row) in &new{let Some(before)=old.get(key)else{continue;};let entry=json!({"resource_id":row.resource_id,"root_path":row.root_path,"relative_path":row.relative_path,"before":before,"after":row});
            match (before.identity(section),row.identity(section)){
                (None,Some(_))=>gained.push(entry),(Some(_),None)=>lost.push(entry),(Some(a),Some(b)) if a!=b=>changed.push(entry),
                _=>if section=="replay"&&(before.reason!=row.reason||before.review_reason!=row.review_reason){reason_changed.push(entry);},
            }
        }
        result[section]=json!({"counts":{"gained":gained.len(),"lost":lost.len(),"changed":changed.len()},"gained":gained,"lost":lost,"changed":changed});
        if section=="replay"{result[section]["counts"]["reason_changed"]=json!(reason_changed.len());result[section]["reason_changed"]=json!(reason_changed);}
    }Ok(result)
}
fn check_answers(rows:&[Row],answers:&BTreeMap<String,Answer>)->R<Value>{
    let mut paths=BTreeMap::new();for row in rows{let path:String=row.relative_path.nfc().collect();if let Some(before)=paths.insert(path,row){return Err(format!("Answer keys require unique NFC relative paths across roots: {}/{} and {}/{}",before.root_path,before.relative_path,row.root_path,row.relative_path).into());}}
    let mut normalized=BTreeMap::new();for (path,answer) in answers{
        let _=&answer.source;let id=answer.vndb.strip_prefix('v').ok_or("Answer must contain a VNDB vNNN id")?;
        if id.is_empty()||!id.bytes().all(|c|c.is_ascii_digit()){return Err("Answer must contain a VNDB vNNN id".into());}
        if normalized.insert(path.nfc().collect::<String>(),answer).is_some(){return Err(format!("Duplicate NFC answer key: {path}").into());}
    }
    let mut result=json!({});for section in ["actual","replay"]{
        let(mut agree,mut false_matches,mut missing)=(0,vec![],vec![]);
        for (path,answer) in &normalized{let row=paths.get(path);match row.and_then(|r|r.identity(section)){
            Some(id)=>if id==answer.vndb{agree+=1;}else{false_matches.push(json!({"relative_path":path,"expected":answer.vndb,"actual":id,"resource_id":row.unwrap().resource_id,"root_path":row.unwrap().root_path}));},
            None=>missing.push(json!({"relative_path":path,"expected":answer.vndb,"resource_id":row.map(|r|&r.resource_id)})),
        }}
        let unreviewed:Vec<_>=rows.iter().filter(|r|r.identity(section).is_some()&&!normalized.contains_key(&r.relative_path.nfc().collect::<String>())).collect();
        result[section]=json!({"counts":{"agree":agree,"false":false_matches.len(),"missing":missing.len(),"unreviewed":unreviewed.len()},"false":false_matches,"missing":missing,"unreviewed":unreviewed});
    }Ok(result)
}
fn summarize(report:&Report)->Value{
    let mut summary=json!({"resources":report.resources.len(),"added":report.diff["counts"]["added"].as_u64().unwrap_or(0),"removed":report.diff["counts"]["removed"].as_u64().unwrap_or(0)});
    for section in ["actual","replay"]{let count=report.resources.iter().filter(|r|r.identity(section).is_some()).count();let mut counts=json!({"review":report.resources.len()-count});counts[if section=="actual"{"matched"}else{"strong"}]=json!(count);
        for key in ["gained","lost","changed"]{counts[key]=json!(report.diff[section]["counts"][key].as_u64().unwrap_or(0));}
        for key in ["false","agree","missing","unreviewed"]{counts[key]=json!(report.answers[section]["counts"][key].as_u64().unwrap_or(0));}
        if section=="replay"{counts["reason_changed"]=json!(report.diff[section]["counts"]["reason_changed"].as_u64().unwrap_or(0));}summary[section]=counts;
    }summary
}
fn exit_code(report:&Report,fail_on:&BTreeSet<String>)->u8{
    if report.error.is_some(){2}else if fail_on.iter().any(|key|report.summary[key].as_u64().unwrap_or(0)>0||["actual","replay"].iter().any(|section|report.summary[section][key].as_u64().unwrap_or(0)>0)){1}else{0}
}
fn populate(args:&Args,report:&mut Report)->R<()>{
    let baseline:Option<Report>=args.baseline.as_deref().map(read).transpose()?;let answers:Option<BTreeMap<String,Answer>>=args.answers.as_deref().map(read).transpose()?;
    let c=Connection::open_with_flags(&args.db,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // Keep all evidence and manual precedents in one consistent read-only snapshot.
    c.execute_batch("BEGIN")?;report.resources=rows(&c)?;c.execute_batch("ROLLBACK")?;
    path_index(&report.resources,&[],"new report")?;
    report.diff=baseline.as_ref().map(|b|diff(&report.resources,b,&args.root_maps)).transpose()?.unwrap_or(Value::Null);
    report.answers=answers.as_ref().map(|a|check_answers(&report.resources,a)).transpose()?.unwrap_or(Value::Null);
    report.summary=summarize(report);Ok(())
}
fn write_report(out:&Path,report:&Report)->R<()>{
    // Atomic replacement also avoids following an output hard link into a catalog.
    let mut output=tempfile::NamedTempFile::new_in(out.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")))?;
    serde_json::to_writer_pretty(&mut output,report)?;output.persist(out)?;Ok(())
}
fn print_report(report:&Report){
    println!("Algorithm {}. {LIMITATION}\nresources: {}; added: {}; removed: {}",report.algorithm_version,report.summary["resources"],report.summary["added"],report.summary["removed"]);
    for section in ["actual","replay"]{
        println!("{section}: {}",report.summary[section]);
        for key in ["gained","lost","changed","reason_changed"]{for entry in report.diff[section][key].as_array().into_iter().flatten(){
            let before=if section=="actual"{&entry["before"]["stored"]}else{&entry["before"]};let after=if section=="actual"{&entry["after"]["stored"]}else{&entry["after"]};
            println!("  {key}: {}/{}: {} -> {}",entry["root_path"].as_str().unwrap_or(""),entry["relative_path"].as_str().unwrap_or(""),before["vndb_id"],after["vndb_id"]);
        }}
    }
    for key in ["added","removed"]{for entry in report.diff[key].as_array().into_iter().flatten(){println!("{key}: {}/{}",entry["root_path"].as_str().unwrap_or(""),entry["relative_path"].as_str().unwrap_or(""));}}
}
fn run(raw:&[OsString])->R<u8>{
    // Recover --out before validating other options so input/runtime errors also get a report.
    let outputs:Vec<_>=raw.windows(2).filter(|v|v[0]=="--out").map(|v|PathBuf::from(&v[1])).collect();
    if outputs.len()!=1{return Err(USAGE.into());}let out=&outputs[0];
    // Refuse to overwrite any input, including symlinks; this also covers malformed command lines.
    if let Ok(canonical)=out.canonicalize(){for (i,input) in raw.iter().enumerate(){if i>0&&raw[i-1]=="--out"{continue;}if Path::new(input).canonicalize().ok().as_ref()==Some(&canonical){return Err("Output must be separate from every input".into());}}}
    let mut report=Report::new();let mut failures=fail_on("changed,false")?;
    let result=(||->R<()>{let args=args(raw)?;debug_assert_eq!(&args.out,out);failures=args.fail_on.clone();populate(&args,&mut report)})();
    if let Err(e)=result{report.error=Some(e.to_string());}
    write_report(out,&report)?;
    print_report(&report);
    if let Some(error)=&report.error{eprintln!("{error}");}Ok(exit_code(&report,&failures))
}
fn main(){let code=match run(&std::env::args_os().skip(1).collect::<Vec<_>>()){Ok(code)=>code,Err(e)=>{eprintln!("{e}");2}};std::process::exit(code.into());}

#[cfg(test)]mod tests{
    use super::*;
    fn row(id:&str,strong:bool)->Row{Row{resource_id:id.into(),root_path:"/root".into(),relative_path:id.into(),vndb_id:Some("v4".into()),strength:Some("strong".into()),ambiguous:false,reason:Some("exact_title".into()),review_reason:if strong{None}else{Some("Conflicting member".into())},stored:Stored{state:None,work_id:None,vndb_id:None}}}
    #[test]fn compares_final_admission_and_checks_all_answers(){
        let baseline=Report{algorithm_version:5,limitation:LIMITATION.into(),summary:Value::Null,diff:Value::Null,answers:Value::Null,error:None,resources:vec![row("gained",false),row("lost",true),row("changed",true),row("reason",false)]};
        let mut changed=row("changed",true);changed.vndb_id=Some("v5".into());let mut reason=row("reason",false);reason.review_reason=Some("Another reason".into());
        let rows=vec![row("gained",true),row("lost",false),changed,reason];let result=diff(&rows,&baseline,&[]).unwrap();assert_eq!(result["replay"]["counts"],json!({"gained":1,"lost":1,"changed":1,"reason_changed":1}));
        let answers=serde_json::from_value(json!({"gained":{"vndb":"v4","source":"user"},"changed":{"vndb":"v4","source":"auto-2026-10-11"},"lost":{"vndb":"v4","source":"user"},"absent":{"vndb":"v4","source":"user"}})).unwrap();
        assert_eq!(check_answers(&rows,&answers).unwrap()["replay"]["counts"],json!({"agree":1,"false":1,"missing":2,"unreviewed":0}));assert_eq!(diff(&rows[..2],&baseline,&[]).unwrap()["counts"]["removed"],2);
    }
    fn baseline(resources:Vec<Row>)->Report{Report{resources,..Report::new()}}
    fn stored(row:&mut Row,id:Option<&str>){row.stored=Stored{state:Some(if id.is_some(){"matched"}else{"review"}.into()),work_id:id.map(|_|"w".into()),vndb_id:id.map(str::to_owned)};}
    #[test]fn id_mode_preserves_old_reports_and_ignores_root_changes(){
        let mut old=row("r",true);old.root_path.clear();let result=diff(&[row("r",true)],&baseline(vec![old]),&[]).unwrap();
        assert_eq!(result["mode"],"id");assert_eq!(result["replay"]["counts"]["changed"],0);
        let old=row("r",true);let mut new=old.clone();new.root_path="/other".into();assert_eq!(diff(&[new],&baseline(vec![old]),&[]).unwrap()["mode"],"id");
    }
    #[test]fn path_mode_pairs_different_ids_and_lists_added_and_removed(){
        let old=row("old",true);let mut new=old.clone();new.resource_id="new".into();let result=diff(&[new,row("added",true)],&baseline(vec![old,row("removed",true)]),&[]).unwrap();
        assert_eq!(result["mode"],"path");assert_eq!(result["counts"],json!({"added":1,"removed":1}));assert_eq!(result["added"][0]["relative_path"],"added");assert_eq!(result["removed"][0]["relative_path"],"removed");assert_eq!(result["replay"]["counts"]["gained"],0);assert_eq!(result["replay"]["counts"]["lost"],0);
    }
    #[test]fn root_maps_use_longest_raw_prefix_before_nfc(){
        let mut old=row("old",true);old.root_path="/Mac/Cafe\u{301}/Games".into();old.relative_path="カ\u{3099}ーム".into();let mut new=old.clone();new.resource_id="new".into();new.root_path="/NAS/Café/Games".into();new.relative_path="ガーム".into();
        let maps=vec![("/Mac/Cafe\u{301}".into(),"/NAS/Cafe\u{301}".into()),("/Mac".into(),"/wrong".into()),("/NAS".into(),"/not-recursive".into()),("/Mac/Café/Games".into(),"/must-not-match-before-normalization".into())];
        let result=diff(&[new],&baseline(vec![old]),&maps).unwrap();assert_eq!(result["mode"],"path");assert_eq!(result["counts"],json!({"added":0,"removed":0}));
    }
    #[test]fn collisions_list_both_paths_on_either_side_including_after_mapping(){
        let mut a=row("a",true);a.relative_path="é".into();let mut b=row("b",true);b.relative_path="e\u{301}".into();
        let error=diff(&[a.clone(),b.clone()],&baseline(vec![]),&[]).unwrap_err().to_string();assert!(error.contains("new report")&&error.contains("/root/é (id a)")&&error.contains("/root/e\u{301} (id b)"));
        b.root_path="/other".into();let error=diff(&[],&baseline(vec![a,b]),&[("/other".into(),"/root".into())]).unwrap_err().to_string();assert!(error.contains("baseline")&&error.contains("id a")&&error.contains("id b"));
    }
    #[test]fn actual_and_replay_diverge_and_answer_keys_accept_nfc(){
        let mut old=row("r",true);old.relative_path="e\u{301}".into();stored(&mut old,Some("v4"));let mut new=old.clone();stored(&mut new,Some("v5"));new.review_reason=Some("Review".into());
        let result=diff(&[new.clone()],&baseline(vec![old]),&[]).unwrap();assert_eq!(result["actual"]["counts"]["changed"],1);assert_eq!(result["replay"]["counts"]["lost"],1);assert!(result["actual"].get("reason_changed").is_none());
        let answers=serde_json::from_value(json!({"é":{"vndb":"v4","source":"user"}})).unwrap();let checked=check_answers(&[new],&answers).unwrap();assert_eq!(checked["actual"]["counts"]["false"],1);assert_eq!(checked["replay"]["counts"]["missing"],1);
        let mut old=row("r",false);stored(&mut old,Some("v4"));let mut new=row("r",true);stored(&mut new,None);let result=diff(&[new],&baseline(vec![old]),&[]).unwrap();assert_eq!(result["actual"]["counts"]["lost"],1);assert_eq!(result["replay"]["counts"]["gained"],1);
        let mut old=row("r",true);stored(&mut old,None);let mut new=old.clone();stored(&mut new,Some("v4"));let result=diff(&[new],&baseline(vec![old]),&[]).unwrap();assert_eq!(result["actual"]["counts"]["gained"],1);
    }
    #[test]fn unreviewed_lists_only_admitted_matches_without_answers(){
        let mut actual=row("actual",false);stored(&mut actual,Some("v4"));let mut review=row("review",false);stored(&mut review,Some("v4"));review.stored.state=Some("review".into());
        let rows=vec![actual,row("replay",true),row("answered",true),review];let answers=serde_json::from_value(json!({"answered":{"vndb":"v4","source":"user"}})).unwrap();let result=check_answers(&rows,&answers).unwrap();
        assert_eq!(result["actual"]["unreviewed"][0]["relative_path"],"actual");assert_eq!(result["replay"]["unreviewed"][0]["relative_path"],"replay");assert_eq!(result["actual"]["counts"]["unreviewed"],1);assert_eq!(result["replay"]["counts"]["unreviewed"],1);assert_eq!(result["replay"]["counts"]["agree"],1);
    }
    #[test]fn parses_options_and_exit_codes_cover_every_category_and_section(){
        let raw:Vec<_>=["db","--out","out","--root-map","/a=/b","--root-map","/a/c=/d"].into_iter().map(OsString::from).collect();let parsed=args(&raw).unwrap();assert_eq!(parsed.root_maps.len(),2);assert_eq!(parsed.fail_on,fail_on("changed,false").unwrap());
        for invalid in ["","wrong","changed,","false,lost,unknown"]{assert!(fail_on(invalid).is_err());}
        for section in ["actual","replay"]{for key in ["changed","false","lost","missing","added","removed"]{let mut report=Report::new();report.summary=json!({"actual":{},"replay":{}});assert_eq!(exit_code(&report,&fail_on(key).unwrap()),0);if ["added","removed"].contains(&key){report.summary[key]=json!(1);}else{report.summary[section][key]=json!(1);}assert_eq!(exit_code(&report,&fail_on(key).unwrap()),1);assert_eq!(exit_code(&report,&fail_on("changed,false").unwrap()),if ["changed","false"].contains(&key){1}else{0});report.error=Some("bad input".into());assert_eq!(exit_code(&report,&fail_on(key).unwrap()),2);}}
    }
    #[test]fn errors_write_reports_and_collisions_exit_two(){
        let t=tempfile::tempdir().unwrap();let db_path=t.path().join("library.sqlite");let out=t.path().join("report.json");let db=galroon_core::db::open(&db_path).unwrap();
        db.lock().unwrap().execute_batch("INSERT INTO roots(id,path,label) VALUES('root','/unused','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('a','root','é','A','archive'),('b','root','é','B','archive');").unwrap();drop(db);
        let raw=vec![db_path.into_os_string(),"--out".into(),out.clone().into_os_string()];assert_eq!(run(&raw).unwrap(),2);let report:Report=read(&out).unwrap();let error=report.error.unwrap();assert!(error.contains("Duplicate path keys")&&error.contains("id a")&&error.contains("id b"));
        let mut invalid=raw.clone();invalid.extend([OsString::from("--fail-on"),OsString::from("bad")]);assert_eq!(run(&invalid).unwrap(),2);assert!(read::<Report>(&out).unwrap().error.unwrap().contains("--fail-on"));
        let mut missing=raw.clone();missing[0]=t.path().join("missing.sqlite").into_os_string();assert_eq!(run(&missing).unwrap(),2);assert!(read::<Report>(&out).unwrap().error.is_some());
        let mut overwrite=raw;overwrite[2]=overwrite[0].clone();assert!(run(&overwrite).is_err());assert!(Connection::open_with_flags(&overwrite[0],OpenFlags::SQLITE_OPEN_READ_ONLY).is_ok());
    }
    #[test]fn successful_runs_write_summaries_before_alarm_exit(){
        let t=tempfile::tempdir().unwrap();let db_path=t.path().join("library.sqlite");let out=t.path().join("report.json");let answers=t.path().join("answers.json");drop(galroon_core::db::open(&db_path).unwrap());std::fs::write(&answers,r#"{"absent":{"vndb":"v4","source":"user"}}"#).unwrap();
        let mut raw=vec![db_path.into_os_string(),"--out".into(),out.clone().into_os_string(),"--answers".into(),answers.into_os_string()];assert_eq!(run(&raw).unwrap(),0);raw.extend([OsString::from("--fail-on"),OsString::from("missing")]);assert_eq!(run(&raw).unwrap(),1);
        let report:Report=read(&out).unwrap();assert_eq!(report.summary["actual"]["missing"],1);assert_eq!(report.summary["replay"]["missing"],1);assert!(report.error.is_none());
    }
    #[test]fn reads_latest_job_even_when_inserted_in_reverse_order_and_keeps_unsearched_resources(){
        let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=galroon_core::db::open(&path).unwrap();
        {let c=db.lock().unwrap();c.execute_batch("INSERT INTO roots(id,path,label) VALUES('root','/unused','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('r0','root','Clannad','Clannad','archive'),('r1','root','Unsearched','Unsearched','archive'),('r2','root','Empty','Empty','archive'); INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES('f0','root','/unused/Clannad','Clannad',1,'1','rar','fixture'),('f1','root','/unused/Unsearched','Unsearched',1,'1','rar','fixture'); INSERT INTO resource_files(resource_id,file_id) VALUES('r0','f0'),('r1','f1'); INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('old','match','completed','{}',1,100),('new','match','completed','{}',1,1); INSERT INTO works(id,title,original_title,vndb_id) VALUES('w','Clannad','Clannad','v4');").unwrap();
            let evidence=json!({"results":[{"id":"v4","title":"Clannad"}],"incomplete":false}).to_string();
            c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,evidence,work_id) VALUES('new','r0',1,'matched',?1,'w')",[evidence]).unwrap();
            c.execute("INSERT INTO match_items(job_id,resource_id,revision,state,evidence) VALUES('old','r0',1,'review','{}')",[]).unwrap();}
        drop(db);let c=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();let rows=rows(&c).unwrap();assert_eq!(rows.len(),3);
        let matched=rows.iter().find(|r|r.resource_id=="r0").unwrap();assert!(matched.strong());assert_eq!(matched.root_path,"/unused");assert_eq!(matched.stored.state.as_deref(),Some("matched"));assert_eq!(matched.stored.vndb_id.as_deref(),Some("v4"));
        assert_eq!(rows.iter().find(|r|r.resource_id=="r1").unwrap().review_reason.as_deref(),Some("Not checked against VNDB yet."));assert!(!rows.iter().find(|r|r.resource_id=="r2").unwrap().strong());
    }
}

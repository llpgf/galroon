//! Read-only live evaluation of the latest recorded review category. Never binds resources.
use galroon_core::{automatch,matching};
use rusqlite::{Connection,OpenFlags};
use serde_json::{Value,json};
use std::{collections::BTreeMap,ffi::OsString,path::{Path,PathBuf}};
type R<T>=Result<T,Box<dyn std::error::Error>>;
const USAGE:&str="live_requery <library.sqlite> --reason <review reason> --out <report.json> [--limit N]";
struct Args{db:PathBuf,out:PathBuf,reason:String,limit:usize}
struct Target{id:String,root:String,path:String,evidence:Value}
fn args(raw:&[OsString])->R<Args>{
    let mut args=raw.iter();let db=args.next().ok_or(USAGE)?;let mut options=BTreeMap::new();
    while let Some(key)=args.next(){let key=key.to_str().ok_or(USAGE)?;if !["--reason","--out","--limit"].contains(&key){return Err(USAGE.into());}
        if options.insert(key,args.next().ok_or(USAGE)?).is_some(){return Err("Duplicate option".into());}
    }
    let reason=options.remove("--reason").and_then(|v|v.to_str()).filter(|v|!v.is_empty()).ok_or(USAGE)?.to_string();
    Ok(Args{db:db.into(),out:options.remove("--out").ok_or(USAGE)?.into(),reason,limit:options.remove("--limit").map(|v|->R<usize>{Ok(v.to_str().ok_or(USAGE)?.parse()?)}).transpose()?.unwrap_or(usize::MAX)})
}
fn targets(c:&Connection,reason:&str,limit:usize)->R<Vec<Target>>{
    let mut s=c.prepare("SELECT r.id,roots.path,r.relative_path,m.evidence FROM resources r JOIN roots ON roots.id=r.root_id JOIN match_items m ON m.rowid=(SELECT mi.rowid FROM match_items mi JOIN jobs j ON j.id=mi.job_id WHERE mi.resource_id=r.id ORDER BY j.rowid DESC LIMIT 1) WHERE json_extract(m.evidence,'$.decision.reason')=?1 ORDER BY r.relative_path,r.id")?;
    let rows=s.query_map([reason],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
    rows.take(limit).map(|r|{let (id,root,path,evidence)=r?;Ok(Target{id,root,path,evidence:serde_json::from_str(&evidence)?})}).collect()
}
fn protect_catalog(db:&Path,out:&Path)->R<()>{
    let db=db.canonicalize()?;
    if out.canonicalize().ok().as_ref()==Some(&db){return Err("Output must be separate from the catalog".into());}
    let parent=out.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")).canonicalize()?;
    if Some(parent.as_path())==db.parent(){
        let name=db.file_name().unwrap().to_string_lossy();let output=out.file_name().ok_or(USAGE)?.to_string_lossy();
        if [name.to_string(),format!("{name}-wal"),format!("{name}-shm"),format!("{name}-journal")].contains(&output.to_string()){return Err("Output must be separate from the catalog and its sidecars".into());}
    }Ok(())
}
fn write_report(out:&Path,report:&Value)->R<()>{
    // Atomic replacement never follows an output hard link into the read-only input catalog.
    let mut file=tempfile::NamedTempFile::new_in(out.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")))?;
    serde_json::to_writer_pretty(&mut file,report)?;file.persist(out)?;Ok(())
}
fn row(target:&Target,input:&matching::Input,result:&Value)->Value{
    let top=&result["results"][0];
    json!({"resource_id":target.id,"root_path":target.root,"path":target.path,
        "old_queries":target.evidence.get("searched").or_else(||target.evidence["source"].get("queries")).cloned().unwrap_or(Value::Null),
        "old_titles":target.evidence["titles"],"new_queries":input.queries,"searched":result["searched"],
        "top_candidate":if top.is_null(){Value::Null}else{json!({"id":top["id"],"title":top["title"],"strength":top["match"]["strength"],"reason":top["match"]["reason"],"ambiguous":top["match"]["ambiguous"]})},
        "decision":result["decision"],"result":result})
}
fn summary(rows:&[Value])->Value{
    let (mut strong,mut review,mut no_candidate,mut errors)=(0,0,0,0);
    for row in rows{
        if row["error"].is_string(){errors+=1;}
        else if row["decision"]["reason"].is_null()&&row["top_candidate"]["strength"]=="strong"{strong+=1;}
        else if row["decision"]["reason"]=="No VNDB candidate found."{no_candidate+=1;}
        else{review+=1;}
    }json!({"resources":rows.len(),"strong":strong,"review":review,"no_candidate":no_candidate,"errors":errors})
}
async fn run(args:Args)->R<bool>{
    protect_catalog(&args.db,&args.out)?;
    let c=Connection::open_with_flags(&args.db,OpenFlags::SQLITE_OPEN_READ_ONLY)?;c.execute_batch("BEGIN")?;
    let targets=targets(&c,&args.reason,args.limit)?;
    let mut report=json!({"algorithm_version":matching::ALGORITHM_VERSION,"reason":args.reason,"selected":targets.len(),"complete":false,"resources":[],"summary":summary(&[])});
    write_report(&args.out,&report)?;
    for target in targets{
        let input=automatch::catalog_input(&c,&target.id).and_then(|v|v.ok_or("Resource has no usable catalog input".into()));
        let result=async{
            let input=input.as_ref().map_err(|e|e.to_string())?;
            // search owns the shared provider cache and normal 1600 ms spacing, including across resources.
            let response=matching::search(input.clone()).await?;
            let result=automatch::replay(&c,&target.id,&response)?.ok_or("Resource has no usable catalog input")?;
            Ok::<_,Box<dyn std::error::Error>>(row(&target,input,&result))
        }.await;
        let entry=match result{Ok(row)=>row,Err(e)=>json!({"resource_id":target.id,"root_path":target.root,"path":target.path,"error":e.to_string(),"old_queries":target.evidence["searched"],"new_queries":input.as_ref().ok().map(|i|&i.queries),"top_candidate":null,"decision":null})};
        report["resources"].as_array_mut().unwrap().push(entry);report["summary"]=summary(report["resources"].as_array().unwrap());
        write_report(&args.out,&report)?;println!("{}/{}: {}",report["resources"].as_array().unwrap().len(),report["selected"],report["summary"]);
    }
    c.execute_batch("ROLLBACK")?;report["complete"]=json!(true);write_report(&args.out,&report)?;
    println!("{}",report["summary"]);Ok(report["summary"]["errors"]==0)
}
#[tokio::main]async fn main(){
    let result=match args(&std::env::args_os().skip(1).collect::<Vec<_>>()){Ok(args)=>run(args).await,Err(e)=>Err(e)};
    std::process::exit(match result{Ok(true)=>0,Ok(false)=>1,Err(e)=>{eprintln!("{e}");2}});
}
#[cfg(test)]mod tests{
    use super::*;
    #[tokio::test]async fn read_only_selection_uses_latest_job_and_limit_without_catalog_changes(){
        let t=tempfile::tempdir().unwrap();let db=t.path().join("library.sqlite");let out=t.path().join("out.json");let c=galroon_core::db::open(&db).unwrap();
        c.lock().unwrap().execute_batch("INSERT INTO roots(id,path,label) VALUES('root','/unused','Fixture'); INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('a','root','A','A','archive'),('b','root','B','B','archive'); INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('old','match','completed','{}',1,100),('new','match','completed','{}',1,1); INSERT INTO match_items(job_id,resource_id,revision,state,evidence) VALUES('new','a',1,'review','{\"decision\":{\"reason\":\"No VNDB candidate found.\"}}'),('old','a',1,'review','{}'),('new','b',1,'review','{}'),('old','b',1,'review','{\"decision\":{\"reason\":\"No VNDB candidate found.\"}}');").unwrap();drop(c);
        let before=std::fs::read(&db).unwrap();let c=Connection::open_with_flags(&db,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let rows=targets(&c,"No VNDB candidate found.",10).unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0].id,"a");assert!(targets(&c,"No VNDB candidate found.",0).unwrap().is_empty());
        assert!(protect_catalog(&db,&db).is_err());assert!(protect_catalog(&db,&t.path().join("library.sqlite-wal")).is_err());
        let raw=vec![db.clone().into_os_string(),"--reason".into(),"No VNDB candidate found.".into(),"--out".into(),out.clone().into_os_string(),"--limit".into(),"0".into()];
        assert!(run(args(&raw).unwrap()).await.unwrap());let report:Value=serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();assert_eq!(report["complete"],true);assert_eq!(report["summary"],summary(&[]));assert_eq!(std::fs::read(db).unwrap(),before);
    }
    #[test]fn counts_final_admission_not_candidate_strength_and_keeps_failures_separate(){
        let rows=vec![json!({"top_candidate":{"strength":"strong"},"decision":{"reason":null}}),json!({"top_candidate":{"strength":"strong"},"decision":{"reason":"Conflicting source"}}),json!({"decision":{"reason":"No VNDB candidate found."}}),json!({"error":"VNDB unavailable"})];
        assert_eq!(summary(&rows),json!({"resources":4,"strong":1,"review":1,"no_candidate":1,"errors":1}));
    }
}

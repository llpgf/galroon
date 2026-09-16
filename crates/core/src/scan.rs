use crate::db::{Db,id,now,event};
use rusqlite::{params,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::json;
use std::{path::{Path,PathBuf},fs};

#[derive(Clone,Serialize,Deserialize)]
pub struct ScanSpec { pub root_id:String, #[serde(default)] pub scope:String, #[serde(default)] pub exclude:Vec<String> }
pub fn canonical(path:&Path) -> Result<PathBuf,String> { path.canonicalize().map_err(|e|e.to_string()) }
pub fn link(meta:&fs::Metadata)->bool {
    #[cfg(windows)] { use std::os::windows::fs::MetadataExt; meta.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))] { meta.file_type().is_symlink() }
}
pub fn in_scope(root:&Path, scope:&str)->Result<PathBuf,String> {
    let rel=Path::new(scope);
    if rel.is_absolute() || rel.components().any(|c| matches!(c,std::path::Component::ParentDir|std::path::Component::Prefix(_)|std::path::Component::RootDir)) {return Err("Scope must be a relative folder inside the source".into());}
    let p=canonical(&root.join(rel))?;
    if !p.starts_with(root) || !p.is_dir() {return Err("Scope is not a source directory".into());} Ok(p)
}
pub fn create(db:&Db,spec:ScanSpec)->Result<String,String> {
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;let jid=enqueue(&tx,spec,true,false)?;tx.commit().map_err(|e|e.to_string())?;Ok(jid)
}
pub(crate) fn enqueue(c:&rusqlite::Connection,spec:ScanSpec,recursive:bool,automatic:bool)->Result<String,String> {
    let root:String=c.query_row("SELECT path FROM roots WHERE id=?1",[&spec.root_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    crate::plans::validate_chain(Path::new(&root))?;let root=canonical(Path::new(&root))?;
    let start=in_scope(&root,&spec.scope)?;
    if crate::backup::protected_path(&start){return Err("Catalog backup folders cannot be scanned as game sources".into());}
    crate::plans::validate_chain(&root.join(&spec.scope))?;
    crate::roots::require_idle(&c)?;
    let jid=id();
    let mut payload=json!(spec);payload["recursive"]=json!(recursive);payload["automatic"]=json!(automatic);
    c.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES(?1,'scan','queued',?2,?3,?3)",params![jid,payload.to_string(),now()]).map_err(|e|e.to_string())?;
    c.execute("INSERT INTO frontier(job_id,path) VALUES(?1,?2)",params![jid,start.to_string_lossy()]).map_err(|e|e.to_string())?;
    Ok(jid)
}
fn state(db:&Db,jid:&str)->String { db.lock().ok().and_then(|c|c.query_row("SELECT state FROM jobs WHERE id=?1",[jid],|r|r.get(0)).ok()).unwrap_or_else(||"failed".into()) }
fn stop(db:&Db,jid:&str)->bool {
    let s=state(db,jid);
    if s=="running" {return false;}
    if let Ok(c)=db.lock() {let target=if s=="pausing" {"paused"} else if s=="cancelling" {"cancelled"} else {&s};let _=c.execute("UPDATE jobs SET state=?2,updated=?3 WHERE id=?1",params![jid,target,now()]);} true
}
fn skipped(db:&Db,jid:&str,path:&Path)->Result<(),String>{let c=db.lock().map_err(|e|e.to_string())?;crate::job_summary::record(&c,jid,"skipped",&path.to_string_lossy(),"yes").map_err(|e|e.to_string())}
fn error(db:&Db,jid:&str,path:&str,message:&str) {
    crate::diagnostics::record("error","scan.error",message,Some(jid));
    if let Ok(mut c)=db.lock(){if let Ok(tx)=c.transaction(){
        let saved=(||->rusqlite::Result<()>{
            tx.execute("UPDATE jobs SET errors=errors+1,message=?2,updated=?3 WHERE id=?1",params![jid,message,now()])?;
            event(&tx,jid,"scan.error",&json!({"message":message}))?;
            crate::job_summary::record(&tx,jid,"failed",path,"yes")
        })();if saved.is_ok(){let _=tx.commit();}
    }}
}
pub fn run(db:Db,jid:String) {
    if let Err(e)=run_inner(&db,&jid) {crate::diagnostics::record("error","scan.failed",&e,Some(&jid));if let Ok(c)=db.lock(){let _=c.execute("UPDATE jobs SET state='failed',message=?2,updated=?3 WHERE id=?1",params![jid,e,now()]);}}
}
fn run_inner(db:&Db,jid:&str)->Result<(),String> {
    let (spec,root,recursive)={let c=db.lock().map_err(|e|e.to_string())?;
        let (s,st):(String,String)=c.query_row("SELECT spec,state FROM jobs WHERE id=?1",[jid],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
        if st!="queued" {return Ok(());}
        let recursive=serde_json::from_str::<serde_json::Value>(&s).map_err(|e|e.to_string())?["recursive"].as_bool().unwrap_or(true);
        let spec:ScanSpec=serde_json::from_str(&s).map_err(|e|e.to_string())?;
        let root:String=c.query_row("SELECT path FROM roots WHERE id=?1",[&spec.root_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        c.execute("UPDATE jobs SET state='running',updated=?2 WHERE id=?1 AND state='queued'",params![jid,now()]).map_err(|e|e.to_string())?;
        (spec,canonical(Path::new(&root))?,recursive)};
    loop {
        if stop(db,jid){return Ok(());}
        let next:Option<String>={let c=db.lock().map_err(|e|e.to_string())?;c.query_row("SELECT path FROM frontier WHERE job_id=?1 AND state IN ('pending','running') ORDER BY path LIMIT 1",[jid],|r|r.get(0)).optional().map_err(|e|e.to_string())?};
        let Some(dir)=next else {break};
        let dp=Path::new(&dir);
        if crate::backup::protected_path(dp){skipped(db,jid,dp)?;let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE frontier SET state='done' WHERE job_id=?1 AND path=?2",params![jid,dir]).map_err(|e|e.to_string())?;continue;}
        if !canonical(dp)?.starts_with(&root) || link(&fs::symlink_metadata(dp).map_err(|e|e.to_string())?) {return Err("Source directory changed or became a link".into());}
        {let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE frontier SET state='running' WHERE job_id=?1 AND path=?2",params![jid,dir]).map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET current_path=?2 WHERE id=?1",params![jid,dir]).map_err(|e|e.to_string())?;}
        let entries=match fs::read_dir(dp) {Ok(e)=>e,Err(e)=>{error(db,jid,&dir,&format!("{dir}: {e}"));let c=db.lock().unwrap();c.execute("UPDATE frontier SET state='error' WHERE job_id=?1 AND path=?2",params![jid,dir]).map_err(|e|e.to_string())?;continue}};
        let mut complete=true;let mut present_children=std::collections::HashSet::new();
        for result in entries {
            if stop(db,jid) {return Ok(());}
            let entry=match result {Ok(e)=>e,Err(e)=>{complete=false;error(db,jid,&dir,&format!("{dir}: {e}"));continue}};
            let path=entry.path();
            let name=entry.file_name().to_string_lossy().to_string();
            // Even unreadable or excluded children were seen. Never infer their absence.
            present_children.insert(entry.file_name());
            if crate::backup::protected_path(&path){skipped(db,jid,&path)?;continue;}
            if name.starts_with(".galroon-")||["@eaDir","#recycle",".snapshot",".galroon"].contains(&name.as_str()) {skipped(db,jid,&path)?;continue;}
            let rel=path.strip_prefix(&root).map_err(|e|e.to_string())?.to_string_lossy().to_string();
            if spec.exclude.iter().any(|ex|Path::new(&rel).starts_with(Path::new(ex))) {skipped(db,jid,&path)?;continue;}
            let meta=match fs::symlink_metadata(&path){Ok(m)=>m,Err(e)=>{complete=false;error(db,jid,&path.to_string_lossy(),&format!("{}: {e}",path.display()));continue}};
            if link(&meta){skipped(db,jid,&path)?;continue;}
            if meta.is_dir(){if recursive{let c=db.lock().map_err(|e|e.to_string())?;c.execute("INSERT OR IGNORE INTO frontier(job_id,path) VALUES(?1,?2)",params![jid,path.to_string_lossy()]).map_err(|e|e.to_string())?;}continue;}
            if !meta.is_file(){skipped(db,jid,&path)?;continue;}
            let full=path.to_string_lossy().to_string();let size=meta.len() as i64;
            let mtime=meta.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|t|t.as_nanos().to_string()).unwrap_or_default();
            let ext=path.extension().map(|s|s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
            let prior:Option<(i64,String)>=tx.query_row("SELECT size,mtime FROM files WHERE root_id=?1 AND path=?2",params![spec.root_id,full],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(root_id,path) DO UPDATE SET hash=CASE WHEN size=excluded.size AND mtime=excluded.mtime THEN hash ELSE NULL END,size=excluded.size,mtime=excluded.mtime,seen_job=excluded.seen_job,availability='present'",params![id(),spec.root_id,full,rel,size,mtime,ext,jid]).map_err(|e|e.to_string())?;
            let fid:String=tx.query_row("SELECT id FROM files WHERE root_id=?1 AND path=?2",params![spec.root_id,full],|r|r.get(0)).map_err(|e|e.to_string())?;
            if prior.is_none(){crate::job_summary::record(&tx,jid,"new_files",&fid,"yes").map_err(|e|e.to_string())?;}else if prior.as_ref()!=Some(&(size,mtime.clone())) && !tx.query_row("SELECT EXISTS(SELECT 1 FROM job_summary_entries WHERE job_id=?1 AND dimension='new_files' AND subject=?2)",params![jid,fid],|r|r.get::<_,bool>(0)).map_err(|e|e.to_string())?{crate::job_summary::record(&tx,jid,"updated_files",&fid,"yes").map_err(|e|e.to_string())?;}
            let added=tx.execute("INSERT OR IGNORE INTO observations(job_id,file_id) VALUES(?1,?2)",params![jid,fid]).map_err(|e|e.to_string())?;
            if added>0 {tx.execute("UPDATE jobs SET processed=processed+1,discovered=discovered+1,bytes=bytes+?2,updated=?3 WHERE id=?1",params![jid,size,now()]).map_err(|e|e.to_string())?;}
            // Keep previously confirmed resource membership when scanning an organized tree.
            let existing:Option<String>=tx.query_row("SELECT resource_id FROM resource_files WHERE file_id=?1 ORDER BY resource_id LIMIT 1",[&fid],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            if existing.is_none(){
                let parts:Vec<_>=Path::new(&rel).components().collect();let volume=if parts.len()==1{crate::multipart::part(&rel)}else{None};
                let resource_path=if parts.len()>1 {parts[0].as_os_str().to_string_lossy().to_string()}else if let Some(p)=&volume{p.entry.clone()}else{rel.clone()};
                let resource_kind=if parts.len()>1 {"directory"}else if volume.as_ref().is_some_and(|p|p.numbered){"multipart"}else if ["rar","zip","7z","001"].contains(&ext.as_str()){"archive"}else if ["iso","mds","mdf","cue","bin"].contains(&ext.as_str()){"disc"}else{"file"};
                let new_resource=tx.execute("INSERT OR IGNORE INTO resources(id,root_id,relative_path,title,kind) VALUES(?1,?2,?3,?4,?5)",params![id(),spec.root_id,resource_path,clean_title(&resource_path),resource_kind]).map_err(|e|e.to_string())?;
                let rid:String=tx.query_row("SELECT id FROM resources WHERE root_id=?1 AND relative_path=?2 AND manual_group=0",params![spec.root_id,resource_path],|r|r.get(0)).map_err(|e|e.to_string())?;
                if new_resource>0 {crate::job_summary::record(&tx,jid,"new_resources",&rid,"yes").map_err(|e|e.to_string())?;event(&tx,jid,"resource.discovered",&json!({"resource_id":rid,"root_id":spec.root_id})).map_err(|e|e.to_string())?;}
                tx.execute("INSERT OR IGNORE INTO resource_files(resource_id,file_id) VALUES(?1,?2)",params![rid,fid]).map_err(|e|e.to_string())?;
                tx.execute("UPDATE resources SET revision=revision+1,kind=CASE WHEN ?2='multipart' THEN 'multipart' ELSE kind END WHERE id=?1",params![rid,resource_kind]).map_err(|e|e.to_string())?;
                tx.execute("UPDATE works SET revision=revision+1 WHERE id IN (SELECT work_id FROM resource_bindings WHERE resource_id=?1)",[&rid]).map_err(|e|e.to_string())?;
            }
            let mut memberships=tx.prepare("SELECT resource_id FROM resource_files WHERE file_id=?1").map_err(|e|e.to_string())?;
            for rid in memberships.query_map([&fid],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?{crate::job_summary::resource(&tx,jid,&rid.map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;}
            drop(memberships);
            tx.commit().map_err(|e|e.to_string())?;
        }
        let c=db.lock().map_err(|e|e.to_string())?;
        // A successfully enumerated parent proves an absent child/subtree is missing.
        // Entries we skipped or could not read remain unknown, never deleted from the catalog.
        if complete {
            let mut stmt=c.prepare("SELECT id,path FROM files WHERE root_id=?1 AND seen_job!=?2 AND availability IN ('present','unverified')").map_err(|e|e.to_string())?;
            let rows=stmt.query_map(params![spec.root_id,jid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
            let mut missing=Vec::new();for row in rows {let (fid,p)=row.map_err(|e|e.to_string())?;let relative=Path::new(&p).strip_prefix(&root).map_err(|e|e.to_string())?;let excluded=crate::backup::protected_path(Path::new(&p))||spec.exclude.iter().any(|ex|relative.starts_with(Path::new(ex)))||relative.components().any(|part|["@eaDir","#recycle",".snapshot",".galroon"].iter().any(|name|part.as_os_str()==std::ffi::OsStr::new(name)));
                let child=Path::new(&p).strip_prefix(dp).ok().and_then(|p|p.components().next()).map(|c|c.as_os_str());if !excluded&&child.is_some_and(|child|!present_children.contains(child)){missing.push(fid);}}
            drop(stmt);for fid in missing {c.execute("UPDATE files SET availability='missing' WHERE id=?1",[fid]).map_err(|e|e.to_string())?;}
        }
        c.execute("UPDATE frontier SET state=?3 WHERE job_id=?1 AND path=?2",params![jid,dir,if complete{"done"}else{"error"}]).map_err(|e|e.to_string())?;
    }
    let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET state='completed',current_path='',message=CASE WHEN errors>0 THEN 'Scan completed with errors. Some folders could not be read.' ELSE 'Scan complete' END,updated=?2 WHERE id=?1",params![jid,now()]).map_err(|e|e.to_string())?;event(&c,jid,"scan.completed",&json!({})).map_err(|e|e.to_string())?;Ok(())
}
pub fn clean_title(s:&str)->String {
    let mut text=regex::Regex::new(r"(?i)\.(rar|zip|7z|iso)(\.\d+)?$").unwrap().replace(s,"").to_string();
    text=regex::Regex::new(r"\[[^\]]*\]").unwrap().replace_all(&text,"").to_string();
    text=regex::Regex::new(r"^\([^)]*\)\s*").unwrap().replace(&text,"").to_string();
    text.trim().to_string()
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]fn removed_subtree_is_missing_but_excluded_scoped_and_offline_files_are_preserved(){
        let t=tempfile::tempdir().unwrap();let root=t.path().join("source");for path in ["A/gone/story.zip","A/excluded/extra.zip","B/outside.zip"]{let p=root.join(path);fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,b"fixture").unwrap();}
        let db=crate::db::open(&t.path().join("state/db.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'test')",[root.to_str().unwrap()]).unwrap();
        let scan=|scope:&str,exclude:Vec<String>|{let j=create(&db,ScanSpec{root_id:"r".into(),scope:scope.into(),exclude}).unwrap();run(db.clone(),j.clone());j};scan("",vec![]);
        fs::rename(root.join("A"),t.path().join("saved-A")).unwrap();fs::create_dir(root.join("A")).unwrap();fs::rename(root.join("B"),t.path().join("saved-B")).unwrap();scan("A",vec!["A/excluded".into()]);
        let status=|suffix:&str|{let c=db.lock().unwrap();c.query_row("SELECT availability FROM files WHERE relative_path=?1",[suffix.replace('/',std::path::MAIN_SEPARATOR_STR)],|r|r.get::<_,String>(0)).unwrap()};assert_eq!(status("A/gone/story.zip"),"missing");assert_eq!(status("A/excluded/extra.zip"),"present");assert_eq!(status("B/outside.zip"),"present");
        let j=create(&db,ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();fs::rename(&root,t.path().join("offline-source")).unwrap();run(db.clone(),j.clone());assert_eq!(state(&db,&j),"failed");assert_eq!(status("B/outside.zip"),"present");assert_eq!(status("A/excluded/extra.zip"),"present");
    }
    #[test] fn scoped_scan_preserves_outside_and_resumes() {
        let t=tempfile::tempdir().unwrap();let root=t.path().join("source");fs::create_dir_all(root.join("A")).unwrap();fs::create_dir_all(root.join("B")).unwrap();fs::write(root.join("A/a.zip"),b"a").unwrap();fs::write(root.join("B/b.rar"),b"b").unwrap();
        let db=crate::db::open(&t.path().join("state/db.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'test')",[root.to_str().unwrap()]).unwrap();
        let j=create(&db,ScanSpec{root_id:"r".into(),scope:"A".into(),exclude:vec![]}).unwrap();run(db.clone(),j.clone());
        assert_eq!(state(&db,&j),"completed");let n:i64=db.lock().unwrap().query_row("SELECT count(*) FROM files",[],|r|r.get(0)).unwrap();assert_eq!(n,1);
        assert!(in_scope(&canonical(&root).unwrap(),"../").is_err());
        let j2=create(&db,ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();run(db.clone(),j2);let n:i64=db.lock().unwrap().query_row("SELECT count(*) FROM files",[],|r|r.get(0)).unwrap();assert_eq!(n,2);
    }
    #[test] fn paused_scan_recovers_after_core_restart_without_duplicate_observations(){
        let t=tempfile::tempdir().unwrap();let root=t.path().join("source");fs::create_dir(&root).unwrap();for i in 0..350{fs::write(root.join(format!("{i}.zip")),b"fixture").unwrap();}
        let state_path=t.path().join("state/library.sqlite");let db=crate::db::open(&state_path).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'test')",[root.to_str().unwrap()]).unwrap();
        let j=create(&db,ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();let worker_db=db.clone();let worker_id=j.clone();let worker=std::thread::spawn(move||run(worker_db,worker_id));
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);loop{let c=db.lock().unwrap();let n:i64=c.query_row("SELECT processed FROM jobs WHERE id=?1",[&j],|r|r.get(0)).unwrap();if n>5{c.execute("UPDATE jobs SET state='pausing' WHERE id=?1 AND state='running'",[&j]).unwrap();break;}assert!(std::time::Instant::now()<deadline);drop(c);std::thread::sleep(std::time::Duration::from_millis(1));}
        worker.join().unwrap();assert_eq!(state(&db,&j),"paused");{let c=db.lock().unwrap();c.execute("UPDATE jobs SET state='running' WHERE id=?1",[&j]).unwrap();}drop(db);
        let db=crate::db::open(&state_path).unwrap();assert_eq!(state(&db,&j),"interrupted");db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&j]).unwrap();run(db.clone(),j.clone());assert_eq!(state(&db,&j),"completed");let c=db.lock().unwrap();let n:i64=c.query_row("SELECT processed FROM jobs WHERE id=?1",[&j],|r|r.get(0)).unwrap();assert_eq!(n,350);let files:i64=c.query_row("SELECT count(*) FROM files",[],|r|r.get(0)).unwrap();assert_eq!(files,350);let summary=crate::job_summary::read(&c,&j).unwrap();assert_eq!(summary["new_files"],350);assert_eq!(summary["new_resources"],350);assert_eq!(summary["updated_files"],0);
    }
}

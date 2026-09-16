//! Persisted, explicitly approved, no-replace file moves. Test only on generated fixtures.
use crate::{db::{Db,id,now},scan};
use rusqlite::{params,OptionalExtension};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{path::{Path,PathBuf},fs,io::Read};
type R<T>=Result<T,String>;

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct Item {pub id:String,pub file_id:String,pub source:String,pub target:String,pub sha256:String,pub size:u64, pub retained:Option<String>}
pub fn hash(p:&Path)->R<(u64,String)>{
    let before=fs::symlink_metadata(p).map_err(|e|e.to_string())?;
    if !before.is_file() || scan::link(&before){return Err("Not a regular file".into());}
    let mut f=fs::File::open(p).map_err(|e|e.to_string())?;let mut h=Sha256::new();let mut buf=vec![0;1024*1024];let mut bytes=0;
    loop{let n=f.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break;}h.update(&buf[..n]);bytes+=n as u64;}
    let after=f.metadata().map_err(|e|e.to_string())?;
    if before.len()!=after.len()||bytes!=before.len()||before.modified().ok()!=after.modified().ok(){return Err("Source changed during verification".into());}
    Ok((bytes,hex::encode(h.finalize())))
}
pub fn validate_chain(path:&Path)->R<()> {
    for p in path.ancestors(){if p.exists(){let m=fs::symlink_metadata(p).map_err(|e|e.to_string())?;if scan::link(&m){return Err("Linked paths cannot be mutated".into());}}}Ok(())
}
fn existing_parent(p:&Path)->R<PathBuf>{let mut p=p.to_path_buf();while !p.exists(){if !p.pop(){return Err("Destination has no accessible parent".into());}}scan::canonical(&p)}
fn same_volume(a:&Path,b:&Path)->R<bool>{
    #[cfg(windows)] {
        use std::os::windows::{fs::OpenOptionsExt,io::AsRawHandle};
        fn volume(p:&Path)->R<u32>{
            let f=fs::OpenOptions::new().read(true).custom_flags(0x02000000).open(p).map_err(|e|e.to_string())?;
            let mut info=unsafe{std::mem::zeroed::<windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION>()};
            if unsafe{windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(f.as_raw_handle(),&mut info)}==0{return Err(std::io::Error::last_os_error().to_string());}
            Ok(info.dwVolumeSerialNumber)
        }
        Ok(volume(a)?==volume(b)?)
    }
    #[cfg(not(windows))] {use std::os::unix::fs::MetadataExt;Ok(fs::metadata(a).map_err(|e|e.to_string())?.dev()==fs::metadata(b).map_err(|e|e.to_string())?.dev())}
}
pub fn no_replace_move(a:&Path,b:&Path)->R<()> {
    validate_chain(a)?;validate_chain(b)?;
    if b.exists(){return Err("Destination already exists; no file was overwritten".into());}
    if !same_volume(a,&existing_parent(b)?)?{return Err("Cross-volume moves are not supported. Choose a folder on the source volume.".into());}
    fs::create_dir_all(b.parent().ok_or("Missing destination parent")?).map_err(|e|e.to_string())?;
    validate_chain(b)?;
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        let aw:Vec<u16>=a.as_os_str().encode_wide().chain(Some(0)).collect();
        let bw:Vec<u16>=b.as_os_str().encode_wide().chain(Some(0)).collect();
        // Antivirus/indexer handles can briefly deny a directory rename. Every
        // retry retains the kernel's no-replace guarantee and rechecks links.
        for attempt in 0..6 {
            validate_chain(a)?;validate_chain(b)?;
            if b.exists(){return Err("Destination already exists; no file was overwritten".into());}
            if unsafe{windows_sys::Win32::Storage::FileSystem::MoveFileExW(aw.as_ptr(),bw.as_ptr(),0x8)}!=0{break;}
            let error=std::io::Error::last_os_error();
            if attempt==5||!matches!(error.raw_os_error(),Some(5|32|33)){
                return Err(format!("Move {} to {} failed: {}",a.display(),b.display(),error));
            }
            std::thread::sleep(std::time::Duration::from_millis(100*(attempt+1)));
        }
    }
    #[cfg(not(windows))] {return Err("Mutation adapter has only been enabled for Windows in this build".into());}
    #[allow(unreachable_code)] Ok(())
}
pub fn safe_name(s:&str)->String {
    let mut n:String=s.chars().map(|c|if c.is_control()||"<>:\"/\\|?*".contains(c){'_'}else{c}).collect();n=n.trim().trim_end_matches(['.',' ']).chars().take(110).collect();if n.is_empty(){n="Untitled".into();}
    let stem=n.split('.').next().unwrap_or("").to_uppercase();if ["CON","PRN","AUX","NUL","COM1","COM2","COM3","COM4","COM5","COM6","COM7","COM8","COM9","LPT1","LPT2","LPT3","LPT4","LPT5","LPT6","LPT7","LPT8","LPT9"].contains(&stem.as_str()){n.insert(0,'_');}n
}
fn store(db:&Db,kind:&str,items:Vec<Item>)->R<Value>{let c=db.lock().map_err(|e|e.to_string())?;store_in(&c,kind,items)}
pub(crate) fn store_in(c:&rusqlite::Connection,kind:&str,items:Vec<Item>)->R<Value>{
    if items.is_empty(){return Err("There are no files to move".into());}
    let payload=serde_json::to_string(&items).map_err(|e|e.to_string())?;let digest=hex::encode(Sha256::digest(payload.as_bytes()));let pid=id();
    c.execute("INSERT INTO plans(id,kind,state,items,digest,created) VALUES(?1,?2,'ready',?3,?4,?5)",params![pid,kind,payload,digest,now()]).map_err(|e|e.to_string())?;
    Ok(json!({"id":pid,"kind":kind,"state":"ready","digest":digest,"items":items}))
}
pub fn organize(db:&Db,rid:&str,destination:&str)->R<Value>{
    let origin={let c=db.lock().map_err(|e|e.to_string())?;crate::roots::require_idle(&c)?;crate::undo::capture(&c,rid)?};
    require_resource_available(db,rid)?;
    {let c=db.lock().map_err(|e|e.to_string())?;let pending:i64=c.query_row("SELECT count(*) FROM plan_locations l JOIN plans p ON p.id=l.plan_id WHERE l.resource_id=?1 AND p.state IN ('approved','executing','partial')",[rid],|r|r.get(0)).map_err(|e|e.to_string())?;if pending>0{return Err("Finish the existing approved plan for this resource first".into());}}
    let dest=scan::canonical(Path::new(destination))?;validate_chain(&dest)?;if !dest.is_dir(){return Err("Choose an existing managed folder".into());}
    {let c=db.lock().map_err(|e|e.to_string())?;crate::managed_root::validate(&c,&dest)?;}
    let (root,relative,original,workid,external,release,edition_id,role,files)={let c=db.lock().map_err(|e|e.to_string())?;
        let (root,rel,original,wid,ext,release,edition_id,role):(String,String,String,String,Option<String>,String,Option<String>,Option<String>)=c.query_row("SELECT rt.path,r.relative_path,w.original_title,w.id,w.vndb_id,r.release_label,b.release_id,b.role FROM resources r JOIN roots rt ON rt.id=r.root_id JOIN works w ON w.id=r.work_id LEFT JOIN resource_bindings b ON b.resource_id=r.id AND b.work_id=r.work_id WHERE r.id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).map_err(|_|"Match this resource to a work first")?;
        let mut s=c.prepare("SELECT f.id,f.path FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 AND f.availability='present' ORDER BY f.path").map_err(|e|e.to_string())?;
        let rows=s.query_map([rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;(root,rel,original,wid,ext,release,edition_id,role,rows)};
    let source=Path::new(&root).join(&relative);let source=scan::canonical(&source)?;
    let manual:bool=db.lock().map_err(|e|e.to_string())?.query_row("SELECT manual_group FROM resources WHERE id=?1",[rid],|r|r.get(0)).map_err(|e|e.to_string())?;
    if !manual&&dest.starts_with(&source){return Err("Managed folder must not be inside the resource".into());}
    let folder=format!("{} [{}]",safe_name(&original),safe_name(external.as_deref().unwrap_or(&workid)));
    let release_folder=if let Some(edition)=edition_id{format!("{} [{}]/{rid}",safe_name(&release),edition)}else if role.as_deref()==Some("extra"){format!("Extras/{rid}")}else{format!("{} [{}]",safe_name(&release),&rid[..8])};let mut items=Vec::new();
    for (fid,p) in files {let p=scan::canonical(Path::new(&p))?;validate_chain(&p)?;let suffix=if source.is_dir(){p.strip_prefix(&source).map_err(|_|"Resource member is outside its folder")?.to_path_buf()}else{PathBuf::from(p.file_name().ok_or("Invalid filename")?)};
        let target=dest.join(&folder).join(&release_folder).join(suffix);if target==p{continue;}if target.exists(){return Err(format!("Destination exists: {}",target.display()));}
        if !same_volume(&p,&dest)?{return Err("Managed folder must be on the same volume".into());}let (size,sha256)=hash(&p)?;
        items.push(Item{id:id(),file_id:fid,source:p.to_string_lossy().into(),target:target.to_string_lossy().into(),sha256,size,retained:None});
    }
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    crate::managed_root::validate(&tx,&dest)?;
    if crate::undo::capture(&tx,rid)?!=origin{return Err("Resource location changed during preview. Create a new plan.".into());}
    let plan=store_in(&tx,"organize",items)?;crate::undo::store(&tx,plan["id"].as_str().unwrap(),&origin)?;
    tx.execute("INSERT INTO plan_locations(plan_id,resource_id,root_path,relative_path) VALUES(?1,?2,?3,?4)",params![plan["id"].as_str().unwrap(),rid,dest.to_string_lossy(),Path::new(&folder).join(&release_folder).to_string_lossy()]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;Ok(plan)
}
pub fn require_resource_available(db:&Db,rid:&str)->R<()>{let c=db.lock().map_err(|e|e.to_string())?;let n:i64=c.query_row("SELECT count(*) FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 AND f.availability!='present'",[rid],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("This resource contains missing or unverified files. Reconnect its source and scan it before continuing.".into());}Ok(())}
pub fn approve(db:&Db,pid:&str,digest:&str)->R<()> {let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;let n=tx.execute("UPDATE plans SET state='approved',approved=?3 WHERE id=?1 AND digest=?2 AND state='ready'",params![pid,digest,now()]).map_err(|e|e.to_string())?;if n!=1{return Err("Plan has changed or is not awaiting approval".into());}crate::managed_root::select_plan(&tx,pid)?;tx.commit().map_err(|e|e.to_string())?;Ok(())}
fn verify(p:&Path,item:&Item)->R<()> {validate_chain(p)?;let (size,h)=hash(p)?;if size!=item.size||h!=item.sha256{return Err("File content changed since preview. Create a new plan.".into());}Ok(())}
pub fn execute(db:&Db,pid:&str)->R<Value>{
    let (kind,items)={let c=db.lock().map_err(|e|e.to_string())?;crate::roots::require_idle(&c)?;let (kind,state,payload,digest):(String,String,String,String)=c.query_row("SELECT kind,state,items,digest FROM plans WHERE id=?1",[pid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
        if state!="approved"&&state!="partial" {return Err("Preview and approve the exact plan before execution".into());}
        if hex::encode(Sha256::digest(payload.as_bytes()))!=digest{return Err("Plan integrity check failed".into());}
        let items=serde_json::from_str::<Vec<Item>>(&payload).map_err(|e|e.to_string())?;
        if kind=="undo_organize"{crate::undo::validate(&c,pid,&items)?;}
        crate::managed_root::select_plan(&c,pid)?;
        c.execute("UPDATE plans SET state='executing' WHERE id=?1",[pid]).map_err(|e|e.to_string())?;(kind,items)};
    let mut failed=0;
    for item in &items {
        let result=(||->R<()>{
            let state:Option<String>={let c=db.lock().map_err(|e|e.to_string())?;c.query_row("SELECT state FROM operations WHERE id=?1",[&item.id],|r|r.get(0)).optional().map_err(|e|e.to_string())?};
            if state.as_deref()==Some("completed"){return verify(Path::new(&item.target),item);}
            let source=Path::new(&item.source);let target=Path::new(&item.target);
            if let Some(keep)=&item.retained {verify(Path::new(keep),item)?;}
            if source.exists() {
                verify(source,item)?;if target.exists(){return Err("Destination conflict: nothing overwritten".into());}
                {let c=db.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO operations(id,plan_id,source,target,state,fingerprint) VALUES(?1,?2,?3,?4,'prepared',?5) ON CONFLICT(id) DO UPDATE SET state='prepared',error=''",params![item.id,pid,item.source,item.target,item.sha256]).map_err(|e|e.to_string())?;}
                no_replace_move(source,target)?;
            }else if state.is_none(){return Err("Source disappeared before execution".into());}
            verify(target,item)?;
            let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
            let planned_root:Option<String>=tx.query_row("SELECT root_path FROM plan_locations WHERE plan_id=?1",[pid],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
            if let Some(rp)=&planned_root {tx.execute("INSERT OR IGNORE INTO roots(id,path,label,role) VALUES(?1,?2,'Managed library','managed')",params![id(),rp]).map_err(|e|e.to_string())?;}
            let existing_root:Option<(String,String)>={let mut s=tx.prepare("SELECT id,path FROM roots").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;rows.into_iter().filter(|(_,p)|planned_root.as_ref().map_or_else(||target.starts_with(p),|rp|rp==p)).max_by_key(|(_,p)|p.len())};
            let (root_id,root_path)=if let Some(r)=existing_root{r}else{let root_id=id();let rp=target.parent().ok_or("No parent")?.to_string_lossy().to_string();tx.execute("INSERT INTO roots(id,path,label,role) VALUES(?1,?2,?3,?4)",params![root_id,rp,"Managed location",if kind=="quarantine"{"quarantine"}else{"managed"}]).map_err(|e|e.to_string())?;(root_id,rp)};
            let rel=target.strip_prefix(scan::canonical(Path::new(&root_path))?).map_err(|e|e.to_string())?.to_string_lossy();tx.execute("UPDATE files SET path=?2,root_id=?3,relative_path=?4 WHERE id=?1",params![item.file_id,item.target,root_id,rel]).map_err(|e|e.to_string())?;
            if kind=="quarantine" {tx.execute("INSERT OR IGNORE INTO quarantine(id,plan_id,original,current) VALUES(?1,?2,?3,?4)",params![item.id,pid,item.source,item.target]).map_err(|e|e.to_string())?;}
            if kind=="restore" {tx.execute("UPDATE quarantine SET state='restored' WHERE current=?1 AND original=?2",params![item.source,item.target]).map_err(|e|e.to_string())?;}
            tx.execute("UPDATE operations SET state='completed',error='' WHERE id=?1",[&item.id]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(())})();
        if let Err(e)=result {failed+=1;let c=db.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO operations(id,plan_id,source,target,state,fingerprint,error) VALUES(?1,?2,?3,?4,'conflict',?5,?6) ON CONFLICT(id) DO UPDATE SET state='conflict',error=excluded.error",params![item.id,pid,item.source,item.target,item.sha256,e]).map_err(|e|e.to_string())?;}
    }
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    let state=if failed==0{"completed"}else{"partial"};
    if failed==0&&["organize","undo_organize"].contains(&kind.as_str()) {
        tx.execute("UPDATE resources SET revision=revision+1,root_id=(SELECT rt.id FROM plan_locations l JOIN roots rt ON rt.path=l.root_path WHERE l.plan_id=?1),relative_path=(SELECT relative_path FROM plan_locations WHERE plan_id=?1) WHERE id=(SELECT resource_id FROM plan_locations WHERE plan_id=?1)",[pid]).map_err(|e|e.to_string())?;
    }
    tx.execute("UPDATE plans SET state=?2 WHERE id=?1",params![pid,state]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(json!({"id":pid,"state":state,"failed":failed,"total":items.len()}))
}
pub fn isolate(db:&Db,fid:&str,keep:&str,dest:&str)->R<Value>{
    if fid==keep{return Err("Choose a different copy to retain".into());}
    let (source,retained)={let c=db.lock().map_err(|e|e.to_string())?;let query=|id:&str|c.query_row("SELECT path FROM files WHERE id=?1 AND availability='present'",[id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string());(query(fid)?,query(keep)?)};
    let src=scan::canonical(Path::new(&source))?;let retained=scan::canonical(Path::new(&retained))?;if src==retained{return Err("Both entries refer to the same path".into());}
    let (size,sha256)=hash(&src)?;let (kbytes,khash)=hash(&retained)?;if size!=kbytes||sha256!=khash{return Err("Copies do not have identical content".into());}
    let dest=scan::canonical(Path::new(dest))?;validate_chain(&dest)?;if !dest.is_dir(){return Err("Choose a quarantine folder".into());}if !same_volume(&src,&dest)?{return Err("Quarantine must be on the same volume".into());}
    let target=dest.join(id()).join(src.file_name().ok_or("Invalid name")?);
    store(db,"quarantine",vec![Item{id:id(),file_id:fid.into(),source:src.to_string_lossy().into(),target:target.to_string_lossy().into(),sha256,size,retained:Some(retained.to_string_lossy().into())}])
}
pub fn restore(db:&Db,qid:&str)->R<Value>{
    let (source,target,fid)={let c=db.lock().map_err(|e|e.to_string())?;c.query_row("SELECT q.current,q.original,f.id FROM quarantine q JOIN files f ON f.path=q.current WHERE q.id=?1 AND q.state='isolated'",[qid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?};
    if Path::new(&target).exists(){return Err("Original location is occupied. Restore cannot overwrite it.".into());}
    let (size,sha256)=hash(Path::new(&source))?;store(db,"restore",vec![Item{id:id(),file_id:fid,source,target,sha256,size,retained:None}])
}

#[cfg(test)] mod tests{
    use super::*;
    #[test]fn nested_directory_can_be_published(){let t=tempfile::tempdir().unwrap();let a=t.path().join("a");let b=t.path().join("b");let c=t.path().join("c");fs::create_dir(&a).unwrap();fs::create_dir(&b).unwrap();fs::write(a.join("hello.txt"),b"hello").unwrap();no_replace_move(&a,&b.join("nested")).unwrap();no_replace_move(&b,&c).unwrap();assert_eq!(fs::read(c.join("nested/hello.txt")).unwrap(),b"hello");}
    #[test]fn active_scan_and_file_execution_cannot_overlap(){let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();let p=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();let pid=p["id"].as_str().unwrap();approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();let spec=scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]};let j=scan::create(&db,spec.clone()).unwrap();assert!(execute(&db,pid).is_err());assert!(t.path().join("source/a.zip").exists());db.lock().unwrap().execute("UPDATE jobs SET state='cancelled' WHERE id=?1",[j]).unwrap();db.lock().unwrap().execute("UPDATE plans SET state='executing' WHERE id=?1",[pid]).unwrap();assert!(scan::create(&db,spec).is_err());}
    #[test]fn recovered_batch_rechecks_previously_completed_targets(){let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();let p=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();let pid=p["id"].as_str().unwrap();approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();execute(&db,pid).unwrap();let target=p["items"][0]["target"].as_str().unwrap();fs::write(target,b"externally changed").unwrap();db.lock().unwrap().execute("UPDATE plans SET state='partial' WHERE id=?1",[pid]).unwrap();assert_eq!(execute(&db,pid).unwrap()["failed"],1);assert_eq!(fs::read(target).unwrap(),b"externally changed");assert_eq!(fs::read(t.path().join("source/b.zip")).unwrap(),b"identical content");}
    #[test]fn interrupted_move_reconciles_without_overwriting(){
        let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();
        let p=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();let pid=p["id"].as_str().unwrap();approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();
        let item:Item=serde_json::from_value(p["items"][0].clone()).unwrap();
        db.lock().unwrap().execute("INSERT INTO operations(id,plan_id,source,target,state,fingerprint) VALUES(?1,?2,?3,?4,'prepared',?5)",params![item.id,pid,item.source,item.target,item.sha256]).unwrap();
        no_replace_move(Path::new(&item.source),Path::new(&item.target)).unwrap();
        // Crash boundary: filesystem committed, database still says prepared.
        assert_eq!(execute(&db,pid).unwrap()["failed"],0);
        let stored:String=db.lock().unwrap().query_row("SELECT path FROM files WHERE id=?1",[&a],|r|r.get(0)).unwrap();assert_eq!(stored,item.target);
        assert_eq!(fs::read(&stored).unwrap(),b"identical content");
    }
    #[test]fn organized_resource_can_be_organized_again(){
        let(t,db,a,_)=fixture();let first=t.path().join("managed-one");let second=t.path().join("managed-two");fs::create_dir(&first).unwrap();fs::create_dir(&second).unwrap();
        let rid:String={let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','Test','Original title')",[]).unwrap();let rid:String=c.query_row("SELECT resource_id FROM resource_files WHERE file_id=?1",[&a],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='w' WHERE id=?1",[&rid]).unwrap();rid};
        for (index,dest) in [&first,&first].into_iter().enumerate(){if index==1{db.lock().unwrap().execute("UPDATE works SET original_title='Renamed title' WHERE id='w'",[]).unwrap();assert!(organize(&db,&rid,second.to_str().unwrap()).unwrap_err().contains("one managed folder"));}let p=organize(&db,&rid,dest.to_str().unwrap()).unwrap();let pid=p["id"].as_str().unwrap();approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();assert_eq!(execute(&db,pid).unwrap()["failed"],0);let (root,relative):(String,String)=db.lock().unwrap().query_row("SELECT rt.path,r.relative_path FROM resources r JOIN roots rt ON rt.id=r.root_id WHERE r.id=?1",[&rid],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(Path::new(&root),scan::canonical(dest).unwrap());assert!(Path::new(&root).join(relative).join("a.zip").is_file());}
    }
    #[test]fn managed_root_is_selected_by_valid_approval_and_survives_reopen(){
        let(t,db,a,_)=fixture();let first=t.path().join("first");let second=t.path().join("second");fs::create_dir(&first).unwrap();fs::create_dir(&second).unwrap();
        let rid:String={let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','Test','Original')",[]).unwrap();let rid:String=c.query_row("SELECT resource_id FROM resource_files WHERE file_id=?1",[a],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='w' WHERE id=?1",[&rid]).unwrap();rid};
        let p=organize(&db,&rid,first.to_str().unwrap()).unwrap();let q=organize(&db,&rid,second.to_str().unwrap()).unwrap();
        assert!(approve(&db,p["id"].as_str().unwrap(),"wrong digest").is_err());
        assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM settings WHERE key=?1",[crate::managed_root::KEY],|r|r.get::<_,i64>(0)).unwrap(),0);
        approve(&db,p["id"].as_str().unwrap(),p["digest"].as_str().unwrap()).unwrap();
        assert!(approve(&db,q["id"].as_str().unwrap(),q["digest"].as_str().unwrap()).unwrap_err().contains("one managed folder"));
        assert_eq!(db.lock().unwrap().query_row("SELECT state FROM plans WHERE id=?1",[q["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).unwrap(),"ready");
        drop(db);let db=crate::db::open(&t.path().join("state/db.sqlite")).unwrap();
        assert_eq!(PathBuf::from(db.lock().unwrap().query_row("SELECT value FROM settings WHERE key=?1",[crate::managed_root::KEY],|r|r.get::<_,String>(0)).unwrap()),scan::canonical(&first).unwrap());
        assert!(approve(&db,q["id"].as_str().unwrap(),q["digest"].as_str().unwrap()).is_err());
        assert!(t.path().join("source/a.zip").is_file());assert!(fs::read_dir(&first).unwrap().next().is_none());
        let backup=crate::backup::export(&db,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restore"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();
        assert_eq!(PathBuf::from(restored.lock().unwrap().query_row("SELECT value FROM settings WHERE key=?1",[crate::managed_root::KEY],|r|r.get::<_,String>(0)).unwrap()),scan::canonical(&first).unwrap());
        let root_id:String=db.lock().unwrap().query_row("SELECT id FROM roots WHERE role='managed'",[],|r|r.get(0)).unwrap();
        let remap=crate::roots::preview(&db,&root_id,&second).unwrap();crate::roots::remap(&db,&root_id,&second,&remap.digest).unwrap();
        assert_eq!(PathBuf::from(db.lock().unwrap().query_row("SELECT value FROM settings WHERE key=?1",[crate::managed_root::KEY],|r|r.get::<_,String>(0)).unwrap()),scan::canonical(&second).unwrap());
        assert_eq!(db.lock().unwrap().query_row("SELECT state FROM plans WHERE id=?1",[p["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).unwrap(),"superseded");

    }
    #[test]fn new_source_overlap_after_approval_blocks_execution_before_moves(){
        let(t,db,a,_)=fixture();let dest=t.path().join("managed");fs::create_dir(&dest).unwrap();
        let rid:String={let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','Test','Original')",[]).unwrap();let rid:String=c.query_row("SELECT resource_id FROM resource_files WHERE file_id=?1",[a],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='w' WHERE id=?1",[&rid]).unwrap();rid};
        for path in [t.path().to_path_buf(),t.path().join("source")] {assert!(organize(&db,&rid,path.to_str().unwrap()).unwrap_err().contains("separate"));}
        let p=organize(&db,&rid,dest.to_str().unwrap()).unwrap();approve(&db,p["id"].as_str().unwrap(),p["digest"].as_str().unwrap()).unwrap();
        db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('overlap',?1,'New source')",[dest.join("nested-source").to_str().unwrap()]).unwrap();
        assert!(execute(&db,p["id"].as_str().unwrap()).unwrap_err().contains("separate"));assert!(t.path().join("source/a.zip").is_file());assert!(fs::read_dir(&dest).unwrap().next().is_none());
    }
    fn fixture()->(tempfile::TempDir,Db,String,String){let t=tempfile::tempdir().unwrap();let root=t.path().join("source");fs::create_dir(&root).unwrap();fs::write(root.join("a.zip"),b"identical content").unwrap();fs::write(root.join("b.zip"),b"identical content").unwrap();let db=crate::db::open(&t.path().join("state/db.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Test')",[root.to_str().unwrap()]).unwrap();let j=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),j);let (a,b)={let c=db.lock().unwrap();let mut s=c.prepare("SELECT id FROM files ORDER BY path").unwrap();let ids=s.query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();(ids[0].clone(),ids[1].clone())};(t,db,a,b)}
    #[test]fn unapproved_move_is_rejected(){let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();let plan=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();assert!(execute(&db,plan["id"].as_str().unwrap()).is_err());assert!(t.path().join("source/a.zip").exists());}
    #[test]fn isolation_and_restore_preserve_content(){let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();let p=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();let pid=p["id"].as_str().unwrap();approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();let result=execute(&db,pid).unwrap();assert_eq!(result["failed"],0);assert!(!t.path().join("source/a.zip").exists());let qid:String=db.lock().unwrap().query_row("SELECT id FROM quarantine",[],|r|r.get(0)).unwrap();let r=restore(&db,&qid).unwrap();approve(&db,r["id"].as_str().unwrap(),r["digest"].as_str().unwrap()).unwrap();assert_eq!(execute(&db,r["id"].as_str().unwrap()).unwrap()["failed"],0);assert_eq!(fs::read(t.path().join("source/a.zip")).unwrap(),b"identical content");}
    #[test]fn changed_retained_copy_prevents_isolation(){let(t,db,a,b)=fixture();let q=t.path().join("quarantine");fs::create_dir(&q).unwrap();let p=isolate(&db,&a,&b,q.to_str().unwrap()).unwrap();approve(&db,p["id"].as_str().unwrap(),p["digest"].as_str().unwrap()).unwrap();fs::write(t.path().join("source/b.zip"),b"changed").unwrap();assert_eq!(execute(&db,p["id"].as_str().unwrap()).unwrap()["failed"],1);assert!(t.path().join("source/a.zip").exists());}
}



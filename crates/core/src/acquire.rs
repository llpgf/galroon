use crate::{db::{Db,id,now},plans,scan};
use rusqlite::params;
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{path::{Path,PathBuf},fs,io::{Read,Write,Seek,SeekFrom},process::{Command,Stdio}};
type R<T>=Result<T,String>;
#[derive(Clone,Serialize,Deserialize)]pub struct Member{pub id:String,pub source:String,pub relative:String,pub size:u64,pub mtime:String}
#[derive(Clone,Serialize,Deserialize)]pub struct Spec{pub resource_id:String,pub destination:String,pub final_folder:String,pub extract:bool,pub files:Vec<Member>,#[serde(default)]pub source_file_count:usize,#[serde(default)]pub reuse_existing:bool}
#[derive(Deserialize)]pub struct Selection{pub file_ids:Vec<String>,pub manifest_digest:String}
fn mtime(m:&fs::Metadata)->String{m.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_nanos().to_string()).unwrap_or_default()}
pub fn helper()->Option<PathBuf>{
    if let Some(v)=std::env::var_os("GALROON_7ZIP"){let p=PathBuf::from(v);if p.is_file(){return Some(p);}}
    let mut candidates=vec![PathBuf::from("tools/7zip/runtime/7z.exe")];
    if let Ok(e)=std::env::current_exe(){if let Some(p)=e.parent(){candidates.push(p.join("runtime/7z.exe"));candidates.push(p.join("resources/runtime/7z.exe"));}}
    candidates.into_iter().find(|p|p.is_file()).and_then(|p|p.canonicalize().ok())
}
pub fn capability()->Value{json!({"available":helper().is_some(),"formats":["zip","7z","rar","iso"],"password_transport":"stdin only","extraction_resume":"Restart the current archive in its staging folder; file copy resumes by offset"})}
fn hidden_command(p:&Path)->Command{let mut c=Command::new(p);#[cfg(windows)]{use std::os::windows::process::CommandExt;c.creation_flags(0x08000000);}c}
pub fn list_archive(p:&Path,password:Option<&str>)->R<Vec<(String,u64)>>{list_archive_controlled(p,password,||Ok(()))}
fn list_archive_controlled(p:&Path,password:Option<&str>,checkpoint:impl FnMut()->R<()>)->R<Vec<(String,u64)>>{
    let exe=helper().ok_or("Archive helper is unavailable")?;
    let mut c=hidden_command(&exe);c.args(["l","-slt","-ba","-sccUTF-8"]).arg("--").arg(p).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out=crate::archive_process::run(&mut c,password,crate::archive_process::LIST_LIMITS,checkpoint)?;
    if !out.status.success(){return Err("Archive cannot be read: a password, missing volume, or supported format may be required".into());}
    let text=std::str::from_utf8(&out.stdout).map_err(|_|"Archive listing is not valid UTF-8")?;
    crate::archive_manifest::parse(&text.replace("\r\n","\n"))
}
pub fn safe_archive_path(p:&str)->R<()> {crate::archive_manifest::safe_path(p)}
type SourceFile=(String,String,i64,String);
fn resource_files(db:&Db,rid:&str)->R<Vec<SourceFile>>{
 let c=db.lock().map_err(|e|e.to_string())?;
 let mut s=c.prepare("SELECT f.id,f.path,f.size,f.mtime FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 AND f.availability='present' ORDER BY f.path").map_err(|e|e.to_string())?;
 let mut files=s.query_map([rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 // Include all known sibling volumes even if an older catalog split them into separate resources.
 let keys:std::collections::BTreeSet<_>=files.iter().filter_map(|(_,p,_,_)|{let path=Path::new(p);let part=path.file_name()?.to_str().and_then(crate::multipart::part)?;Some((path.parent()?.to_string_lossy().to_lowercase(),part.key))}).collect();
 if !keys.is_empty(){let mut s=c.prepare("SELECT id,path,size,mtime,availability FROM files WHERE root_id IN (SELECT f.root_id FROM files f JOIN resource_files rf ON rf.file_id=f.id WHERE rf.resource_id=?1)").map_err(|e|e.to_string())?;
 let rows=s.query_map([rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 for(fid,p,size,mtime,availability)in rows{let path=Path::new(&p);if let Some(part)=path.file_name().and_then(|s|s.to_str()).and_then(crate::multipart::part){let key=(path.parent().unwrap_or(Path::new("")).to_string_lossy().to_lowercase(),part.key);if keys.contains(&key){if availability!="present"{return Err("A known archive volume is missing or unverified. Reconnect and scan its source.".into());}if !files.iter().any(|f|f.0==fid){files.push((fid,p,size,mtime));}}}}
 }files.sort_by(|a,b|a.1.cmp(&b.1));crate::multipart::check_known(&files.iter().map(|f|f.1.clone()).collect::<Vec<_>>())?;Ok(files)
}
fn manifest_digest(rid:&str,files:&[SourceFile])->String{hex::encode(Sha256::digest(serde_json::to_vec(&(rid,files)).expect("Serializable manifest")))}
fn selected_files(rid:&str,files:&[SourceFile],selection:Option<&Selection>)->R<Vec<SourceFile>>{
    let Some(selection)=selection else{return Ok(files.to_vec());};
    if selection.manifest_digest!=manifest_digest(rid,files){return Err("The file list changed. Refresh the preview and select your files again.".into());}
    let ids:std::collections::BTreeSet<_>=selection.file_ids.iter().collect();
    if ids.is_empty()||ids.len()!=selection.file_ids.len(){return Err("Select one or more distinct files to copy".into());}
    let selected:Vec<_>=files.iter().filter(|f|ids.contains(&f.0)).cloned().collect();
    if selected.len()!=ids.len(){return Err("A selected file is not in this resource. Refresh the preview.".into());}
    let paths:std::collections::BTreeSet<_>=selected.iter().map(|f|&f.1).collect();
    for group in crate::multipart::check_known(&files.iter().map(|f|f.1.clone()).collect::<Vec<_>>())?{
        let count=group.members.iter().filter(|p|paths.contains(p)).count();
        if count>0&&count!=group.members.len(){return Err("Select every file in an archive volume group, or deselect the whole group.".into());}
    }Ok(selected)
}
fn resource_base(files:&[SourceFile])->R<PathBuf>{let first=files.first().ok_or("Resource has no available files")?;let mut base=PathBuf::from(&first.1);base.pop();for (_,p,_,_) in files{while !Path::new(p).starts_with(&base){if !base.pop(){return Err("Resource spans incompatible roots".into());}}}Ok(base)}
pub fn preview(db:&Db,rid:&str)->R<Value>{plans::require_resource_available(db,rid)?;let files=resource_files(db,rid)?;let base=resource_base(&files)?;let groups=crate::multipart::check_known(&files.iter().map(|f|f.1.clone()).collect::<Vec<_>>())?;Ok(json!({"source_folder":base,"files":files.iter().map(|(id,path,size,_)|json!({"id":id,"path":path,"size":size,"archive_entry":is_archive_entry(path),"relative":Path::new(path).strip_prefix(&base).map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(|_|path.clone())})).collect::<Vec<_>>(),"bytes":files.iter().map(|f|f.2).sum::<i64>(),"volume_groups":groups,"manifest_digest":manifest_digest(rid,&files)}))}
pub fn create(db:&Db,rid:&str,dest:&str,extract:bool)->R<String>{
    create_selected(db,rid,dest,extract,None)
}
pub fn create_selected(db:&Db,rid:&str,dest:&str,extract:bool,selection:Option<&Selection>)->R<String>{
    {let c=db.lock().map_err(|e|e.to_string())?;crate::roots::require_idle(&c)?;}
    plans::require_resource_available(db,rid)?;
    let dest=scan::canonical(Path::new(dest))?;plans::validate_chain(&dest)?;if !dest.is_dir(){return Err("Choose an existing destination folder".into());}
    let title:String=db.lock().map_err(|e|e.to_string())?.query_row("SELECT title FROM resources WHERE id=?1",[rid],|r|r.get(0)).map_err(|e|e.to_string())?;let files=resource_files(db,rid)?;
    if files.is_empty(){return Err("Resource has no available files".into());}
    let source_file_count=files.len();let selected=selected_files(rid,&files,selection)?;
    if extract&&!selected.iter().any(|f|is_archive_entry(&f.1)){return Err("Select a supported archive to extract, or turn off extraction.".into());}
    let base=resource_base(&files)?;let reuse_existing=dest==scan::canonical(&base)?;
    if reuse_existing&&extract{return Err("Choose a separate destination to extract archives. Existing source files are reused without extraction.".into());}
    let mut members=Vec::new();for(fid,p,size,modified)in selected{let path=scan::canonical(Path::new(&p))?;plans::validate_chain(&path)?;if dest.starts_with(&path){return Err("Destination cannot be inside source data".into());}let metadata=fs::metadata(&path).map_err(|e|e.to_string())?;if !metadata.is_file()||metadata.len()!=size as u64||mtime(&metadata)!=modified{return Err("A selected source file changed. Scan the source and refresh the preview.".into());}let relative=path.strip_prefix(&base).map_err(|e|e.to_string())?.to_string_lossy().to_string();members.push(Member{id:fid,source:path.to_string_lossy().into(),relative,size:size as u64,mtime:modified});}
    if extract{for m in members.iter().filter(|m|is_archive_entry(&m.relative)){let target=extraction_relative(&m.relative);if members.iter().any(|other|relative_inside(&other.relative,&target)){return Err("An extraction folder conflicts with a selected source file. Use copy-only or adjust your selection.".into());}}}
    let final_folder=if reuse_existing{dest.clone()}else{dest.join(format!("{} [{}]",plans::safe_name(&title),&rid[..8]))};if !reuse_existing&&final_folder.exists(){return Err("Destination already exists. Choose another folder.".into());}
    let spec=Spec{resource_id:rid.into(),destination:dest.to_string_lossy().into(),final_folder:final_folder.to_string_lossy().into(),extract,files:members,source_file_count,reuse_existing};let jid=id();let c=db.lock().map_err(|e|e.to_string())?;crate::roots::require_idle(&c)?;c.execute("INSERT INTO jobs(id,kind,state,spec,discovered,created,updated) VALUES(?1,'acquire','queued',?2,?3,?4,?4)",params![jid,serde_json::to_string(&spec).unwrap(),spec.files.len() as i64,now()]).map_err(|e|e.to_string())?;Ok(jid)
}
fn validate_existing(member:&Member)->R<()> {
    let source=Path::new(&member.source);plans::validate_chain(source)?;
    let metadata=fs::metadata(source).map_err(|e|e.to_string())?;
    if !metadata.is_file()||metadata.len()!=member.size||mtime(&metadata)!=member.mtime{return Err("Source changed since this task was queued. Scan the source and create a new task.".into());}Ok(())
}
fn relative_inside(relative:&str,target:&Path)->bool{
    #[cfg(windows)]{Path::new(&relative.to_lowercase()).starts_with(target.to_string_lossy().to_lowercase())}
    #[cfg(not(windows))]{Path::new(relative).starts_with(target)}
}
fn extraction_relative(relative:&str)->PathBuf{let path=Path::new(relative);path.with_file_name(format!("{}-extracted",path.file_name().unwrap_or_default().to_string_lossy()))}
pub(crate) fn control(db:&Db,jid:&str)->R<()> {let c=db.lock().map_err(|e|e.to_string())?;let state:String=c.query_row("SELECT state FROM jobs WHERE id=?1",[jid],|r|r.get(0)).map_err(|e|e.to_string())?;if state=="running"{return Ok(());}let next=if state=="pausing"{"paused"}else if state=="cancelling"{"cancelled"}else{&state};c.execute("UPDATE jobs SET state=?2,updated=?3 WHERE id=?1",params![jid,next,now()]).map_err(|e|e.to_string())?;Err("Task stopped at a safe checkpoint".into())}
fn progress(db:&Db,jid:&str,bytes:u64,processed:usize,current:&str)->R<()> {let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET bytes=?2,processed=?3,current_path=?4,updated=?5 WHERE id=?1",params![jid,bytes as i64,processed as i64,current,now()]).map_err(|e|e.to_string())?;Ok(())}
fn verify_prefix(source:&Path,partial:&Path,bytes:u64)->R<()> {
    let mut a=fs::File::open(source).map_err(|e|e.to_string())?;let mut b=fs::File::open(partial).map_err(|e|e.to_string())?;
    let mut left=bytes;let mut ab=vec![0;1024*1024];let mut bb=vec![0;1024*1024];
    while left>0{let n=left.min(ab.len() as u64)as usize;a.read_exact(&mut ab[..n]).map_err(|e|e.to_string())?;b.read_exact(&mut bb[..n]).map_err(|e|e.to_string())?;if ab[..n]!=bb[..n]{return Err("Staged content differs from its source; create a new transfer".into());}left-=n as u64;}Ok(())
}
#[derive(Serialize,Deserialize)]struct Receipt{files:Vec<(String,u64,String)>}
fn receipt(folder:&Path)->R<Receipt>{
    let mut files=Vec::new();let mut pending=vec![folder.to_path_buf()];
    while let Some(dir)=pending.pop(){plans::validate_chain(&dir)?;for entry in fs::read_dir(dir).map_err(|e|e.to_string())?{let p=entry.map_err(|e|e.to_string())?.path();plans::validate_chain(&p)?;if p.is_dir(){pending.push(p);}else{let(size,hash)=plans::hash(&p)?;files.push((p.strip_prefix(folder).map_err(|e|e.to_string())?.to_string_lossy().into(),size,hash));}}}
    files.sort();Ok(Receipt{files})
}
fn finish(db:&Db,jid:&str,spec:&Spec)->R<()> {
    let selected=if spec.source_file_count>spec.files.len(){format!("Selected {} of {} source files. ",spec.files.len(),spec.source_file_count)}else{String::new()};
    let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET state='completed',message=?2,current_path='',updated=?3 WHERE id=?1",params![jid,format!("{selected}{} {}",if spec.reuse_existing{"Using existing files at"}else{"Ready at"},spec.final_folder),now()]).map_err(|e|e.to_string())?;Ok(())
}
pub fn run(db:Db,jid:String,password:Option<String>){if let Err(e)=inner(&db,&jid,password.as_deref()){crate::diagnostics::record("error","acquire.failed",&e,Some(&jid));if let Ok(c)=db.lock(){let _=c.execute("UPDATE jobs SET state='failed',message=?2,errors=errors+1,updated=?3 WHERE id=?1 AND state='running'",params![jid,e,now()]);}}}
fn inner(db:&Db,jid:&str,password:Option<&str>)->R<()> {
    let spec:Spec={let c=db.lock().map_err(|e|e.to_string())?;let spec:String=c.query_row("SELECT spec FROM jobs WHERE id=?1 AND state='queued'",[jid],|r|r.get(0)).map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET state='running',message='' WHERE id=?1",[jid]).map_err(|e|e.to_string())?;serde_json::from_str(&spec).map_err(|e|e.to_string())?};
    if spec.reuse_existing {
        if spec.extract{return Err("Existing files cannot be extracted in place. Choose a separate destination.".into());}
        plans::require_resource_available(db,&spec.resource_id)?;
        for(index,m)in spec.files.iter().enumerate(){control(db,jid)?;validate_existing(m)?;progress(db,jid,0,index+1,&m.source)?;}
        for group in crate::multipart::check_known(&spec.files.iter().map(|m|m.source.clone()).collect::<Vec<_>>())?{verify_volume_group(db,jid,Path::new(&group.entry),password)?;}
        // Verification can take time. Recheck after it and after a resumed task.
        for m in &spec.files{control(db,jid)?;validate_existing(m)?;}
        control(db,jid)?;return finish(db,jid,&spec);
    }
    let stage=Path::new(&spec.destination).join(format!(".galroon-{}",jid));plans::validate_chain(&stage)?;fs::create_dir_all(&stage).map_err(|e|e.to_string())?;let copied=stage.join("files");let journal=stage.join("publication.json");
    if Path::new(&spec.final_folder).exists(){
        if copied.exists()||!journal.is_file(){return Err("Destination is occupied; no files were overwritten".into());}
        plans::validate_chain(&journal)?;
        let saved:Receipt=serde_json::from_slice(&fs::read(&journal).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        if receipt(Path::new(&spec.final_folder))?.files!=saved.files{return Err("Published files changed; manual review is required".into());}
        return finish(db,jid,&spec);
    }
    fs::create_dir_all(&copied).map_err(|e|e.to_string())?;
    let mut total=0u64;let mut buffer=vec![0u8;1024*1024];
    for(index,m)in spec.files.iter().enumerate(){control(db,jid)?;let source=Path::new(&m.source);plans::validate_chain(source)?;let metadata=fs::metadata(source).map_err(|e|e.to_string())?;if metadata.len()!=m.size||mtime(&metadata)!=m.mtime{return Err("Source changed since the transfer was queued. Create a new task.".into());}
        let target=copied.join(&m.relative);plans::validate_chain(&target)?;if target.exists(){if fs::metadata(&target).map_err(|e|e.to_string())?.len()!=m.size{return Err("Staged file size conflict".into());}verify_prefix(source,&target,m.size)?;total+=m.size;progress(db,jid,total,index+1,&m.relative)?;continue;}
        fs::create_dir_all(target.parent().ok_or("Missing destination parent")?).map_err(|e|e.to_string())?;let partial=stage.join(format!("{}.part",m.id));plans::validate_chain(&partial)?;
        let mut dest=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&partial).map_err(|e|e.to_string())?;let offset=dest.metadata().map_err(|e|e.to_string())?.len();if offset>m.size{return Err("Partial file is larger than its source".into());}
        verify_prefix(source,&partial,offset)?;
        crate::archive_manifest::require_space(m.size-offset,fs2::available_space(&stage).map_err(|e|e.to_string())?)?;
        let mut input=fs::File::open(source).map_err(|e|e.to_string())?;input.seek(SeekFrom::Start(offset)).map_err(|e|e.to_string())?;dest.seek(SeekFrom::Start(offset)).map_err(|e|e.to_string())?;total+=offset;
        loop {control(db,jid)?;let n=input.read(&mut buffer).map_err(|e|e.to_string())?;if n==0{break;}dest.write_all(&buffer[..n]).map_err(|e|e.to_string())?;total+=n as u64;progress(db,jid,total,index,&m.relative)?;}
        dest.sync_all().map_err(|e|e.to_string())?;drop(dest);if fs::metadata(&partial).map_err(|e|e.to_string())?.len()!=m.size||mtime(&fs::metadata(source).map_err(|e|e.to_string())?)!=m.mtime{return Err("Source changed during transfer".into());}plans::no_replace_move(&partial,&target)?;progress(db,jid,total,index+1,&m.relative)?;
    }
    control(db,jid)?;
    for group in crate::multipart::check_known(&spec.files.iter().map(|m|m.relative.clone()).collect::<Vec<_>>())?{verify_volume_group(db,jid,&copied.join(&group.entry),password)?;}
    if spec.extract {
        let archives:Vec<_>=spec.files.iter().filter(|m|is_archive_entry(&m.relative)).collect();
        if archives.is_empty(){return Err("No supported archive entry found. Files are staged; retry as copy-only.".into());}
        for m in archives {control(db,jid)?;let input=copied.join(&m.relative);let members=list_archive_controlled(&input,password,||control(db,jid))?;
            let needed=crate::archive_manifest::total_bytes(members.iter().map(|(_,s)|*s))?;crate::archive_manifest::require_space(needed,fs2::available_space(&stage).map_err(|e|e.to_string())?)?;
            // Preserve the extraction checkpoint location for jobs queued by older versions.
            let relative=if spec.source_file_count==0{PathBuf::from(format!("{}-extracted",plans::safe_name(Path::new(&m.relative).file_stem().and_then(|s|s.to_str()).unwrap_or("Archive"))))}else{extraction_relative(&m.relative)};
            let final_extract=copied.join(relative);
            if final_extract.exists(){continue;}
            // New staging attempt avoids reusing partially extracted trees after interruption.
            let extract_stage=stage.join(format!("extract-{}",id()));fs::create_dir(&extract_stage).map_err(|e|e.to_string())?;
            let exe=helper().ok_or("Archive helper unavailable")?;let mut cmd=hidden_command(&exe);cmd.args(["x","-y","-sccUTF-8","-bd"]).arg(format!("-o{}",extract_stage.display())).arg("--").arg(&input).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
            let mut child=cmd.spawn().map_err(|e|e.to_string())?;if let Some(mut stdin)=child.stdin.take(){if let Some(pw)=password{stdin.write_all(pw.as_bytes()).map_err(|e|e.to_string())?;stdin.write_all(b"\n").map_err(|e|e.to_string())?;}}
            loop{if control(db,jid).is_err(){let _=child.kill();let _=child.wait();return Err("Extraction interrupted; current archive restarts on resume".into());}if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{if !status.success(){return Err("Extraction failed. Check password, volumes and available space.".into());}break;}progress(db,jid,total,spec.files.len(),&format!("Extracting {}",m.relative))?;std::thread::sleep(std::time::Duration::from_millis(200));}
            for(p,size)in members{let p=extract_stage.join(p);plans::validate_chain(&p)?;let metadata=fs::metadata(&p).map_err(|_|"Archive extraction did not produce the expected files")?;if !metadata.is_file()||metadata.len()!=size{return Err("Archive extraction did not produce the expected file sizes".into());}}
            plans::no_replace_move(&extract_stage,&final_extract)?;
        }
    }
    control(db,jid)?;progress(db,jid,total,spec.files.len(),"Verifying publication")?;
    let publication=receipt(&copied)?;plans::validate_chain(&journal)?;
    let mut record=fs::File::create(&journal).map_err(|e|e.to_string())?;record.write_all(&serde_json::to_vec(&publication).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;record.sync_all().map_err(|e|e.to_string())?;drop(record);
    control(db,jid)?;plans::no_replace_move(&copied,Path::new(&spec.final_folder))?;
    finish(db,jid,&spec)
}
pub fn is_archive_entry(s:&str)->bool{let name=Path::new(s).file_name().and_then(|s|s.to_str()).unwrap_or(s);if let Some(p)=crate::multipart::part(name){return p.index==1;}let name=name.to_lowercase();[".zip",".7z",".iso"].iter().any(|ext|name.ends_with(ext))}
fn verify_volume_group(db:&Db,jid:&str,path:&Path,password:Option<&str>)->R<()>{
 let exe=helper().ok_or("Archive helper is required to verify multipart completeness")?;let mut cmd=hidden_command(&exe);cmd.args(["t","-bd","-y"]).arg("--").arg(path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());let mut child=cmd.spawn().map_err(|e|e.to_string())?;
 if let Some(mut input)=child.stdin.take(){if let Some(pw)=password{input.write_all(pw.as_bytes()).map_err(|e|e.to_string())?;input.write_all(b"\n").map_err(|e|e.to_string())?;}}
 loop{if control(db,jid).is_err(){let _=child.kill();let _=child.wait();return Err("Volume verification interrupted; it restarts on resume".into());}if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{if !status.success(){return Err("Archive volume verification failed. Check for a missing last volume, damaged data or a required password.".into());}break;}let c=db.lock().map_err(|e|e.to_string())?;c.execute("UPDATE jobs SET current_path=?2,updated=?3 WHERE id=?1",params![jid,format!("Verifying volume group {}",path.file_name().unwrap_or_default().to_string_lossy()),now()]).map_err(|e|e.to_string())?;drop(c);std::thread::sleep(std::time::Duration::from_millis(150));}Ok(())
}
#[cfg(test)]mod tests{use super::*;#[test]fn archive_paths_cannot_escape(){for p in ["../escape","C:\\escape","/absolute","a/../b","a:stream","folder./file"]{assert!(safe_archive_path(p).is_err(),"{p}");}assert!(safe_archive_path("Game/data/file.bin").is_ok());}#[test]fn first_volume_selection(){assert!(is_archive_entry("game.part01.rar"));assert!(!is_archive_entry("game.part02.rar"));assert!(is_archive_entry("game.7z.001"));assert!(!is_archive_entry("game.7z.002"));}}

#[cfg(test)]mod selection_tests{
 use super::*;
 fn fixture(names:&[&str])->(tempfile::TempDir,Db,String,PathBuf,PathBuf){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("source");let destination=t.path().join("destination");fs::create_dir_all(&destination).unwrap();
  for name in names{let p=root.join("Game").join(name);fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,format!("fixture: {name}")).unwrap();}
  let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Selection fixture')",[root.to_str().unwrap()]).unwrap();
  rescan(&db);let rid=db.lock().unwrap().query_row("SELECT id FROM resources WHERE title='Game'",[],|r|r.get::<_,String>(0)).unwrap();(t,db,rid,root,destination)
 }
 fn rescan(db:&Db){let job=scan::create(db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),job);}
 fn selection(db:&Db,rid:&str,suffixes:&[&str])->Selection{let p=preview(db,rid).unwrap();Selection{manifest_digest:p["manifest_digest"].as_str().unwrap().into(),file_ids:p["files"].as_array().unwrap().iter().filter(|f|suffixes.iter().any(|s|f["path"].as_str().unwrap().ends_with(s))).map(|f|f["id"].as_str().unwrap().into()).collect()}}
 #[test]fn same_source_destination_reuses_selected_files_without_staging(){
  let(_t,db,rid,root,_dest)=fixture(&["base.bin","extras/readme.txt"]);let source=root.join("Game");let before=receipt(&source).unwrap().files;
  let p=preview(&db,&rid).unwrap();assert_eq!(scan::canonical(Path::new(p["source_folder"].as_str().unwrap())).unwrap(),scan::canonical(&source).unwrap());
  let select=selection(&db,&rid,&["readme.txt"]);let job=create_selected(&db,&rid,source.join(".").to_str().unwrap(),false,Some(&select)).unwrap();run(db.clone(),job.clone(),None);
  let(state,message,processed,bytes,spec):(String,String,i64,i64,String)=db.lock().unwrap().query_row("SELECT state,message,processed,bytes,spec FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
  assert_eq!(state,"completed","{message}");assert!(message.contains("Selected 1 of 2"));assert!(message.contains("Using existing files"));assert_eq!((processed,bytes),(1,0));let spec:Spec=serde_json::from_str(&spec).unwrap();assert!(spec.reuse_existing);assert_eq!(Path::new(&spec.final_folder),scan::canonical(&source).unwrap());
  assert_eq!(receipt(&source).unwrap().files,before);assert!(!source.join(format!(".galroon-{job}")).exists());
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);assert_eq!(receipt(&source).unwrap().files,before);
  assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
 }
 #[test]fn reused_files_are_rechecked_after_queue_and_resume(){
  let(_t,db,rid,root,_dest)=fixture(&["base.bin","readme.txt"]);let source=root.join("Game");let select=selection(&db,&rid,&["readme.txt"]);
  let job=create_selected(&db,&rid,source.to_str().unwrap(),false,Some(&select)).unwrap();fs::write(source.join("readme.txt"),b"changed after queue").unwrap();run(db.clone(),job.clone(),None);
  let(state,message):(String,String)=db.lock().unwrap().query_row("SELECT state,message FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(state,"failed");assert!(message.contains("Source changed"));assert!(!source.join(format!(".galroon-{job}")).exists());
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get::<_,String>(0)).unwrap(),"failed");
 }
 #[test]fn in_place_extraction_is_rejected_and_old_copy_jobs_keep_copy_mode(){
  let(_t,db,rid,root,dest)=fixture(&["game.zip"]);assert!(create(&db,&rid,root.join("Game").to_str().unwrap(),true).unwrap_err().contains("separate destination"));
  let job=create(&db,&rid,dest.to_str().unwrap(),false).unwrap();let raw:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[job],|r|r.get(0)).unwrap();let mut v:Value=serde_json::from_str(&raw).unwrap();v.as_object_mut().unwrap().remove("reuse_existing");assert!(!serde_json::from_value::<Spec>(v).unwrap().reuse_existing);
 }
 #[cfg(windows)]
 #[test]fn windows_locked_partial_fails_without_publication_and_resumes_after_unlock(){
  use std::os::windows::fs::OpenOptionsExt;
  let(_t,db,rid,root,dest)=fixture(&["base.bin"]);
  let original=receipt(&root).unwrap().files;
  let job=create(&db,&rid,dest.to_str().unwrap(),false).unwrap();
  let raw:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();
  let spec:Spec=serde_json::from_str(&raw).unwrap();
  let stage=dest.join(format!(".galroon-{job}"));fs::create_dir_all(&stage).unwrap();
  let partial=stage.join(format!("{}.part",spec.files[0].id));
  let source=fs::read(&spec.files[0].source).unwrap();fs::write(&partial,&source[..5]).unwrap();
  // A real Windows sharing violation, without changing ACLs or filling a disk.
  let lock=fs::OpenOptions::new().read(true).share_mode(0).open(&partial).unwrap();
  run(db.clone(),job.clone(),None);
  let(state,errors):(String,i64)=db.lock().unwrap().query_row("SELECT state,errors FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!((state.as_str(),errors),("failed",1));assert!(!Path::new(&spec.final_folder).exists());
  drop(lock);assert_eq!(fs::read(&partial).unwrap(),source[..5]);assert_eq!(receipt(&root).unwrap().files,original);
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);
  assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
  assert_eq!(fs::read(Path::new(&spec.final_folder).join(&spec.files[0].relative)).unwrap(),source);assert_eq!(receipt(&root).unwrap().files,original);
 }
 #[cfg(windows)]
 #[test]
 fn windows_midstream_write_failure_retains_prefix_and_resumes() {
  use std::os::windows::io::AsRawHandle;
  use windows_sys::Win32::Storage::FileSystem::{LockFile,UnlockFile};
  let (_t,db,rid,root,destination)=fixture(&["payload.bin"]);
  let chunk=1024*1024usize;
  let data:Vec<u8>=(0..3*chunk+173).map(|n|((n*31+n/chunk)%251) as u8).collect();
  fs::write(root.join("Game/payload.bin"),&data).unwrap();rescan(&db);
  let original=receipt(&root).unwrap().files;
  let job=create(&db,&rid,destination.to_str().unwrap(),false).unwrap();
  let raw:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();
  let spec:Spec=serde_json::from_str(&raw).unwrap();
  let stage=destination.join(format!(".galroon-{job}"));fs::create_dir_all(&stage).unwrap();
  let partial=stage.join(format!("{}.part",spec.files[0].id));
  let lock=fs::OpenOptions::new().create_new(true).read(true).write(true).open(&partial).unwrap();
  // Windows range locks can extend beyond EOF. The real transfer may write its
  // first MiB, but its second write must fail through a different file handle.
  assert_ne!(unsafe {LockFile(lock.as_raw_handle(),chunk as u32,0,chunk as u32,0)},0,"{}",std::io::Error::last_os_error());
  run(db.clone(),job.clone(),None);
  let (state,errors,bytes):(String,i64,i64)=db.lock().unwrap().query_row("SELECT state,errors,bytes FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
  assert_eq!((state.as_str(),errors,bytes),("failed",1,chunk as i64));
  assert_eq!(fs::metadata(&partial).unwrap().len(),chunk as u64);
  assert!(!Path::new(&spec.final_folder).exists());assert!(!stage.join("publication.json").exists());
  assert_ne!(unsafe {UnlockFile(lock.as_raw_handle(),chunk as u32,0,chunk as u32,0)},0);drop(lock);
  assert_eq!(fs::read(&partial).unwrap(),data[..chunk]);
  assert_eq!(receipt(&root).unwrap().files,original);
  drop(db);let db=crate::db::open(&_t.path().join("state/library.sqlite")).unwrap();
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();
  run(db.clone(),job.clone(),None);
  assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
  assert_eq!(fs::read(Path::new(&spec.final_folder).join(&spec.files[0].relative)).unwrap(),data);
  assert_eq!(receipt(&root).unwrap().files,original);assert!(!partial.exists());
 }
 #[cfg(windows)]
 #[test]
 fn windows_queued_source_and_destination_junctions_fail_without_escape() {
  struct Junction(PathBuf);
  impl Drop for Junction {fn drop(&mut self){let _=fs::remove_dir(&self.0);}}
  for replace_source in [true,false] {
   let(t,db,rid,root,destination)=fixture(&["base.bin"]);
   let original=receipt(&root).unwrap().files;
   let job=create(&db,&rid,destination.to_str().unwrap(),false).unwrap();
   let raw:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();
   let spec:Spec=serde_json::from_str(&raw).unwrap();
   let outside=t.path().join("outside");fs::create_dir(&outside).unwrap();fs::write(outside.join("sentinel.txt"),b"Keep generated sentinel").unwrap();
   let outside_before=receipt(&outside).unwrap().files;
   let replaced=if replace_source {root.join("Game")} else {destination.clone()};
   let retained=t.path().join("retained-original");fs::rename(&replaced,&retained).unwrap();
   // A real NTFS junction needs no symlink privilege. Paths are process env
   // values, never interpolated into executable PowerShell source.
   let result=hidden_command(Path::new("powershell.exe"))
    .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GALROON_TEST_JUNCTION -Target $env:GALROON_TEST_JUNCTION_TARGET | Out-Null"])
    .env("GALROON_TEST_JUNCTION",&replaced).env("GALROON_TEST_JUNCTION_TARGET",&outside).output().unwrap();
   assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
   let junction=Junction(replaced.clone());
   assert!(scan::link(&fs::symlink_metadata(&replaced).unwrap()));
   run(db.clone(),job.clone(),None);
   let(state,message):(String,String)=db.lock().unwrap().query_row("SELECT state,message FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
   assert_eq!(state,"failed");assert!(message.contains("Linked paths"),"{message}");
   assert!(!Path::new(&spec.final_folder).exists());
   assert_eq!(receipt(&outside).unwrap().files,outside_before);
   // Nonrecursive removal unlinks just the junction. Restore the exact
   // generated directory before retrying the durable task.
   fs::remove_dir(&replaced).unwrap();drop(junction);fs::rename(&retained,&replaced).unwrap();
   assert_eq!(receipt(&root).unwrap().files,original);
   db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);
   assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
   assert_eq!(receipt(&root).unwrap().files,original);assert_eq!(receipt(&outside).unwrap().files,outside_before);
   assert_eq!(fs::read(Path::new(&spec.final_folder).join(&spec.files[0].relative)).unwrap(),fs::read(&spec.files[0].source).unwrap());
  }
 }
 #[test]fn publication_journal_write_failure_retains_verified_staging_for_retry(){
  let(_t,db,rid,root,dest)=fixture(&["base.bin","extras/readme.txt"]);let original=receipt(&root).unwrap().files;
  let job=create(&db,&rid,dest.to_str().unwrap(),false).unwrap();
  let raw:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();let spec:Spec=serde_json::from_str(&raw).unwrap();
  let stage=dest.join(format!(".galroon-{job}"));let journal=stage.join("publication.json");
  // A directory at the journal path causes a real filesystem create/write failure.
  fs::create_dir_all(&journal).unwrap();run(db.clone(),job.clone(),None);
  assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"failed");
  assert!(!Path::new(&spec.final_folder).exists());let staged=receipt(&stage.join("files")).unwrap().files;assert_eq!(staged.len(),2);
  assert_eq!(receipt(&root).unwrap().files,original);fs::remove_dir(&journal).unwrap();
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);
  assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[&job],|r|r.get::<_,String>(0)).unwrap(),"completed");
  assert_eq!(receipt(Path::new(&spec.final_folder)).unwrap().files,staged);assert!(journal.is_file());assert_eq!(receipt(&root).unwrap().files,original);
 }
 #[test]fn selected_copy_preserves_relative_layout_and_originals(){
  let(_t,db,rid,root,dest)=fixture(&["base.bin","extras/readme.txt","extras/art.bin"]);let select=selection(&db,&rid,&["readme.txt"]);
  let job=create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&select)).unwrap();run(db.clone(),job.clone(),None);
  let(state,message,spec):(String,String,String)=db.lock().unwrap().query_row("SELECT state,message,spec FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(state,"completed","{message}");assert!(message.contains("Selected 1 of 3"));let spec:Spec=serde_json::from_str(&spec).unwrap();
  let output=Path::new(&spec.final_folder);assert_eq!(fs::read(output.join("extras/readme.txt")).unwrap(),fs::read(root.join("Game/extras/readme.txt")).unwrap());assert!(!output.join("base.bin").exists());assert!(!output.join("extras/art.bin").exists());for name in ["base.bin","extras/readme.txt","extras/art.bin"]{assert_eq!(fs::read(root.join("Game").join(name)).unwrap(),format!("fixture: {name}").as_bytes());}
  db.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();run(db.clone(),job.clone(),None);assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get::<_,String>(0)).unwrap(),"completed");
 }
 #[test]fn whole_volume_groups_and_valid_distinct_ids_are_required(){
  let(_t,db,rid,_root,dest)=fixture(&["game.7z.001","game.7z.002","readme.txt"]);let single=selection(&db,&rid,&["game.7z.001"]);assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&single)).unwrap_err().contains("every file"));
  for ids in [vec![],vec!["unknown".into()],vec![single.file_ids[0].clone(),single.file_ids[0].clone()]]{let s=Selection{file_ids:ids,manifest_digest:single.manifest_digest.clone()};assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&s)).is_err());}
  let both=selection(&db,&rid,&["game.7z.001","game.7z.002"]);let job=create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&both)).unwrap();let spec:String=db.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[job],|r|r.get(0)).unwrap();assert_eq!(serde_json::from_str::<Spec>(&spec).unwrap().files.len(),2);
 }
 #[test]fn preview_changes_and_unscanned_source_changes_require_review(){
  let(_t,db,rid,root,dest)=fixture(&["base.bin","readme.txt"]);let old=selection(&db,&rid,&["readme.txt"]);fs::write(root.join("Game/readme.txt"),b"changed").unwrap();assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&old)).unwrap_err().contains("source file changed"));
  rescan(&db);assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&old)).unwrap_err().contains("file list changed"));
  let new=selection(&db,&rid,&["readme.txt"]);fs::write(root.join("Game/new.bin"),b"new").unwrap();rescan(&db);assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&new)).is_err());
  let fresh=selection(&db,&rid,&["readme.txt"]);assert!(create_selected(&db,&rid,dest.to_str().unwrap(),true,Some(&fresh)).unwrap_err().contains("supported archive"));
  fs::remove_file(root.join("Game/base.bin")).unwrap();rescan(&db);assert!(preview(&db,&rid).is_err());assert!(create_selected(&db,&rid,dest.to_str().unwrap(),false,Some(&fresh)).is_err());
 }
 #[test]fn extraction_folders_preserve_location_and_do_not_hide_selected_files(){
  assert_ne!(extraction_relative("base/game.zip"),extraction_relative("extras/game.zip"));assert_ne!(extraction_relative("game.zip"),extraction_relative("game.7z"));
  let(_t,db,rid,_root,dest)=fixture(&["game.zip","game.zip-extracted/readme.txt"]);let s=selection(&db,&rid,&["game.zip","readme.txt"]);assert!(create_selected(&db,&rid,dest.to_str().unwrap(),true,Some(&s)).unwrap_err().contains("conflicts"));
 }
}



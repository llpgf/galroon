//! Catalog-only source relocation. File identities and manual bindings survive a path change.
use crate::{db::{Db,id,now},plans,scan};
use rusqlite::{Connection,params};
use serde::{Serialize,Deserialize};
use std::path::{Path,PathBuf,Component};
use sha2::{Digest,Sha256};
type R<T>=Result<T,String>;
#[derive(Serialize,Deserialize)]pub struct Preview{pub root_id:String,pub old_path:String,pub new_path:String,pub files:usize,pub found:usize,pub missing:usize,pub changed:usize,pub jobs:usize,pub plans:usize,pub digest:String}
fn sql<T>(r:rusqlite::Result<T>)->R<T>{r.map_err(|e|e.to_string())}
pub fn require_idle(c:&Connection)->R<()>{
 let n:i64=sql(c.query_row("SELECT (SELECT count(*) FROM jobs WHERE kind!='artwork' AND state IN ('queued','running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get(0)))?;
 if n>0{return Err("Pause running tasks and wait for file operations to finish first".into());}Ok(())
}
fn relative(value:&str)->R<PathBuf>{let p=Path::new(value);if p.as_os_str().is_empty()||p.components().any(|c|!matches!(c,Component::Normal(_)|Component::CurDir)){return Err("Stored file has an unsafe relative path".into());}Ok(p.into())}
/// Joins a stored relative path one component at a time. A catalog moved from macOS stores "dir/file";
/// pushed onto a Windows verbatim root (\\?\UNC\...) as one piece, the "/" would not be a separator.
fn under(target:&Path,rel:&str)->R<PathBuf>{let mut out=target.to_path_buf();for part in relative(rel)?.components(){if let Component::Normal(name)=part{out.push(name);}}Ok(out)}
/// The stored name, or its NFC form when only that exists. macOS reports SMB names decomposed (NFD) while the
/// share keeps them composed, and Windows compares names byte for byte. Returns the relative path that exists.
fn locate(target:&Path,rel:&str)->R<(PathBuf,String)>{
 use unicode_normalization::UnicodeNormalization;
 let raw=under(target,rel)?;if std::fs::symlink_metadata(&raw).is_ok(){return Ok((raw,rel.into()));}
 let composed:String=rel.nfc().collect();if composed!=rel{let nfc=under(target,&composed)?;if std::fs::symlink_metadata(&nfc).is_ok(){return Ok((nfc,composed));}}
 Ok((raw,rel.into()))
}
fn snapshot(c:&Connection,rid:&str,target:&Path)->R<(Preview,Vec<(String,String,String)>)>{
 require_idle(c)?;plans::validate_chain(target)?;let target=scan::canonical(target)?;if !target.is_dir(){return Err("Choose an existing source folder".into());}
 let old:String=sql(c.query_row("SELECT path FROM roots WHERE id=?1",[rid],|r|r.get(0)))?;
 if Path::new(&old)==target{return Err("This source already uses that folder".into());}
 let mut roots=sql(c.prepare("SELECT path FROM roots WHERE id!=?1"))?;
 for row in sql(roots.query_map([rid],|r|r.get::<_,String>(0)))?{let other=PathBuf::from(sql(row)?);let other=other.canonicalize().unwrap_or(other);if target.starts_with(&other)||other.starts_with(&target){return Err("The new folder overlaps another source".into());}}
 let mut s=sql(c.prepare("SELECT id,path,relative_path,size FROM files WHERE root_id=?1 ORDER BY id"))?;
 let rows=sql(sql(s.query_map([rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;
 let mut mapped=Vec::new();let(mut found,mut missing,mut changed)=(0,0,0);let mut evidence=Vec::new();
 for (fid,old_file,rel,size) in rows{let (new,found_rel)=locate(&target,&rel)?;plans::validate_chain(&new)?;let status=match std::fs::symlink_metadata(&new){Ok(m) if m.is_file()&&!scan::link(&m)&&m.len()==size as u64=>{found+=1;"found"},Ok(_)=>{changed+=1;"changed"},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{missing+=1;"missing"},Err(e)=>return Err(e.to_string())};evidence.push(serde_json::json!([fid,old_file,rel,size,status,found_rel]));mapped.push((fid,new.to_string_lossy().into_owned(),found_rel));}
 let mut s=sql(c.prepare("SELECT id,state FROM jobs WHERE state NOT IN ('completed','superseded') ORDER BY id"))?;let jobs=sql(sql(s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;
 let mut s=sql(c.prepare("SELECT id,state,digest FROM plans WHERE state NOT IN ('completed','superseded') ORDER BY id"))?;let plans=sql(sql(s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;
 let payload=serde_json::json!([rid,old,target,evidence,jobs,plans]);let digest=hex::encode(Sha256::digest(payload.to_string().as_bytes()));
 Ok((Preview{root_id:rid.into(),old_path:old,new_path:target.to_string_lossy().into(),files:mapped.len(),found,missing,changed,jobs:jobs.len(),plans:plans.len(),digest},mapped))
}
pub fn preview(db:&Db,rid:&str,target:&Path)->R<Preview>{let c=db.lock().map_err(|e|e.to_string())?;Ok(snapshot(&c,rid,target)?.0)}
pub fn remap(db:&Db,rid:&str,target:&Path,digest:&str)->R<Preview>{
 let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;let (preview,mapped)=snapshot(&tx,rid,target)?;
 if preview.digest!=digest{return Err("Source mapping changed since preview. Review it again.".into());}
 for(fid,path,rel)in mapped{sql(tx.execute("UPDATE files SET path=?2,relative_path=?3,availability='unverified',hash=NULL WHERE id=?1",params![fid,path,rel]))?;}
 // Resource names follow the same rule, so a later scan of the new location finds the same resources.
 let resources={let mut s=sql(tx.prepare("SELECT id,relative_path FROM resources WHERE root_id=?1 AND manual_group=0"))?;let rows=sql(sql(s.query_map([rid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;rows};
 for(id,rel)in resources{if let Ok((_,found))=locate(Path::new(&preview.new_path),&rel){if found!=rel{let _=tx.execute("UPDATE resources SET relative_path=?2 WHERE id=?1",params![id,found]);}}}
 sql(tx.execute("UPDATE roots SET path=?2 WHERE id=?1",params![rid,preview.new_path]))?;
 sql(tx.execute("UPDATE settings SET value=?2 WHERE key=?3 AND value=?1",params![preview.old_path,preview.new_path,crate::managed_root::KEY]))?;
 // Old manifests are immutable history. Never silently rewrite their approved destinations.
 sql(tx.execute("UPDATE jobs SET state='superseded',message='Source mapping changed. Start a new task; previous staging is retained.',updated=?1 WHERE state NOT IN ('completed','superseded')",[now()]))?;
 sql(tx.execute("UPDATE plans SET state='superseded',approved=NULL WHERE state NOT IN ('completed','superseded')",[]))?;
 let rows={let mut s=sql(tx.prepare("SELECT id,original,current FROM quarantine"))?;let rows=sql(sql(s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))))?.collect::<rusqlite::Result<Vec<_>>>())?;rows};
 let rewrite=|p:&str|->String{Path::new(p).strip_prefix(&preview.old_path).map(|rel|Path::new(&preview.new_path).join(rel).to_string_lossy().into_owned()).unwrap_or_else(|_|p.into())};
 for(q,original,current)in rows{sql(tx.execute("UPDATE quarantine SET original=?2,current=?3 WHERE id=?1",params![q,rewrite(&original),rewrite(&current)]))?;}
 sql(tx.execute("INSERT INTO root_history(id,root_id,old_path,new_path,created) VALUES(?1,?2,?3,?4,?5)",params![id(),rid,preview.old_path,preview.new_path,now()]))?;sql(tx.commit())?;Ok(preview)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn stored_relative_paths_join_by_component(){
  let root=Path::new("root");assert_eq!(under(root,"a/b.rar").unwrap(),root.join("a").join("b.rar"));assert!(under(root,"../x").is_err());
  // A name stored decomposed (NFD) is found under its composed form, and the found form is reported.
  let t=tempfile::tempdir().unwrap();let composed="ガ.rar";std::fs::write(t.path().join(composed),b"x").unwrap();
  let decomposed:String=unicode_normalization::UnicodeNormalization::nfd(composed).collect();
  let (path,found)=locate(t.path(),&decomposed).unwrap();assert!(path.exists());assert!(found==composed||found==decomposed);
  #[cfg(windows)]{let verbatim=Path::new(r"\\?\UNC\nas\share");assert_eq!(under(verbatim,"dir/file.rar").unwrap().to_string_lossy(),r"\\?\UNC\nas\share\dir\file.rar");}
 }
 #[test]fn remap_keeps_bindings_requires_review_and_reindex(){
 let t=tempfile::tempdir().unwrap();let source=t.path().join("old");let target=t.path().join("new");std::fs::create_dir(&source).unwrap();std::fs::create_dir(&target).unwrap();std::fs::write(source.join("story.zip"),b"original").unwrap();std::fs::write(target.join("story.zip"),b"original").unwrap();let db=crate::db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Story')",[source.to_string_lossy()]).unwrap();let j=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),j);
 let fid:String=db.lock().unwrap().query_row("SELECT id FROM files",[],|r|r.get(0)).unwrap();let rid:String=db.lock().unwrap().query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();db.lock().unwrap().execute_batch("INSERT INTO works(id,title,original_title,notes) VALUES('w','Title','Title','Keep these notes'); UPDATE resources SET work_id='w'; INSERT INTO plans(id,kind,state,items,digest,created,approved) VALUES('p','organize','approved','[]','old',1,1);").unwrap();
 let preview=preview(&db,"r",&target).unwrap();assert_eq!(preview.found,1);assert!(remap(&db,"r",&target,"wrong").is_err());remap(&db,"r",&target,&preview.digest).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT availability FROM files WHERE id=?1",[&fid],|r|r.get::<_,String>(0)).unwrap(),"unverified");assert_eq!(c.query_row("SELECT state FROM plans WHERE id='p'",[],|r|r.get::<_,String>(0)).unwrap(),"superseded");drop(c);
 let j=scan::create(&db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),j);let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM files",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT work_id FROM resources WHERE id=?1",[rid],|r|r.get::<_,String>(0)).unwrap(),"w");assert_eq!(c.query_row("SELECT availability FROM files WHERE id=?1",[fid],|r|r.get::<_,String>(0)).unwrap(),"present");assert_eq!(std::fs::read(source.join("story.zip")).unwrap(),b"original");assert_eq!(std::fs::read(target.join("story.zip")).unwrap(),b"original");
 }
}

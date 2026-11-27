//! Core-level active collection pointer (`active-library.json`), shared by Unix Core and desktop controller.
use crate::{db,plans};
use serde_json::{Value,json};
use std::{fs,io::Write,path::{Component,Path,PathBuf}};
type R<T>=Result<T,String>;
pub const FILE:&str="active-library.json";
const MAX_POINTER_BYTES:u64=1024*1024;

pub fn pointer_path(data_root:&Path)->PathBuf{data_root.join(FILE)}

fn check_pointer_file(pointer:&Path)->R<bool>{
 plans::validate_chain(pointer)?;
 match fs::symlink_metadata(pointer){
  Ok(m)=>{
   if m.file_type().is_symlink()||!m.is_file(){return Err("Collection pointer must be a regular file, not a symbolic link".into());}
   if m.len()>MAX_POINTER_BYTES{return Err("Collection pointer is too large".into());}
   Ok(true)
  }
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),
  Err(e)=>Err(e.to_string())
 }
}

fn validate_target(data_root:&Path,target:&Path)->R<PathBuf>{
 if !target.is_absolute()||target.components().any(|c|matches!(c,Component::ParentDir)){return Err("Collection pointer must be an absolute path without parent segments".into());}
 plans::validate_chain(data_root)?;plans::validate_chain(target)?;
 let meta=fs::symlink_metadata(target).map_err(|e|e.to_string())?;
 if meta.file_type().is_symlink()||!meta.is_dir(){return Err("Collection pointer target must be a real directory".into());}
 let base=data_root.canonicalize().map_err(|e|e.to_string())?;
 let dir=target.canonicalize().map_err(|e|e.to_string())?;
 if !dir.starts_with(&base){return Err("Collection pointer is outside its local data directory".into());}
 plans::validate_chain(&dir)?;
 Ok(dir)
}

/// Resolves the active collection directory for `data_root`.
/// Returns `data_root` unchanged when no `active-library.json` exists.
pub fn resolve(data_root:&Path)->R<PathBuf>{
 plans::validate_chain(data_root)?;
 let pointer=pointer_path(data_root);
 if !check_pointer_file(&pointer)?{return Ok(data_root.to_path_buf());}
 let v:Value=serde_json::from_slice(&fs::read(&pointer).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let current=v["current"].as_str().ok_or("Invalid collection pointer")?;
 validate_target(data_root,Path::new(current))
}

fn write_pointer(data_root:&Path,config:&Value)->R<()>{
 let pointer=pointer_path(data_root);check_pointer_file(&pointer)?;
 let pending=data_root.join(format!("active-library-{}.json",db::id()));plans::validate_chain(&pending)?;
 let result=(||{
  let mut options=fs::OpenOptions::new();options.create_new(true).write(true);
  #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
  let mut file=options.open(&pending).map_err(|e|e.to_string())?;
  file.write_all(&serde_json::to_vec_pretty(config).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
  drop(file);
  check_pointer_file(&pointer)?;
  fs::rename(&pending,&pointer).map_err(|e|e.to_string())
 })();
 if result.is_err(){let _=fs::remove_file(&pending);}
 result
}

/// Atomically switches `active-library.json` to `new` (which must live inside `data_root`),
/// appending `previous` to the `previous` history array.
pub fn commit(data_root:&Path,new:&Path,previous:Value)->R<PathBuf>{
 let dir=validate_target(data_root,new)?;
 let pointer=pointer_path(data_root);
 let mut config=if check_pointer_file(&pointer)?{serde_json::from_slice::<Value>(&fs::read(&pointer).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?}else{json!({"previous":[]})};
 config["previous"].as_array_mut().ok_or("Invalid collection history")?.push(previous);
 config["current"]=json!(dir);
 write_pointer(data_root,&config)?;
 Ok(dir)
}

/// Atomically reverts `active-library.json` back to `previous_dir` after a failed relaunch.
pub fn rollback(data_root:&Path,previous_dir:&Path)->R<PathBuf>{
 let dir=validate_target(data_root,previous_dir)?;
 let pointer=pointer_path(data_root);
 let mut config=if check_pointer_file(&pointer)?{serde_json::from_slice::<Value>(&fs::read(&pointer).map_err(|e|e.to_string())?).unwrap_or_else(|_|json!({"previous":[]}))}else{json!({"previous":[]})};
 if !config["previous"].is_array(){config["previous"]=json!([]);}
 config["current"]=json!(dir);
 write_pointer(data_root,&config)?;
 Ok(dir)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn resolve_defaults_to_data_root_and_commit_updates_atomically(){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("data");fs::create_dir(&root).unwrap();
  assert_eq!(resolve(&root).unwrap(),root);
  let next=root.join("collections").join("c1");fs::create_dir_all(&next).unwrap();
  let committed=commit(&root,&next,json!({"path":root,"replaced_at":1})).unwrap();
  assert_eq!(committed,next.canonicalize().unwrap());assert_eq!(resolve(&root).unwrap(),committed);
  assert!(!fs::read_dir(&root).unwrap().any(|e|e.unwrap().file_name().to_string_lossy().starts_with("active-library-")));
  let rolled=rollback(&root,&root).unwrap();assert_eq!(rolled,root.canonicalize().unwrap());assert_eq!(resolve(&root).unwrap(),rolled);
 }
 #[test]fn rejects_escape_relative_and_missing_targets(){
  let t=tempfile::tempdir().unwrap();let root=t.path().join("data");let outside=t.path().join("outside");fs::create_dir(&root).unwrap();fs::create_dir(&outside).unwrap();
  assert!(commit(&root,&outside,json!({})).is_err());assert!(!pointer_path(&root).exists());
  for bad in [outside.to_string_lossy().into_owned(),"collections/rel".into(),format!("{}/../outside",root.display()),root.join("missing").to_string_lossy().into_owned()]{
   fs::write(pointer_path(&root),json!({"current":bad,"previous":[]}).to_string()).unwrap();
   assert!(resolve(&root).is_err(),"{bad}");
  }
 }
 #[cfg(unix)]#[test]fn rejects_symlink_pointer_and_symlink_target(){
  use std::os::unix::fs::symlink;
  let t=tempfile::tempdir().unwrap();let root=t.path().join("data");let outside=t.path().join("outside");fs::create_dir(&root).unwrap();fs::create_dir(&outside).unwrap();
  let link_target=root.join("linked-collection");symlink(&outside,&link_target).unwrap();
  assert!(commit(&root,&link_target,json!({})).is_err());
  fs::write(pointer_path(&root),json!({"current":link_target,"previous":[]}).to_string()).unwrap();assert!(resolve(&root).is_err());
  fs::remove_file(pointer_path(&root)).unwrap();
  let real_col=root.join("collections").join("c1");fs::create_dir_all(&real_col).unwrap();
  let fake=outside.join("active-library.json");fs::write(&fake,json!({"current":real_col,"previous":[]}).to_string()).unwrap();
  symlink(&fake,pointer_path(&root)).unwrap();
  assert!(resolve(&root).is_err());assert!(commit(&root,&real_col,json!({})).is_err());
 }
}

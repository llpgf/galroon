//! Owner-only token files for Cores without a system credential store (Linux/NAS).
//! Same protection level as lan-key.pem in device_dir; never part of catalog backups.
use std::{fs,io::Write,path::{Path,PathBuf}};

fn path(dir:&Path,service:&str,id:&str)->Result<PathBuf,String>{
  let safe=|s:&str|!s.is_empty()&&s.len()<=64&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_');
  if !safe(service)||!safe(id){return Err("Invalid credential reference".into());}
  Ok(checked(&dir.join("credentials"))?.join(format!("{service}.{id}")))
}
// The folder and its files must be real entries, not symlinks redirecting tokens elsewhere.
fn checked(path:&Path)->Result<&Path,String>{
  match fs::symlink_metadata(path){Ok(m) if m.file_type().is_symlink()=>Err("Credential storage must not be a symlink".into()),Ok(_)=>Ok(path),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(path),Err(e)=>Err(e.to_string())}
}
fn private_dir(dir:&Path)->Result<PathBuf,String>{
  let dir=dir.join("credentials");checked(&dir)?;
  match fs::create_dir(&dir){Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e.to_string())}
  if !fs::symlink_metadata(&dir).map_err(|e|e.to_string())?.is_dir(){return Err("Credential storage must be a folder".into());}
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;}
  Ok(dir)
}
pub(crate) fn write(dir:&Path,service:&str,id:&str,secret:&str)->Result<(),String>{
  let target=path(dir,service,id)?;let tmp=private_dir(dir)?.join(format!(".{service}.{id}.tmp"));
  let _=fs::remove_file(&tmp);let mut options=fs::OpenOptions::new();options.write(true).create_new(true);
  #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
  let result=(||{let mut f=options.open(&tmp)?;f.write_all(secret.as_bytes())?;f.sync_all()?;fs::rename(&tmp,&target)})();
  if result.is_err(){let _=fs::remove_file(&tmp);}result.map_err(|e|e.to_string())
}
pub(crate) fn read(dir:&Path,service:&str,id:&str)->Result<String,String>{let path=path(dir,service,id)?;fs::read_to_string(checked(&path)?).map_err(|e|e.to_string())}
pub(crate) fn delete(dir:&Path,service:&str,id:&str)->Result<(),String>{
  match fs::remove_file(path(dir,service,id)?){Ok(())=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(e.to_string())}
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn round_trip_is_owner_only_and_rejects_unsafe_references(){
  let t=tempfile::tempdir().unwrap();write(t.path(),"bangumi","a-1","token-1").unwrap();write(t.path(),"bangumi","a-1","token-2").unwrap();
  assert_eq!(read(t.path(),"bangumi","a-1").unwrap(),"token-2");assert!(read(t.path(),"vndb","a-1").is_err());
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;let mode=|p:&Path|fs::metadata(p).unwrap().permissions().mode()&0o777;assert_eq!(mode(&t.path().join("credentials")),0o700);assert_eq!(mode(&t.path().join("credentials/bangumi.a-1")),0o600);}
  assert_eq!(fs::read_dir(t.path().join("credentials")).unwrap().count(),1);
  for bad in ["../x","","a/b","a.b"]{assert!(write(t.path(),"bangumi",bad,"x").is_err());assert!(read(t.path(),"bangumi",bad).is_err());}
  delete(t.path(),"bangumi","a-1").unwrap();delete(t.path(),"bangumi","a-1").unwrap();assert!(read(t.path(),"bangumi","a-1").is_err());
 }
 #[cfg(unix)]#[test]fn symlinked_folder_or_token_is_rejected(){
  let t=tempfile::tempdir().unwrap();let elsewhere=tempfile::tempdir().unwrap();
  std::os::unix::fs::symlink(elsewhere.path(),t.path().join("credentials")).unwrap();
  assert!(write(t.path(),"bangumi","a","x").is_err());assert!(read(t.path(),"bangumi","a").is_err());assert!(delete(t.path(),"bangumi","a").is_err());assert_eq!(fs::read_dir(elsewhere.path()).unwrap().count(),0);
  fs::remove_file(t.path().join("credentials")).unwrap();write(t.path(),"bangumi","a","x").unwrap();
  let leak=elsewhere.path().join("leak");std::os::unix::fs::symlink(&leak,t.path().join("credentials/.bangumi.b.tmp")).unwrap();write(t.path(),"bangumi","b","y").unwrap();assert!(!leak.exists());
  fs::write(elsewhere.path().join("other"),"z").unwrap();std::os::unix::fs::symlink(elsewhere.path().join("other"),t.path().join("credentials/bangumi.c")).unwrap();assert!(read(t.path(),"bangumi","c").is_err());
 }
}

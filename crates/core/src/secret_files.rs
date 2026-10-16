//! Owner-only token files for Cores without a system credential store (Linux/NAS).
//! Same protection level as lan-key.pem in device_dir; never part of catalog backups.
use std::{fs,io::Write,path::{Path,PathBuf}};

fn path(dir:&Path,service:&str,id:&str)->Result<PathBuf,String>{
  let safe=|s:&str|!s.is_empty()&&s.len()<=64&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_');
  if !safe(service)||!safe(id){return Err("Invalid credential reference".into());}
  Ok(dir.join("credentials").join(format!("{service}.{id}")))
}
fn private_dir(dir:&Path)->Result<PathBuf,String>{
  let dir=dir.join("credentials");fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;}
  Ok(dir)
}
pub(crate) fn write(dir:&Path,service:&str,id:&str,secret:&str)->Result<(),String>{
  let target=path(dir,service,id)?;let tmp=private_dir(dir)?.join(format!(".{service}.{id}.tmp"));
  let mut options=fs::OpenOptions::new();options.write(true).create(true).truncate(true);
  #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
  let result=(||{let mut f=options.open(&tmp)?;f.write_all(secret.as_bytes())?;f.sync_all()?;fs::rename(&tmp,&target)})();
  if result.is_err(){let _=fs::remove_file(&tmp);}result.map_err(|e|e.to_string())
}
pub(crate) fn read(dir:&Path,service:&str,id:&str)->Result<String,String>{fs::read_to_string(path(dir,service,id)?).map_err(|e|e.to_string())}
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
}

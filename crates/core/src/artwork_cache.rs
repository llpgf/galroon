//! Reconstructible artwork cache, separate from the catalog and private decisions.
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};
use std::path::Path;
pub const MAX_IMAGE_BYTES:usize=8*1024*1024;
pub const MAX_CACHE_BYTES:u64=256*1024*1024;
pub struct Cache{connection:Connection,budget:u64}
pub struct Image{pub mime:String,pub bytes:Vec<u8>}
#[derive(serde::Serialize)]pub struct Stats{pub images:i64,pub payload_bytes:i64,pub payload_limit:u64,pub allocated_bytes:i64}
fn mime(bytes:&[u8])->Option<&'static str>{
 if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Some("image/png")}
 else if bytes.starts_with(b"\xff\xd8\xff"){Some("image/jpeg")}
 else if bytes.len()>=12&&&bytes[..4]==b"RIFF"&&&bytes[8..12]==b"WEBP"{Some("image/webp")}
 else{None}
}
impl Cache{
 pub fn open(base:&Path)->Result<Self,String>{Self::with_budget(base,MAX_CACHE_BYTES)}
 fn with_budget(base:&Path,budget:u64)->Result<Self,String>{
  let path=base.join("artwork-cache.sqlite");crate::plans::validate_chain(&path)?;
  let connection=Connection::open(path).map_err(|e|e.to_string())?;
  connection.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e|e.to_string())?;
  connection.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; CREATE TABLE IF NOT EXISTS images(key TEXT PRIMARY KEY,mime TEXT NOT NULL,data BLOB NOT NULL,touched INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS images_lru ON images(touched,key);").map_err(|e|e.to_string())?;
  Ok(Self{connection,budget})
 }
 pub fn key(url:&str)->String{hex::encode(Sha256::digest(url.as_bytes()))}
 pub fn stats(&self)->Result<Stats,String>{
  let(images,payload_bytes)=self.connection.query_row("SELECT count(*),coalesce(sum(length(data)),0) FROM images",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
  let pages:i64=self.connection.query_row("PRAGMA page_count",[],|r|r.get(0)).map_err(|e|e.to_string())?;let size:i64=self.connection.query_row("PRAGMA page_size",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  Ok(Stats{images,payload_bytes,payload_limit:self.budget,allocated_bytes:pages*size})
 }
 pub fn clear(&mut self)->Result<Stats,String>{self.connection.execute("DELETE FROM images",[]).map_err(|e|e.to_string())?;self.connection.execute_batch("VACUUM").map_err(|e|e.to_string())?;self.stats()}
 pub fn get(&self,url:&str)->Result<Option<Image>,String>{
  let key=Self::key(url);
  let found:Option<(String,Vec<u8>)>=self.connection.query_row("SELECT mime,data FROM images WHERE key=?1",[&key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  let Some((kind,bytes))=found else{return Ok(None)};
  if bytes.len()>MAX_IMAGE_BYTES||mime(&bytes)!=Some(kind.as_str()){self.connection.execute("DELETE FROM images WHERE key=?1",[key]).map_err(|e|e.to_string())?;return Ok(None);}
  self.connection.execute("UPDATE images SET touched=(SELECT coalesce(max(touched),0)+1 FROM images) WHERE key=?1",[key]).map_err(|e|e.to_string())?;
  Ok(Some(Image{mime:kind,bytes}))
 }
 pub fn put(&mut self,url:&str,bytes:&[u8])->Result<(),String>{
  if bytes.len()>MAX_IMAGE_BYTES||bytes.len() as u64>self.budget{return Err("Artwork exceeds cache size limit".into());}
  let kind=mime(bytes).ok_or("Unsupported artwork format")?;let key=Self::key(url);
  let tx=self.connection.transaction().map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO images(key,mime,data,touched) VALUES(?1,?2,?3,(SELECT coalesce(max(touched),0)+1 FROM images)) ON CONFLICT(key) DO UPDATE SET mime=excluded.mime,data=excluded.data,touched=excluded.touched",params![key,kind,bytes]).map_err(|e|e.to_string())?;
  let mut total:i64=tx.query_row("SELECT coalesce(sum(length(data)),0) FROM images",[],|r|r.get(0)).map_err(|e|e.to_string())?;
  while total>self.budget as i64{let (old,size):(String,i64)=tx.query_row("SELECT key,length(data) FROM images ORDER BY touched,key LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;tx.execute("DELETE FROM images WHERE key=?1",[old]).map_err(|e|e.to_string())?;total-=size;}
  tx.commit().map_err(|e|e.to_string())?;
  self.connection.execute_batch("PRAGMA incremental_vacuum(256)").map_err(|e|e.to_string())?;Ok(())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 fn png(value:u8)->Vec<u8>{let mut b=b"\x89PNG\r\n\x1a\n".to_vec();b.extend([value;24]);b}
 #[test]fn clear_reclaims_cache_and_leaves_catalog_file_untouched(){
  let t=tempfile::tempdir().unwrap();std::fs::write(t.path().join("library.sqlite"),b"untouched fixture").unwrap();let mut c=Cache::open(t.path()).unwrap();let mut bytes=png(1);bytes.extend(vec![1;1024*1024]);c.put("a",&bytes).unwrap();let before=c.stats().unwrap();assert_eq!(before.images,1);assert_eq!(before.payload_bytes,bytes.len() as i64);let after=c.clear().unwrap();assert_eq!(after.images,0);assert_eq!(after.payload_bytes,0);assert!(after.allocated_bytes<before.allocated_bytes);assert_eq!(std::fs::read(t.path().join("library.sqlite")).unwrap(),b"untouched fixture");drop(c);assert_eq!(Cache::open(t.path()).unwrap().stats().unwrap().images,0);
 }
 #[test]fn persistent_lru_replacement_and_invalid_input_preserve_good_entries(){
  let t=tempfile::tempdir().unwrap();let mut c=Cache::with_budget(t.path(),64).unwrap();
  c.put("a",&png(1)).unwrap();c.put("b",&png(2)).unwrap();assert_eq!(c.get("a").unwrap().unwrap().bytes,png(1));c.put("c",&png(3)).unwrap();assert!(c.get("b").unwrap().is_none());
  assert!(c.put("a",b"<svg>bad</svg>").is_err());assert!(c.put("a",&vec![0;MAX_IMAGE_BYTES+1]).is_err());assert_eq!(c.get("a").unwrap().unwrap().bytes,png(1));
  c.put("a",&png(4)).unwrap();drop(c);let c=Cache::with_budget(t.path(),64).unwrap();assert_eq!(c.get("a").unwrap().unwrap().bytes,png(4));assert!(c.get("c").unwrap().is_some());
  let total:i64=c.connection.query_row("SELECT sum(length(data)) FROM images",[],|r|r.get(0)).unwrap();assert_eq!(total,64);
 }
 #[test]fn corrupt_cached_format_is_a_miss_and_keys_never_contain_paths(){
  let t=tempfile::tempdir().unwrap();let mut c=Cache::open(t.path()).unwrap();c.put("https://example.invalid/a",&png(1)).unwrap();
  c.connection.execute("UPDATE images SET data=CAST('<html>error</html>' AS BLOB)",[]).unwrap();assert!(c.get("https://example.invalid/a").unwrap().is_none());assert_eq!(Cache::key("../../secret").len(),64);
 }
}



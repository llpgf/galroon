//! Permanent offline artwork and public metadata, separate from the evictable cache.
use crate::artwork_cache::{Image,MAX_IMAGE_BYTES,mime};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Serialize,Deserialize};
use serde_json::Value;
use std::path::Path;
type R<T>=Result<T,String>;
pub const FILE:&str="artwork-library.sqlite";
pub struct Library{pub(crate) connection:Connection}
fn profile_key(key:&str)->R<()>{
 let parts:Vec<_>=key.split('.').collect();
 let valid=match parts.as_slice(){
  ["collection","v1",kind,id]=>match *kind{"character"=>Some('c'),"person"=>Some('s'),"company"=>Some('p'),_=>None}.is_some_and(|prefix|crate::exploration::valid_id(id,prefix)),
  ["exploration","v4","work",id,"1"]=>crate::exploration::valid_id(id,'v'),
  ["exploration","v3",kind,id,page]=>match *kind{"character"=>Some('c'),"person"=>Some('s'),"company"=>Some('p'),_=>None}.is_some_and(|prefix|crate::exploration::valid_id(id,prefix)&&page.parse::<u32>().is_ok_and(|n|(1..=1000).contains(&n))),
  _=>false
 };if !valid{return Err("Unsupported offline profile".into());}Ok(())
}
#[derive(Serialize,Deserialize,Debug)]pub struct Stats{pub images:i64,pub bytes:i64,pub profiles:i64}
impl Library{
 pub fn open(base:&Path)->R<Self>{
  let path=base.join(FILE);crate::plans::validate_chain(&path)?;
  let connection=Connection::open(path).map_err(|e|e.to_string())?;
  connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e|e.to_string())?;
  connection.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS images(url TEXT PRIMARY KEY,mime TEXT NOT NULL,data BLOB NOT NULL); CREATE TABLE IF NOT EXISTS profiles(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(|e|e.to_string())?;
  Ok(Self{connection})
 }
 pub fn image(&self,url:&str)->R<Option<Image>>{
  let found:Option<(String,Vec<u8>)>=self.connection.query_row("SELECT mime,data FROM images WHERE url=?1",[url],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  found.map(|(kind,bytes)|{if bytes.len()>MAX_IMAGE_BYTES||mime(&bytes)!=Some(kind.as_str()){return Err("Saved artwork is damaged".into());}Ok(Image{mime:kind,bytes})}).transpose()
 }
 pub fn contains(&self,url:&str)->R<bool>{self.connection.query_row("SELECT EXISTS(SELECT 1 FROM images WHERE url=?1)",[url],|r|r.get(0)).map_err(|e|e.to_string())}
 pub fn put(&self,url:&str,image:&Image)->R<()>{
  if image.bytes.len()>MAX_IMAGE_BYTES||mime(&image.bytes)!=Some(image.mime.as_str()){return Err("Unsupported saved artwork".into());}
  self.connection.execute("INSERT OR IGNORE INTO images(url,mime,data) VALUES(?1,?2,?3)",params![url,image.mime,image.bytes]).map_err(|e|e.to_string())?;Ok(())
 }
 pub fn profile(&self,key:&str)->R<Option<Value>>{
  let raw:Option<String>=self.connection.query_row("SELECT value FROM profiles WHERE key=?1",[key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  raw.map(|v|serde_json::from_str(&v).map_err(|e|e.to_string())).transpose()
 }
 pub fn save_profile(&self,key:&str,value:&Value)->R<()>{profile_key(key)?;self.connection.execute("INSERT INTO profiles(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value.to_string()]).map_err(|e|e.to_string())?;Ok(())}
 pub fn profiles(&self)->R<Vec<(String,String)>>{
  let mut s=self.connection.prepare("SELECT key,value FROM profiles ORDER BY key").map_err(|e|e.to_string())?;
  let rows=s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
  for(key,value)in &rows{profile_key(key)?;serde_json::from_str::<Value>(value).map_err(|e|e.to_string())?;}Ok(rows)
 }
 pub fn stats(&self)->R<Stats>{
  self.connection.query_row("SELECT count(*),coalesce(sum(length(data)),0),(SELECT count(*) FROM profiles) FROM images",[],|r|Ok(Stats{images:r.get(0)?,bytes:r.get(1)?,profiles:r.get(2)?})).map_err(|e|e.to_string())
 }
 pub fn snapshot(&self,destination:&Path)->R<Stats>{
  self.connection.backup(rusqlite::MAIN_DB,destination,None).map_err(|e|e.to_string())?;
  let connection=Connection::open(destination).map_err(|e|e.to_string())?;
  // A backup is a standalone file, with no WAL sidecar needed to read it.
  connection.execute_batch("PRAGMA journal_mode=DELETE;").map_err(|e|e.to_string())?;Self{connection}.stats()
 }
}
pub fn image(base:&Path,url:&str)->R<Option<Image>>{if !base.join(FILE).exists(){return Ok(None);}Library::open(base)?.image(url)}
pub fn contains(base:&Path,url:&str)->R<bool>{if !base.join(FILE).exists(){return Ok(false);}Library::open(base)?.contains(url)}
pub fn profile(base:&Path,key:&str)->R<Option<Value>>{if !base.join(FILE).exists(){return Ok(None);}Library::open(base)?.profile(key)}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn permanent_images_survive_cache_eviction_and_clear(){
  let t=tempfile::tempdir().unwrap();let image=Image{mime:"image/png".into(),bytes:b"\x89PNG\r\n\x1a\nfixture".to_vec()};
  Library::open(t.path()).unwrap().put("saved",&image).unwrap();
  let mut cache=crate::artwork_cache::Cache::open(t.path()).unwrap();cache.put("temporary",&image.bytes).unwrap();cache.clear().unwrap();
  let saved=Library::open(t.path()).unwrap();assert_eq!(saved.image("saved").unwrap().unwrap().bytes,image.bytes);assert_eq!(saved.stats().unwrap().images,1);
 }
 #[test]fn active_reader_does_not_block_parallel_image_writers(){
  let t=tempfile::tempdir().unwrap();let image=Image{mime:"image/png".into(),bytes:b"\x89PNG\r\n\x1a\nfixture".to_vec()};Library::open(t.path()).unwrap().put("first",&image).unwrap();
  let mut cache=crate::artwork_cache::Cache::open(t.path()).unwrap();cache.put("first",&image.bytes).unwrap();
  let saved=Connection::open(t.path().join(FILE)).unwrap();let cached=Connection::open(t.path().join("artwork-cache.sqlite")).unwrap();saved.execute_batch("BEGIN; SELECT * FROM images;").unwrap();cached.execute_batch("BEGIN; SELECT * FROM images;").unwrap();
  let (send,receive)=std::sync::mpsc::channel();let mut writers=Vec::new();
  for i in 0..4{let base=t.path().to_path_buf();let send=send.clone();writers.push(std::thread::spawn(move||{let result=(||->R<()>{let bytes=b"\x89PNG\r\n\x1a\nfixture".to_vec();let mut cache=crate::artwork_cache::Cache::open(&base)?;cache.put(&i.to_string(),&bytes)?;Library::open(&base)?.put(&i.to_string(),&Image{mime:"image/png".into(),bytes})})();send.send(result).unwrap();}));}
  for _ in 0..4{receive.recv_timeout(std::time::Duration::from_secs(2)).unwrap().unwrap();}for writer in writers{writer.join().unwrap();}
  assert_eq!(Library::open(t.path()).unwrap().stats().unwrap().images,5);
 }
}

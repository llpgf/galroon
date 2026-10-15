use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use super::{Error,Result,remote::{Collection,Subject,User}};
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 if !c.prepare("PRAGMA table_info(works)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?.iter().any(|v|v=="personal_rating"){c.execute("ALTER TABLE works ADD COLUMN personal_rating INTEGER CHECK(personal_rating BETWEEN 10 AND 100)",[])?;}
 c.execute_batch("CREATE TABLE IF NOT EXISTS vndb_sync_links(work_id TEXT PRIMARY KEY REFERENCES works(id),subject_id INTEGER NOT NULL UNIQUE,base_status INTEGER,base_rating INTEGER,hidden INTEGER NOT NULL DEFAULT 0);
 CREATE TABLE IF NOT EXISTS vndb_sync_remote(subject_id INTEGER PRIMARY KEY,status INTEGER NOT NULL,rating INTEGER NOT NULL,subject TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS vndb_sync_ignored(subject_id INTEGER PRIMARY KEY);
 CREATE TRIGGER IF NOT EXISTS vndb_sync_visibility_insert AFTER INSERT ON vndb_sync_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;
 CREATE TRIGGER IF NOT EXISTS vndb_sync_visibility_update AFTER UPDATE OF hidden,work_id,subject_id ON vndb_sync_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;
 CREATE TRIGGER IF NOT EXISTS vndb_sync_visibility_delete AFTER DELETE ON vndb_sync_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;")
}
#[derive(Clone,Serialize,Deserialize)]pub struct Account{pub user:User,pub credential:String,pub mode:String,pub automatic:bool,pub initialized:bool,pub last_sync:Option<i64>,pub preview:String,pub error:String,#[serde(default)]pub results:Vec<serde_json::Value>}
pub fn account(c:&Connection)->Result<Option<Account>>{c.query_row("SELECT value FROM settings WHERE key='vndb_sync.account'",[],|r|r.get::<_,String>(0)).optional()?.map(|v|serde_json::from_str(&v).map_err(Error::from)).transpose()}
pub fn save_account(c:&Connection,a:&Account)->Result<()>{c.execute("INSERT INTO settings(key,value) VALUES('vndb_sync.account',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(a)?])?;Ok(())}
pub fn local_status(status:i64)->Result<&'static str>{match status{1=>Ok("playing"),2=>Ok("completed"),3=>Ok("on_hold"),4=>Ok("dropped"),5=>Ok("backlog"),_=>Err(Error("VNDB has no unambiguous play status. Choose the local status explicitly.".into()))}}
pub fn remote_status(status:&str)->Result<i64>{match status{"playing"=>Ok(1),"completed"=>Ok(2),"on_hold"=>Ok(3),"dropped"=>Ok(4),"backlog"=>Ok(5),_=>Err(Error("Invalid local play status".into()))}}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Candidate{pub id:String,pub title:String}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Row{pub subject_id:i64,pub field:String,pub title:String,pub work_id:Option<String>,pub local:Option<i64>,pub remote:Option<i64>,pub base:Option<i64>,pub remote_rating:Option<i64>,pub kind:String,pub candidates:Vec<Candidate>,pub fingerprint:String}
impl Row{fn seal(&mut self)->Result<()>{self.fingerprint.clear();self.fingerprint=hex::encode(Sha256::digest(serde_json::to_vec(self)?));Ok(())}}
pub fn classify(local:i64,remote:Option<i64>,base:Option<i64>)->&'static str{match(remote,base){(None,_)=>"missing",(Some(r),_)if local==r=>"same",(Some(_),None)=>"initial",(Some(_),Some(b))if local==b=>"pull",(Some(r),Some(b))if r==b=>"push",_=>"conflict"}}
pub fn replace_remote(c:&Connection,rows:&[Collection])->Result<()>{c.execute("DELETE FROM vndb_sync_remote",[])?;for row in rows{let id=super::subject_id(&row.id)?;c.execute("INSERT INTO vndb_sync_remote VALUES(?1,?2,?3,?4)",params![id,row.status(),row.vote.unwrap_or(0),serde_json::to_string(&row.vn)?])?;}Ok(())}
fn candidates(c:&Connection,id:i64)->Result<Vec<Candidate>>{let mut q=c.prepare("SELECT id,title FROM works w WHERE merged_into IS NULL AND vndb_id=?1 AND NOT EXISTS(SELECT 1 FROM vndb_sync_links b WHERE b.work_id=w.id)")?;let rows=q.query_map([format!("v{id}")],|r|Ok(Candidate{id:r.get(0)?,title:r.get(1)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;Ok(rows)}
pub fn rows(c:&Connection,mode:&str)->Result<Vec<Row>>{
 let mut out=Vec::new();let mut q=c.prepare("SELECT b.subject_id,w.id,w.title,w.status,coalesce(w.personal_rating,0),b.base_status,b.base_rating,b.hidden,r.status,r.rating FROM vndb_sync_links b JOIN works w ON w.id=b.work_id LEFT JOIN vndb_sync_remote r ON r.subject_id=b.subject_id WHERE w.merged_into IS NULL ORDER BY w.title")?;
 for item in q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?,r.get::<_,Option<i64>>(6)?,r.get::<_,bool>(7)?,r.get::<_,Option<i64>>(8)?,r.get::<_,Option<i64>>(9)?)))?{
  let(sid,id,title,status,rating,bs,br,hidden,rs,rr)=item?;
  for(field,local,remote,base)in[("status",remote_status(&status)?,rs,bs),("rating",rating,rr,br)]{
   let kind=if hidden{"hidden"}else if field=="status"&&remote.is_some_and(|v|v<=0){"unsupported"}else{classify(local,remote,base)};
   let mut row=Row{subject_id:sid,field:field.into(),title:title.clone(),work_id:Some(id.clone()),local:Some(local),remote,base,remote_rating:None,kind:kind.into(),candidates:vec![],fingerprint:String::new()};row.seal()?;out.push(row);
  }
 }
 let mut q=c.prepare("SELECT r.subject_id,r.status,r.rating,r.subject,EXISTS(SELECT 1 FROM vndb_sync_ignored i WHERE i.subject_id=r.subject_id) FROM vndb_sync_remote r WHERE NOT EXISTS(SELECT 1 FROM vndb_sync_links b WHERE b.subject_id=r.subject_id) ORDER BY r.subject_id")?;
 for item in q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,bool>(4)?)))?{let(id,status,rating,raw,ignored)=item?;let subject:Subject=serde_json::from_str(&raw)?;let candidates=candidates(c,id)?;let kind=if ignored{"ignored"}else if !candidates.is_empty(){"match"}else if mode=="existing"{"skipped"}else{"import"};let mut row=Row{subject_id:id,field:"entry".into(),title:subject.title,work_id:None,local:None,remote:Some(status),remote_rating:Some(rating),base:None,kind:kind.into(),candidates,fingerprint:String::new()};row.seal()?;out.push(row);}
 Ok(out)
}
pub fn current(c:&Connection,mode:&str,id:i64,field:&str,fingerprint:&str)->Result<Row>{let row=rows(c,mode)?.into_iter().find(|r|r.subject_id==id&&r.field==field).ok_or_else(||Error("VNDB preview changed. Preview again.".into()))?;if row.fingerprint!=fingerprint{return Err(Error("VNDB or local state changed. Preview again.".into()));}Ok(row)}
pub fn import(c:&Connection,id:i64)->Result<String>{
 if !candidates(c,id)?.is_empty(){return Err(Error("Link the existing VNDB card instead of creating a duplicate".into()));}
 let(status,rating,raw):(i64,i64,String)=c.query_row("SELECT status,rating,subject FROM vndb_sync_remote WHERE subject_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;let subject:Subject=serde_json::from_str(&raw)?;let work=crate::db::id();
 c.execute("INSERT INTO works(id,title,original_title,vndb_id,description,cover,released,status,personal_rating,source_json) VALUES(?1,?2,?2,?3,?4,?5,?6,?7,nullif(?8,0),?9)",params![work,subject.title,format!("v{id}"),subject.description.as_deref().unwrap_or(""),subject.cover(),subject.released.as_deref().unwrap_or(""),local_status(status).unwrap_or("backlog"),rating,raw])?;
 c.execute("INSERT INTO vndb_sync_links(work_id,subject_id,base_status,base_rating) VALUES(?1,?2,?3,?4)",params![work,id,if status>0{Some(status)}else{None},rating])?;c.execute("DELETE FROM vndb_sync_ignored WHERE subject_id=?1",[id])?;Ok(work)
}
pub fn baseline(c:&Connection,id:i64,field:&str,value:i64)->Result<()>{let(base,column)=columns(field)?;c.execute(&format!("UPDATE vndb_sync_links SET {base}=?2 WHERE subject_id=?1"),params![id,value])?;c.execute(&format!("UPDATE vndb_sync_remote SET {column}=?2 WHERE subject_id=?1"),params![id,value])?;Ok(())}
fn columns(field:&str)->Result<(&'static str,&'static str)>{match field{"status"=>Ok(("base_status","status")),"rating"=>Ok(("base_rating","rating")),_=>Err(Error("Invalid VNDB field".into()))}}
pub fn pull(c:&Connection,row:&Row,value:i64)->Result<()>{let id=row.work_id.as_ref().ok_or_else(||Error("Link a work first".into()))?;if row.field=="status"{let status=local_status(value)?;c.execute("UPDATE works SET status=?2,revision=revision+1,overrides=json_patch(overrides,?3) WHERE id=?1",params![id,status,json!({"status":status}).to_string()])?;}else{super::remote::patch("rating",value)?;c.execute("UPDATE works SET personal_rating=nullif(?2,0),revision=revision+1 WHERE id=?1",params![id,value])?;}baseline(c,row.subject_id,&row.field,value)}
pub fn merge(c:&Connection,source:&str,target:&str)->Result<()>{
 let link:Option<i64>=c.query_row("SELECT subject_id FROM vndb_sync_links WHERE work_id=?1",[source],|r|r.get(0)).optional()?;let Some(id)=link else{return Ok(());};
 if c.query_row("SELECT EXISTS(SELECT 1 FROM vndb_sync_links WHERE work_id=?1)",[target],|r|r.get::<_,bool>(0))?{return Err(Error("Both works have VNDB sync links. Unlink one before merging.".into()));}
 let target_vn:Option<String>=c.query_row("SELECT vndb_id FROM works WHERE id=?1",[target],|r|r.get(0))?;if target_vn.is_some_and(|v|!v.is_empty()&&v!=format!("v{id}")){return Err(Error("Cannot merge sync links for different VNDB works".into()));}
 c.execute("UPDATE works SET status=(SELECT status FROM works WHERE id=?1),personal_rating=(SELECT personal_rating FROM works WHERE id=?1) WHERE id=?2",params![source,target])?;c.execute("UPDATE vndb_sync_links SET work_id=?2,hidden=0 WHERE work_id=?1",params![source,target])?;Ok(())
}

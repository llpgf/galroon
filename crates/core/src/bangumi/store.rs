use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use super::{Error,Result,remote::{Collection,Subject,User}};

pub fn migrate(c:&Connection)->rusqlite::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS bangumi_links(work_id TEXT PRIMARY KEY REFERENCES works(id),subject_id INTEGER NOT NULL UNIQUE,base_status INTEGER,imported INTEGER NOT NULL DEFAULT 0,hidden INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE IF NOT EXISTS bangumi_remote(subject_id INTEGER PRIMARY KEY,status INTEGER NOT NULL,subject TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS bangumi_ignored(subject_id INTEGER PRIMARY KEY);
    CREATE TRIGGER IF NOT EXISTS bangumi_visibility_insert AFTER INSERT ON bangumi_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;
    CREATE TRIGGER IF NOT EXISTS bangumi_visibility_update AFTER UPDATE OF hidden,work_id,subject_id ON bangumi_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;
    CREATE TRIGGER IF NOT EXISTS bangumi_visibility_delete AFTER DELETE ON bangumi_links BEGIN UPDATE smart_catalog_state SET collection_revision=collection_revision+1,revision=revision+1 WHERE singleton=1; END;")
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Account {pub user:User,pub credential:String,pub mode:String,pub automatic:bool,pub initialized:bool,pub last_sync:Option<i64>,pub preview:String,pub error:String}
pub fn account(c:&Connection)->Result<Option<Account>> {
    c.query_row("SELECT value FROM settings WHERE key='bangumi.account'",[],|r|r.get::<_,String>(0)).optional()?.map(|v|serde_json::from_str(&v).map_err(|_|Error("Invalid Bangumi account settings".into()))).transpose()
}
pub fn save_account(c:&Connection,a:&Account)->Result<()> {
    c.execute("INSERT INTO settings(key,value) VALUES('bangumi.account',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(a)?])?;Ok(())
}
pub fn local_status(status:i64)->Result<&'static str> {match status{1=>Ok("backlog"),2=>Ok("completed"),3=>Ok("playing"),4=>Ok("on_hold"),5=>Ok("dropped"),_=>Err(Error("Invalid Bangumi collection status".into()))}}
pub fn remote_status(status:&str)->Result<i64> {match status{"backlog"=>Ok(1),"completed"=>Ok(2),"playing"=>Ok(3),"on_hold"=>Ok(4),"dropped"=>Ok(5),_=>Err(Error("Invalid local play status".into()))}}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct Candidate {pub id:String,pub title:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Row {pub subject_id:i64,pub title:String,pub work_id:Option<String>,pub local:Option<i64>,pub remote:Option<i64>,pub base:Option<i64>,pub revision:Option<i64>,pub kind:String,pub candidates:Vec<Candidate>,pub fingerprint:String}
impl Row {
    fn seal(&mut self)->Result<()> {self.fingerprint.clear();self.fingerprint=hex::encode(Sha256::digest(serde_json::to_vec(self)?));Ok(())}
}
pub fn classify(local:i64,remote:Option<i64>,base:Option<i64>)->&'static str {
    match (remote,base) {
        (None,_)=>"missing",
        (Some(r),_) if local==r=>"same",
        (Some(_),None)=>"initial",
        (Some(_),Some(b)) if local==b=>"pull",
        (Some(r),Some(b)) if r==b=>"push",
        _=>"conflict",
    }
}
pub fn replace_remote(c:&Connection,rows:&[Collection])->Result<()> {
    c.execute("DELETE FROM bangumi_remote",[])?;
    for row in rows {c.execute("INSERT INTO bangumi_remote VALUES(?1,?2,?3)",params![row.subject_id,row.status,serde_json::to_string(&row.subject)?])?;}Ok(())
}
// Bangumi and VNDB punctuate the same title differently (「—」/「－」, 「～」 spacing, a trailing 「。」),
// spell readings inline (「不可視の薬（インビジブル）と」) and drop subtitles. Candidates come in tiers:
// exact, folded, compact (no spaces or readings), then a title prefix on the very same release date.
// The user still confirms every match; a prefix never accepts a following digit, so sequels stay apart.
struct Local {id:String,title:String,released:String,exact:Vec<String>,folded:Vec<String>,compact:Vec<String>}
fn compact(title:&str)->String {
    static READING:std::sync::OnceLock<regex::Regex>=std::sync::OnceLock::new();
    let reading=READING.get_or_init(||regex::Regex::new(r"[（(][\p{Hiragana}\p{Katakana}\p{Han}ー・\s]+[)）]").unwrap());
    crate::collection_query::fold(&reading.replace_all(title," ")).replace(' ',"")
}
fn locals(c:&Connection)->Result<Vec<Local>> {
    use crate::collection_query::{fold,normalized};
    let mut q=c.prepare("SELECT id,title,original_title,aliases,released FROM works w WHERE merged_into IS NULL AND NOT EXISTS(SELECT 1 FROM bangumi_links b WHERE b.work_id=w.id) ORDER BY title,id")?;
    let mut out=Vec::new();
    for item in q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))? {
        let(id,title,original,aliases,released)=item?;let mut titles=serde_json::from_str::<Vec<String>>(&aliases)?;titles.push(title.clone());titles.push(original);
        let keys=|key:fn(&str)->String|titles.iter().map(|t|key(t)).filter(|k|!k.is_empty()).collect();
        out.push(Local{id,title,released,exact:keys(normalized),folded:keys(fold),compact:keys(compact)});
    }Ok(out)
}
fn dated_prefix(local:&[String],remote:&[String])->bool {
    local.iter().any(|a|remote.iter().any(|b|{
        let(short,long)=if a.chars().count()<=b.chars().count(){(a,b)}else{(b,a)};
        short.chars().count()>=4&&short!=long&&long.strip_prefix(short.as_str()).is_some_and(|tail|!tail.starts_with(|c:char|c.is_numeric()))
    }))
}
fn candidates(locals:&[Local],subject:&Subject)->Vec<Candidate> {
    use crate::collection_query::{fold,normalized};
    let names=[subject.title(),subject.name.as_str()];
    let exact=names.map(normalized);let folded=names.map(fold);let compact=names.map(compact).into_iter().filter(|k|!k.is_empty()).collect::<Vec<_>>();
    let date=subject.date.as_deref().filter(|d|d.len()==10).unwrap_or("");
    let tier=|l:&Local|if l.exact.iter().any(|n|exact.contains(n)){Some(0)}else if l.folded.iter().any(|n|folded.contains(n)){Some(1)}
        else if l.compact.iter().any(|n|compact.contains(n)){Some(2)}else if !date.is_empty()&&l.released==date&&dated_prefix(&l.compact,&compact){Some(3)}else{None};
    let mut found:Vec<(u8,&Local)>=locals.iter().filter_map(|l|tier(l).map(|t|(t,l))).collect();found.sort_by_key(|(t,_)|*t);
    found.into_iter().take(10).map(|(_,l)|Candidate{id:l.id.clone(),title:l.title.clone()}).collect()
}
pub fn rows(c:&Connection,mode:&str)->Result<Vec<Row>> {
    let mut out=Vec::new();
    let mut q=c.prepare("SELECT b.subject_id,w.id,w.title,w.status,w.revision,b.base_status,b.hidden,r.status FROM bangumi_links b JOIN works w ON w.id=b.work_id LEFT JOIN bangumi_remote r ON r.subject_id=b.subject_id WHERE w.merged_into IS NULL ORDER BY w.title")?;
    for row in q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?,r.get::<_,bool>(6)?,r.get::<_,Option<i64>>(7)?)))? {
        let(subject_id,id,title,status,revision,base,hidden,remote)=row?;let local=remote_status(&status)?;
        let mut row=Row{subject_id,title,work_id:Some(id),local:Some(local),remote,base,revision:Some(revision),kind:if hidden{"hidden"}else{classify(local,remote,base)}.into(),candidates:vec![],fingerprint:String::new()};row.seal()?;out.push(row);
    }
    let locals=locals(c)?;
    let mut q=c.prepare("SELECT r.subject_id,r.status,r.subject,EXISTS(SELECT 1 FROM bangumi_ignored i WHERE i.subject_id=r.subject_id) FROM bangumi_remote r WHERE NOT EXISTS(SELECT 1 FROM bangumi_links b WHERE b.subject_id=r.subject_id) ORDER BY r.subject_id")?;
    for item in q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,bool>(3)?)))? {
        let(subject_id,status,raw,ignored)=item?;let subject:Subject=serde_json::from_str(&raw)?;let candidates=candidates(&locals,&subject);
        let kind=if ignored{"ignored"}else if !candidates.is_empty(){"match"}else if mode=="existing"{"skipped"}else{"import"};
        let mut row=Row{subject_id,title:subject.title().into(),work_id:None,local:None,remote:Some(status),base:None,revision:None,kind:kind.into(),candidates,fingerprint:String::new()};row.seal()?;out.push(row);
    }Ok(out)
}
pub fn current(c:&Connection,mode:&str,id:i64,fingerprint:&str)->Result<Row> {
    let row=rows(c,mode)?.into_iter().find(|r|r.subject_id==id).ok_or_else(||Error("Bangumi preview changed. Preview again.".into()))?;
    if row.fingerprint!=fingerprint {return Err(Error("Bangumi or local state changed. Preview again.".into()));}Ok(row)
}
pub fn import(c:&Connection,subject_id:i64)->Result<String> {
    let(status,raw):(i64,String)=c.query_row("SELECT status,subject FROM bangumi_remote WHERE subject_id=?1",[subject_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let subject:Subject=serde_json::from_str(&raw)?;let id=crate::db::id();
    let baseline=json!({"title":subject.title(),"original_title":subject.name,"description":subject.summary,"cover":subject.cover(),"released":subject.date.as_deref().unwrap_or("")});
    c.execute("INSERT INTO works(id,title,original_title,description,cover,released,status,source_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,subject.title(),subject.name,subject.summary,subject.cover(),subject.date.as_deref().unwrap_or(""),local_status(status)?,baseline.to_string()])?;
    c.execute("INSERT INTO bangumi_links(work_id,subject_id,base_status,imported) VALUES(?1,?2,?3,1)",params![id,subject_id,status])?;
    c.execute("DELETE FROM bangumi_ignored WHERE subject_id=?1",[subject_id])?;Ok(id)
}
pub fn pull(c:&Connection,row:&Row,status:i64)->Result<()> {
    let id=row.work_id.as_ref().ok_or_else(||Error("Work is not linked".into()))?;
    c.execute("UPDATE works SET status=?2,revision=revision+1,overrides=json_patch(overrides,?3) WHERE id=?1",params![id,local_status(status)?,json!({"status":local_status(status)?}).to_string()])?;
    baseline(c,row.subject_id,status)
}
pub fn baseline(c:&Connection,subject:i64,status:i64)->Result<()> {
    c.execute("UPDATE bangumi_links SET base_status=?2 WHERE subject_id=?1",params![subject,status])?;
    c.execute("UPDATE bangumi_remote SET status=?2 WHERE subject_id=?1",params![subject,status])?;Ok(())
}
// Retain the Bangumi identity when the user attaches a later local scan via Group works.
pub fn merge(c:&Connection,source:&str,target:&str)->Result<()> {
    let source_link:Option<i64>=c.query_row("SELECT subject_id FROM bangumi_links WHERE work_id=?1",[source],|r|r.get(0)).optional()?;
    if source_link.is_none(){return Ok(());}
    if c.query_row("SELECT EXISTS(SELECT 1 FROM bangumi_links WHERE work_id=?1)",[target],|r|r.get::<_,bool>(0))? {return Err(Error("Both works have Bangumi links. Unlink one before merging.".into()));}
    c.execute("UPDATE works SET status=(SELECT status FROM works WHERE id=?1) WHERE id=?2",params![source,target])?;
    c.execute("UPDATE bangumi_links SET work_id=?2,hidden=0 WHERE work_id=?1",params![source,target])?;Ok(())
}

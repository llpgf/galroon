//! VNDB identity for cards imported from Bangumi. Scanned works get theirs from their files; an imported card has
//! none, so it is looked up once by its Bangumi title. Only one VNDB work with the same title and the same release
//! day is linked automatically; anything else waits on the work page for the user to pick or search.
use axum::{extract::{Path,State},http::HeaderMap,Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde::Deserialize;
use serde_json::{Value,json};
use std::time::Duration;
use super::{Error,Result,remote::Subject,store::compact};
use crate::{App,ApiError};

pub fn migrate(c:&Connection)->rusqlite::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS bangumi_vndb(work_id TEXT PRIMARY KEY REFERENCES works(id),state TEXT NOT NULL,candidates TEXT NOT NULL DEFAULT '[]',checked INTEGER NOT NULL)")
}
#[derive(Debug,PartialEq)]
pub enum Decision {Link(String),Review(Vec<Value>),None}
fn titles(v:&Value)->Vec<String> {
    let mut out=vec![v["title"].as_str().unwrap_or_default().to_owned(),v["alttitle"].as_str().unwrap_or_default().to_owned()];
    out.extend(v["titles"].as_array().into_iter().flatten().filter_map(|t|t["title"].as_str().map(str::to_owned)));
    out.extend(v["aliases"].as_array().into_iter().flatten().filter_map(|t|t.as_str().map(str::to_owned)));
    out.into_iter().map(|t|compact(&t)).filter(|t|!t.is_empty()).collect()
}
/// `taken` says whether another active card already holds a VNDB id; such a work is shown but never linked here.
pub fn decide(subject:&Subject,results:&[Value],taken:&dyn Fn(&str)->bool)->Decision {
    let names:Vec<String>=[subject.name.as_str(),subject.name_cn.as_str()].into_iter().map(compact).filter(|n|!n.is_empty()).collect();
    let date=subject.date.as_deref().filter(|d|d.len()==10).unwrap_or("");
    let titled=|v:&&Value|titles(v).iter().any(|t|names.contains(t));
    let dated=|v:&&Value|!date.is_empty()&&v["released"].as_str()==Some(date);
    let exact:Vec<&Value>=results.iter().filter(|v|titled(v)&&dated(v)).collect();
    if let [only]=exact.as_slice() {if let Some(id)=only["id"].as_str().filter(|id|!taken(id)) {return Decision::Link(id.into());}}
    let candidates:Vec<Value>=results.iter().filter(|v|titled(v)||dated(v)).take(5).filter_map(|v|{
        let id=v["id"].as_str()?;
        let reason=match(titled(&v),dated(&v)){(true,true)=>"title_date",(true,false)=>"title",_=>"date"};
        Some(json!({"id":id,"title":v["alttitle"].as_str().filter(|s|!s.is_empty()).or(v["title"].as_str()).unwrap_or(id),"released":v["released"],"reason":reason,"taken":taken(id)}))
    }).collect();
    if candidates.is_empty(){Decision::None}else{Decision::Review(candidates)}
}
async fn search(name:&str)->Result<Vec<Value>> {
    crate::vndb_rate::wait(Duration::from_millis(1600)).await;
    let response=reqwest::Client::new().post(format!("{}/vn",crate::matching::vndb_api())).timeout(Duration::from_secs(25))
        .json(&json!({"filters":["search","=",name],"fields":"title,alttitle,titles.title,aliases,released","results":10})).send().await.map_err(|e|Error(e.to_string()))?;
    if !response.status().is_success(){return Err(Error(format!("VNDB returned {}; try again later",response.status())));}
    let data:Value=response.json().await.map_err(|e|Error(e.to_string()))?;
    Ok(data["results"].as_array().cloned().unwrap_or_default())
}
fn taken(c:&Connection,work:&str,id:&str)->bool {
    c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE vndb_id=?1 AND merged_into IS NULL AND id!=?2)",params![id,work],|r|r.get(0)).unwrap_or(true)
}
fn pending(c:&Connection)->Result<Option<(String,Subject)>> {
    let row:Option<(String,String)>=c.query_row("SELECT b.work_id,r.subject FROM bangumi_links b JOIN works w ON w.id=b.work_id JOIN bangumi_remote r ON r.subject_id=b.subject_id
        WHERE b.imported=1 AND w.merged_into IS NULL AND coalesce(w.vndb_id,'')='' AND NOT EXISTS(SELECT 1 FROM bangumi_vndb v WHERE v.work_id=b.work_id) ORDER BY b.subject_id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    row.map(|(work,raw)|Ok((work,serde_json::from_str(&raw)?))).transpose()
}
fn link(c:&Connection,work:&str,id:&str)->Result<()> {
    c.execute("UPDATE works SET vndb_id=?2,revision=revision+1 WHERE id=?1",params![work,id])?;
    c.execute("INSERT INTO bangumi_vndb(work_id,state,checked) VALUES(?1,'linked',?2) ON CONFLICT(work_id) DO UPDATE SET state='linked',candidates='[]',checked=excluded.checked",params![work,crate::db::now()])?;Ok(())
}
/// Looks up one imported card per call; returns false when none is waiting. A provider error leaves it pending.
pub async fn step(a:&App)->Result<bool> {
    let Some((work,subject))=pending(&a.db.lock().unwrap())? else{return Ok(false)};
    let results=search(if subject.name.is_empty(){&subject.name_cn}else{&subject.name}).await?;
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;record(&tx,&work,&subject,&results)?;tx.commit()?;Ok(true)
}
/// Stores the outcome unless the card was paired, merged or already decided while VNDB answered.
pub(crate) fn record(c:&Connection,work:&str,subject:&Subject,results:&[Value])->Result<()> {
    if !c.query_row("SELECT EXISTS(SELECT 1 FROM bangumi_links b JOIN works w ON w.id=b.work_id WHERE b.work_id=?1 AND b.imported=1 AND w.merged_into IS NULL AND coalesce(w.vndb_id,'')='' AND NOT EXISTS(SELECT 1 FROM bangumi_vndb v WHERE v.work_id=b.work_id))",[work],|r|r.get::<_,bool>(0))?{return Ok(());}
    match decide(subject,results,&|id|taken(c,work,id)) {
        Decision::Link(id)=>link(c,work,&id)?,
        Decision::Review(candidates)=>{c.execute("INSERT INTO bangumi_vndb(work_id,state,candidates,checked) VALUES(?1,'review',?2,?3)",params![work,serde_json::to_string(&candidates)?,crate::db::now()])?;},
        Decision::None=>{c.execute("INSERT INTO bangumi_vndb(work_id,state,checked) VALUES(?1,'none',?2)",params![work,crate::db::now()])?;},
    }
    Ok(())
}
pub async fn get(State(a):State<App>,h:HeaderMap,Path(work):Path<String>)->crate::Result<Value>{
    crate::access::authenticate(&a,&h)?;
    let row:Option<(String,String)>=a.db.lock().unwrap().query_row("SELECT state,candidates FROM bangumi_vndb WHERE work_id=?1",[&work],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let(state,candidates)=row.unwrap_or_else(||("pending".into(),"[]".into()));
    Ok(Json(json!({"state":state,"candidates":serde_json::from_str::<Value>(&candidates).map_err(|e|ApiError(e.to_string()))?})))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Link{work_id:String,revision:i64,vndb_id:String}
/// Manual pairing for a card without files. Two cards for one VNDB work are merged with Group works instead.
pub async fn set(State(a):State<App>,h:HeaderMap,Json(input):Json<Link>)->crate::Result<Value>{
    if crate::access::authenticate(&a,&h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}
    if !input.vndb_id.strip_prefix('v').is_some_and(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())){return Err(ApiError("Invalid VNDB work identifier".into()));}
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;pair(&tx,&input.work_id,input.revision,&input.vndb_id)?;tx.commit()?;Ok(Json(json!({"ok":true})))
}
pub(crate) fn pair(tx:&Connection,work:&str,expected:i64,vndb_id:&str)->std::result::Result<(),ApiError>{
    let input=(work,expected,vndb_id);
    let current:Option<(i64,Option<String>)>=tx.query_row("SELECT revision,vndb_id FROM works WHERE id=?1 AND merged_into IS NULL",[input.0],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let Some((revision,existing))=current else{return Err(ApiError("Work not found".into()))};
    if revision!=input.1{return Err(ApiError("This work changed. Reload before editing.".into()));}
    if existing.is_some_and(|v|!v.is_empty()){return Err(ApiError("This card is already paired with VNDB".into()));}
    if taken(tx,input.0,input.2){return Err(ApiError("Another card already has this VNDB work. Use Group works to merge the two cards.".into()));}
    link(tx,input.0,input.2)?;Ok(())
}

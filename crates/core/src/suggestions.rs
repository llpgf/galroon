//! AI proposals are catalog records; only a person's confirmation uses manual binding.
use axum::{extract::{State,Path,Query},http::{HeaderMap,StatusCode},response::{IntoResponse,Response},Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde::Deserialize;
use serde_json::{Value,json};
use crate::{App,db::{self,Db},matching,editions};
type R<T>=Result<T,Error>;
#[derive(Debug)]pub struct Error(String);
impl From<String> for Error{fn from(v:String)->Self{Self(v)}}
impl From<rusqlite::Error> for Error{fn from(v:rusqlite::Error)->Self{Self(v.to_string())}}
impl From<crate::ApiError> for Error{fn from(v:crate::ApiError)->Self{Self(v.0)}}
impl IntoResponse for Error{fn into_response(self)->Response{if self.0.starts_with("Conflict:"){(StatusCode::CONFLICT,Json(json!({"error":self.0,"code":"suggestion_changed"}))).into_response()}else{crate::ApiError(self.0).into_response()}}}
fn conflict()->Error{Error("Conflict: resource or suggestion changed. Reload before confirming.".into())}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS match_suggestions(id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id),resource_revision INTEGER NOT NULL,works TEXT NOT NULL CHECK(json_valid(works) AND json_array_length(works)>0),query TEXT NOT NULL,rationale TEXT NOT NULL,source TEXT NOT NULL,verified INTEGER NOT NULL CHECK(verified IN (0,1)),verification TEXT NOT NULL CHECK(json_valid(verification)),state TEXT NOT NULL CHECK(state IN ('open','confirmed','dismissed','stale')),created INTEGER NOT NULL,updated INTEGER NOT NULL);
 CREATE UNIQUE INDEX IF NOT EXISTS suggestion_open_resource ON match_suggestions(resource_id) WHERE state='open';
 CREATE INDEX IF NOT EXISTS suggestion_state ON match_suggestions(state,created);
 CREATE TRIGGER IF NOT EXISTS suggestion_revision AFTER UPDATE OF revision ON resources WHEN OLD.revision IS NOT NEW.revision BEGIN UPDATE match_suggestions SET state='stale',updated=unixepoch() WHERE resource_id=NEW.id AND state='open'; END;")}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Create{pub revision:i64,pub works:Vec<String>,pub query:String,pub rationale:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Revision{pub revision:i64}
#[derive(Default,Deserialize)]#[serde(deny_unknown_fields)]pub struct Page{pub before:Option<i64>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Batch{pub ids:Vec<String>,pub revisions:Vec<i64>}
fn writable(a:&App,h:&HeaderMap)->R<String>{let who=crate::access::authenticate(a,h)?;if !matches!(who.role.as_str(),"owner"|"desktop"){return Err(Error("Desktop access required".into()));}let name=if who.role=="owner"{"owner".into()}else{a.db.lock().unwrap().query_row("SELECT name FROM sessions WHERE id=?1",[who.id],|r|r.get::<_,String>(0))?};Ok(format!("{}:{name}",if who.role=="desktop"{"mcp"}else{"owner"}))}
fn validate(v:&Create)->R<()>{if v.revision<1||v.works.is_empty()||v.works.len()>32||v.query.trim().is_empty()||v.query.len()>512||v.rationale.trim().is_empty()||v.rationale.len()>8192{return Err(Error("Supply a revision, 1–32 works, query (1–512 bytes) and rationale (1–8192 bytes)".into()));}let mut ids=std::collections::HashSet::new();for id in &v.works{if !id.strip_prefix('v').is_some_and(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit()))||!ids.insert(id){return Err(Error("Unique VNDB work ids required".into()));}}Ok(())}
fn available(c:&Connection,rid:&str,revision:i64)->R<()>{let current:Option<(i64,bool)>=c.query_row("SELECT revision,work_id IS NOT NULL OR EXISTS(SELECT 1 FROM resource_bindings WHERE resource_id=resources.id) FROM resources WHERE id=?1",[rid],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;if current!=Some((revision,false)){return Err(conflict());}Ok(())}
/// Work-owned names only. Provider release titles cannot establish a verified suggestion.
fn verify(input:&matching::Input,query:&str,work:&Value)->Value{
 let targets:Vec<String>=std::iter::once(query).chain(input.titles.iter().map(String::as_str)).map(matching::normalized).filter(|s|!s.is_empty()).collect();
 let exact=matching::candidate_names(work).into_iter().find(|name|targets.contains(&matching::normalized(name)));
 let no_hints=input.brands.is_empty()&&input.dates.is_empty();let agrees=matching::corroborated(input,work);
 json!({"vndb_id":work["id"],"verified":exact.is_some()&&(no_hints||agrees),"exact_name":exact,"title_reason":if exact.is_some(){"exact_own_name"}else{"no_exact_own_name"},"corroboration":if no_hints{"no_folder_brand_or_date"}else if agrees{"brand_or_date_agrees"}else{"brand_or_date_not_corroborated"}})
}
fn store(c:&Connection,rid:&str,v:&Create,source:&str,candidates:&[Value])->R<Value>{
 validate(v)?;available(c,rid,v.revision)?;
 let input=crate::automatch::catalog_input(c,rid)?.ok_or_else(||Error("Resource has no usable files".into()))?;
 let mut works=Vec::new();let mut reasons=Vec::new();for id in &v.works{let work=candidates.iter().find(|w|w["id"]==*id).ok_or_else(||Error("VNDB work not found".into()))?;let title=work["title"].as_str().filter(|t|!t.is_empty()).ok_or_else(||Error("VNDB work has no title".into()))?;works.push(json!({"vndb_id":id,"title":title}));reasons.push(verify(&input,&v.query,work));}
 let verified=reasons.iter().all(|v|v["verified"]==true);let id=db::id();let now=db::now();
 c.execute("UPDATE match_suggestions SET state='dismissed',updated=?2 WHERE resource_id=?1 AND state='open'",params![rid,now])?;
 c.execute("INSERT INTO match_suggestions(id,resource_id,resource_revision,works,query,rationale,source,verified,verification,state,created,updated) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'open',?10,?10)",params![id,rid,v.revision,json!(works).to_string(),v.query,v.rationale,source,verified,json!(reasons).to_string(),now])?;get(c,&id)
}
fn row(r:&rusqlite::Row)->rusqlite::Result<Value>{let parse=|i|->rusqlite::Result<Value>{let s:String=r.get(i)?;serde_json::from_str(&s).map_err(|e|rusqlite::Error::FromSqlConversionFailure(i,rusqlite::types::Type::Text,Box::new(e)))};Ok(json!({"id":r.get::<_,String>(0)?,"resource_id":r.get::<_,String>(1)?,"resource_revision":r.get::<_,i64>(2)?,"works":parse(3)?,"query":r.get::<_,String>(4)?,"rationale":r.get::<_,String>(5)?,"source":r.get::<_,String>(6)?,"verified":r.get::<_,bool>(7)?,"verification":parse(8)?,"state":r.get::<_,String>(9)?,"created":r.get::<_,i64>(10)?,"updated":r.get::<_,i64>(11)?}))}
const COLUMNS:&str="id,resource_id,resource_revision,works,query,rationale,source,verified,verification,state,created,updated";
fn get(c:&Connection,id:&str)->R<Value>{c.query_row(&format!("SELECT {COLUMNS} FROM match_suggestions WHERE id=?1"),[id],row).optional()?.ok_or_else(conflict)}
pub fn open(c:&Connection,rid:&str)->Result<Value,String>{c.query_row(&format!("SELECT {COLUMNS} FROM match_suggestions WHERE resource_id=?1 AND state='open'"),[rid],row).optional().map(|v|v.unwrap_or(Value::Null)).map_err(|e|e.to_string())}
pub async fn create(State(a):State<App>,h:HeaderMap,Path(rid):Path<String>,Json(v):Json<Create>)->R<Json<Value>>{
 let source=writable(&a,&h)?;validate(&v)?;available(&a.db.lock().unwrap(),&rid,v.revision)?;
 let mut candidates=Vec::new();for id in &v.works{candidates.push(matching::fetch_work(id).await?);}
 let mut c=a.db.lock().unwrap();let tx=c.transaction()?;let result=store(&tx,&rid,&v,&source,&candidates)?;tx.commit()?;Ok(Json(result))
}
pub async fn for_resource(State(a):State<App>,h:HeaderMap,Path(rid):Path<String>)->R<Json<Value>>{crate::auth(&a,&h)?;Ok(Json(json!({"suggestion":open(&a.db.lock().unwrap(),&rid)?})))}
pub async fn list(State(a):State<App>,h:HeaderMap,Query(q):Query<Page>)->R<Json<Value>>{
 crate::auth(&a,&h)?;if q.before.is_some_and(|n|n<=0){return Err(Error("Invalid suggestion cursor".into()));}let c=a.db.lock().unwrap();let mut s=c.prepare(&format!("SELECT {COLUMNS},rowid FROM match_suggestions WHERE state='open' AND rowid<?1 ORDER BY rowid DESC LIMIT 61"))?;let mut rows=s.query_map([q.before.unwrap_or(i64::MAX)],|r|Ok((row(r)?,r.get::<_,i64>(12)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;let more=rows.len()>60;rows.truncate(60);let next=if more{rows.last().map(|r|r.1)}else{None};Ok(Json(json!({"items":rows.into_iter().map(|r|r.0).collect::<Vec<_>>(),"next":next})))
}
fn pending(c:&Connection,id:&str,revision:i64,verified_only:bool)->R<Value>{let v=get(c,id)?;if v["state"]!="open"||v["resource_revision"]!=revision{return Err(conflict());}available(c,v["resource_id"].as_str().unwrap(),revision)?;if verified_only&&v["verified"]!=true{return Err(Error("Only verified suggestions may be batch confirmed".into()));}Ok(v)}
fn work_fields(v:&Value)->Value{let title=v["title"].as_str().unwrap_or("");json!({"vndb_id":v["id"],"title":title,"original_title":v["alttitle"].as_str().filter(|s|!s.is_empty()).unwrap_or(title),"description":v["description"].as_str().unwrap_or(""),"cover":v["image"]["url"].as_str().unwrap_or(""),"developer":v["developers"].as_array().into_iter().flatten().filter_map(|d|d["name"].as_str()).collect::<Vec<_>>().join(", "),"released":v["released"].as_str().unwrap_or(""),"tags":v["tags"].as_array().into_iter().flatten().filter_map(|t|t["name"].as_str()).collect::<Vec<_>>()})}
fn bind(db:&Db,id:&str,revision:i64,verified_only:bool,candidates:&[Value])->R<Value>{
 let mut c=db.lock().unwrap();let tx=c.transaction()?;let v=pending(&tx,id,revision,verified_only)?;let rid=v["resource_id"].as_str().unwrap();let mut bindings=Vec::new();
 for work in v["works"].as_array().unwrap(){let candidate=candidates.iter().find(|c|c["id"]==work["vndb_id"]).ok_or_else(||Error("VNDB work not found".into()))?;let work=crate::create_work_in(&tx,&work_fields(candidate))?;let wid=work["id"].as_str().unwrap().to_owned();if bindings.iter().any(|b:&editions::Binding|b.work_id==wid){continue;}bindings.push(editions::Binding{work_id:wid,release_id:None,role:"main".into()});}
 // The same write primitive as the manual match dialog, with every collection member a main work.
 let result=editions::manual_bindings_in(&tx,rid,editions::EditBindings{revision,primary_work_id:bindings.first().map(|b|b.work_id.clone()),bindings},"manual.bind")?;
 tx.execute("UPDATE match_suggestions SET state='confirmed',updated=?2 WHERE id=?1",params![id,db::now()])?;tx.commit()?;Ok(json!({"id":id,"state":"confirmed","resource":result}))
}
async fn confirm_one(db:&Db,id:&str,revision:i64,verified_only:bool)->R<Value>{let v=pending(&db.lock().unwrap(),id,revision,verified_only)?;let mut candidates=Vec::new();for work in v["works"].as_array().unwrap(){candidates.push(matching::fetch_work(work["vndb_id"].as_str().unwrap()).await?);}bind(db,id,revision,verified_only,&candidates)}
pub async fn confirm(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Revision>)->R<Json<Value>>{writable(&a,&h)?;Ok(Json(confirm_one(&a.db,&id,v.revision,false).await?))}
pub async fn batch_confirm(State(a):State<App>,h:HeaderMap,Json(v):Json<Batch>)->R<Json<Value>>{writable(&a,&h)?;if v.ids.is_empty()||v.ids.len()>60||v.ids.len()!=v.revisions.len()||v.ids.iter().collect::<std::collections::HashSet<_>>().len()!=v.ids.len(){return Err(Error("Supply 1–60 unique ids and corresponding revisions".into()));}let mut results=Vec::new();for(id,revision)in v.ids.iter().zip(v.revisions){results.push(match confirm_one(&a.db,id,revision,true).await{Ok(value)=>value,Err(e)=>json!({"id":id,"error":e.0,"confirmed":false})});}Ok(Json(json!({"results":results})))}
pub async fn dismiss(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->R<Json<Value>>{writable(&a,&h)?;let c=a.db.lock().unwrap();if c.execute("UPDATE match_suggestions SET state='dismissed',updated=?2 WHERE id=?1 AND state='open'",params![id,db::now()])?!=1{return Err(conflict());}Ok(Json(json!({"id":id,"state":"dismissed"})))}
#[cfg(test)]mod tests;

//! Immutable smart-list result pages. Call build inside a catalog transaction.
use crate::{ApiError,smart_lists::{Definition,Scope,Sort},smart_rules::{WorkFacts,Truth}};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
use axum::{extract::{State,Path,Query},http::HeaderMap,Json};
type R<T>=Result<T,ApiError>;
pub fn migrate(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS smart_snapshots(id TEXT PRIMARY KEY,list_id TEXT NOT NULL REFERENCES smart_lists(id),definition_revision INTEGER NOT NULL,created INTEGER NOT NULL,total INTEGER NOT NULL,unknown_count INTEGER NOT NULL,collection_count INTEGER NOT NULL,external_count INTEGER NOT NULL,input_digest TEXT NOT NULL); CREATE TABLE IF NOT EXISTS smart_snapshot_entries(snapshot_id TEXT NOT NULL REFERENCES smart_snapshots(id),position INTEGER NOT NULL,work_key TEXT NOT NULL,title TEXT NOT NULL,PRIMARY KEY(snapshot_id,position),UNIQUE(snapshot_id,work_key)); CREATE TABLE IF NOT EXISTS smart_snapshot_requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,snapshot_id TEXT NOT NULL REFERENCES smart_snapshots(id)); CREATE TABLE IF NOT EXISTS smart_snapshot_freshness(snapshot_id TEXT PRIMARY KEY REFERENCES smart_snapshots(id),expires INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS snapshots_list ON smart_snapshots(list_id,created); CREATE TABLE IF NOT EXISTS smart_snapshot_cache(snapshot_id TEXT PRIMARY KEY REFERENCES smart_snapshots(id),lease_until INTEGER NOT NULL DEFAULT 0,expired INTEGER NOT NULL DEFAULT 0);")}
// Caller holds the catalog mutex; pruning runs in a transaction.
fn require_cached(c:&Connection,id:&str)->R<()>{
 let expired:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM smart_snapshot_cache WHERE snapshot_id=?1 AND expired=1)",[id],|r|r.get(0))?;
 if expired{return Err(ApiError("Snapshot expired. Reload the smart list to review current results.".into()));}Ok(())
}
pub fn prune(c:&Connection,now:i64)->R<usize>{
 let mut q=c.prepare("SELECT s.id FROM smart_snapshots s LEFT JOIN smart_snapshot_cache k ON k.snapshot_id=s.id WHERE coalesce(k.expired,0)=0 AND max(s.created+1800,coalesce(k.lease_until,0))<?1 AND (SELECT count(*) FROM smart_snapshots newer WHERE newer.list_id=s.list_id AND newer.rowid>s.rowid)>=3 LIMIT 20")?;
 let ids=q.query_map([now],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for id in &ids{
 c.execute("DELETE FROM smart_snapshot_entries WHERE snapshot_id=?1",[id])?;
 c.execute("INSERT INTO smart_snapshot_cache(snapshot_id,expired) VALUES(?1,1) ON CONFLICT(snapshot_id) DO UPDATE SET expired=1",[id])?;
 }
 Ok(ids.len())
}
pub(crate) fn latest_expired(c:&Connection,list:&str,now:i64)->R<bool>{
 Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM smart_snapshot_freshness WHERE snapshot_id=(SELECT id FROM smart_snapshots WHERE list_id=?1 ORDER BY rowid DESC LIMIT 1) AND expires<=?2)",params![list,now],|r|r.get(0))?)
}
pub(crate) struct Evaluation{valid_until:Option<i64>,results:Vec<(String,String,Option<u16>)>,unknown:i64,collection:i64,external:i64,input_digest:String}
fn check_cancel(cancelled:&dyn Fn()->bool)->R<()>{if cancelled(){Err(ApiError("Smart computation stopped".into()))}else{Ok(())}}
pub(crate) fn evaluate_scope(c:&Connection,definition:&Definition,root_online:&BTreeMap<String,bool>,cancelled:&dyn Fn()->bool,now:i64)->R<Evaluation>{
 check_cancel(cancelled)?;
 if definition.schema_version!=1{return Err(ApiError("Unsupported rule version".into()));}crate::smart_rules::validate(&definition.rule).map_err(ApiError)?;
 if !crate::smart_lists::broken_dependencies(c,definition)?.is_empty(){return Err(ApiError("Repair missing or deleted tag conditions before previewing".into()));}
 let mut q=c.prepare("SELECT id,title FROM works WHERE merged_into IS NULL ORDER BY id LIMIT 100001")?;let locals=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let needs=crate::smart_facts::Needs::for_rule(&definition.rule);
 let mut candidates=Vec::<(String,String,WorkFacts,bool)>::new();let mut identities=BTreeSet::new();for(id,title)in locals{check_cancel(cancelled)?;let key=format!("local:{id}");identities.insert(crate::lists::identity(c,&key)?);candidates.push((key,title,crate::smart_facts::load_needed_at(c,&id,root_online,now,Some(&needs)).map_err(ApiError)?,true));}
 if matches!(definition.scope,Scope::StoredReferences){let mut q=c.prepare("SELECT e.work_key,e.title FROM list_entries e JOIN curated_lists l ON l.id=e.list_id WHERE l.deleted=0 ORDER BY e.added,e.id LIMIT 100001")?;let refs=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;if refs.len()>100000{return Err(ApiError("Stored reference scope exceeds the current computation limit".into()));}for(key,title)in refs{check_cancel(cancelled)?;let identity=crate::lists::identity(c,&key)?;if identities.insert(identity){let facts=crate::smart_facts::reference(c,&key,now).map_err(ApiError)?;candidates.push((key,title,facts,false));}}}
 if candidates.len()>100000{return Err(ApiError("Scope exceeds 100000 works; narrow the source before computing".into()));}
 let valid_until=candidates.iter().filter_map(|(_,_,facts,_)|facts.valid_until).min();
 let mut digest=Sha256::new();digest.update(serde_json::to_vec(&definition).map_err(|e|ApiError(e.to_string()))?);let mut results=Vec::new();let(mut unknown,mut collection,mut external)=(0,0,0);
 for (key,title,facts,local) in candidates{check_cancel(cancelled)?;digest.update(serde_json::to_vec(&(&key,&title,&facts)).map_err(|e|ApiError(e.to_string()))?);match crate::smart_rules::evaluate_checked(&definition.rule,&facts).map_err(ApiError)?{Truth::Yes=>{if local{collection+=1}else{external+=1}results.push((key,title,facts.year));},Truth::Unknown=>unknown+=1,Truth::No=>{}}}
 results.sort_by(|a,b|{let order=match definition.sort{Sort::Title=>{let cmp=a.1.to_lowercase().cmp(&b.1.to_lowercase());if definition.descending{cmp.reverse()}else{cmp}},Sort::Year=>match(a.2,b.2){(Some(a),Some(b))=>if definition.descending{b.cmp(&a)}else{a.cmp(&b)},(None,None)=>std::cmp::Ordering::Equal,(None,_)=>std::cmp::Ordering::Greater,(_,None)=>std::cmp::Ordering::Less}};order.then_with(||a.0.cmp(&b.0))});
 Ok(Evaluation{valid_until,results,unknown,collection,external,input_digest:hex::encode(digest.finalize())})
}
pub fn preview_definition(c:&Connection,definition:&Definition,root_online:&BTreeMap<String,bool>)->R<Value>{preview_checked(c,definition,root_online,&||false)}
fn preview_checked(c:&Connection,definition:&Definition,root_online:&BTreeMap<String,bool>,cancelled:&dyn Fn()->bool)->R<Value>{let result=evaluate_scope(c,definition,root_online,cancelled,crate::db::now())?;Ok(json!({"total":result.results.len(),"unknown_count":result.unknown,"collection_count":result.collection,"external_count":result.external,"sample":result.results.iter().take(10).map(|(key,title,_)|json!({"work_key":key,"title":title})).collect::<Vec<_>>() }))}
static PREVIEW_GATE:std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>>=std::sync::OnceLock::new();
struct PreviewCancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for PreviewCancellation{fn drop(&mut self){self.0.store(true,std::sync::atomic::Ordering::SeqCst);}}
pub async fn preview(State(a):State<crate::App>,h:HeaderMap,Json(definition):Json<Definition>)->crate::Result<Value>{
 crate::auth(&a,&h)?;
 let permit=PREVIEW_GATE.get_or_init(||std::sync::Arc::new(tokio::sync::Semaphore::new(2))).clone().try_acquire_owned().map_err(|_|ApiError("Other previews are running. Retry in a moment.".into()))?;
 let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));let _guard=PreviewCancellation(cancelled.clone());
 let path=a.state_dir.join("library.sqlite");
 let result=tokio::task::spawn_blocking(move||->R<Value>{
  let _permit=permit;let started=std::time::Instant::now();crate::diagnostics::record("info","smart.preview.started","Preview started",None);
  let mut read=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let tx=read.transaction()?;
  let roots={let mut q=tx.prepare("SELECT id,path FROM roots")?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
  let online=roots.into_iter().map(|(id,path)|(id,std::fs::metadata(path).is_ok_and(|m|m.is_dir()))).collect();
  let result=preview_checked(&tx,&definition,&online,&||cancelled.load(std::sync::atomic::Ordering::SeqCst));
  let code=if cancelled.load(std::sync::atomic::Ordering::SeqCst){"smart.preview.cancelled"}else if result.is_ok(){"smart.preview.completed"}else{"smart.preview.failed"};
  crate::diagnostics::record("info",code,&format!("Preview finished in {} ms",started.elapsed().as_millis()),None);result
 }).await.map_err(|e|ApiError(e.to_string()))??;
 Ok(Json(result))
}

#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct Build{pub revision:i64,pub request_id:String}
fn replay(c:&Connection,list:&str,input:&Build)->R<Option<String>>{
 if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}
 let wanted=hex::encode(Sha256::digest(serde_json::to_vec(&(list,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((digest,id))=c.query_row("SELECT digest,snapshot_id FROM smart_snapshot_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{
  if digest!=wanted{return Err(ApiError("Request ID already used".into()));}require_cached(c,&id)?;return Ok(Some(id));
 }Ok(None)
}
pub fn build(c:&Connection,list:&str,input:&Build,root_online:&BTreeMap<String,bool>)->R<String>{build_cancellable(c,list,input,root_online,&||false)}
pub fn build_cancellable(c:&Connection,list:&str,input:&Build,root_online:&BTreeMap<String,bool>,cancelled:&dyn Fn()->bool)->R<String>{build_at(c,list,input,root_online,cancelled,crate::db::now())}
pub(crate) fn build_at(c:&Connection,list:&str,input:&Build,root_online:&BTreeMap<String,bool>,cancelled:&dyn Fn()->bool,now:i64)->R<String>{build_using(c,list,input,root_online,cancelled,now,None)}
pub(crate) fn build_using(c:&Connection,list:&str,input:&Build,root_online:&BTreeMap<String,bool>,cancelled:&dyn Fn()->bool,now:i64,prepared:Option<Evaluation>)->R<String>{
 check_cancel(cancelled)?;
 if input.request_id.is_empty()||input.request_id.len()>128{return Err(ApiError("Request ID required".into()));}let request_digest=hex::encode(Sha256::digest(serde_json::to_vec(&(list,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some(id)=replay(c,list,input)?{return Ok(id);}

 let row=crate::smart_lists::read_one(c,list)?;if row["deleted"]==true||row["revision"].as_i64()!=Some(input.revision){return Err(ApiError("Smart list changed. Reload before computing.".into()));}if row["needs_repair"]==true{return Err(ApiError("Repair tag dependencies before computing. Previous results are retained.".into()));}
 let definition:Definition=serde_json::from_value(row["definition"].clone()).map_err(|e|ApiError(e.to_string()))?;crate::smart_rules::validate(&definition.rule).map_err(ApiError)?;
 let Evaluation{valid_until,results,unknown,collection,external,input_digest}=match prepared{Some(result)=>result,None=>evaluate_scope(c,&definition,root_online,cancelled,now)?};
 check_cancel(cancelled)?;let id=crate::db::id();c.execute("INSERT INTO smart_snapshots(id,list_id,definition_revision,created,total,unknown_count,collection_count,external_count,input_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![id,list,input.revision,crate::db::now(),results.len() as i64,unknown,collection,external,input_digest])?;
 if let Some(expires)=valid_until{c.execute("INSERT INTO smart_snapshot_freshness(snapshot_id,expires) VALUES(?1,?2)",params![id,expires])?;}
 for (position,(key,title,_)) in results.iter().enumerate(){check_cancel(cancelled)?;c.prepare_cached("INSERT INTO smart_snapshot_entries(snapshot_id,position,work_key,title) VALUES(?1,?2,?3,?4)")?.execute(params![id,position as i64,key,title])?;}
 check_cancel(cancelled)?;c.execute("INSERT INTO smart_snapshot_requests(id,digest,snapshot_id) VALUES(?1,?2,?3)",params![input.request_id,request_digest,id])?;Ok(id)
}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct Convert{pub snapshot:String,pub expected_total:i64,pub name:String,pub request_id:String}
pub fn convert(c:&Connection,list:&str,input:&Convert)->R<Value>{
 if input.request_id.is_empty()||input.request_id.len()>128||input.name.trim().is_empty()||input.name.chars().count()>80||input.name.chars().any(char::is_control){return Err(ApiError("Valid name and request ID required".into()));}
 let digest=hex::encode(Sha256::digest(serde_json::to_vec(&("snapshot_to_manual",list,input)).map_err(|e|ApiError(e.to_string()))?));
 if let Some((saved,result))=c.query_row("SELECT digest,result FROM smart_requests WHERE id=?1",[&input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if saved!=digest{return Err(ApiError("Request ID already used".into()));}return serde_json::from_str(&result).map_err(|e|ApiError(e.to_string()));}
 let current=crate::smart_lists::read_one(c,list)?;if current["deleted"]==true||current["needs_repair"]==true{return Err(ApiError("Restore or repair the smart list before converting".into()));}
 let(total,revision):(i64,i64)=c.query_row("SELECT total,definition_revision FROM smart_snapshots WHERE id=?1 AND list_id=?2",params![input.snapshot,list],|r|Ok((r.get(0)?,r.get(1)?)))?;
 if total!=input.expected_total||current["revision"].as_i64()!=Some(revision){return Err(ApiError("Result scope changed. Review the snapshot again.".into()));}
 require_cached(c,&input.snapshot)?;
 let actual:i64=c.query_row("SELECT count(*) FROM smart_snapshot_entries WHERE snapshot_id=?1",[&input.snapshot],|r|r.get(0))?;if actual!=total{return Err(ApiError("Snapshot is incomplete".into()));}
 let id=crate::db::id();c.execute("INSERT INTO curated_lists(id,name,description) VALUES(?1,?2,?3)",params![id,input.name.trim(),current["description"].as_str().unwrap_or("")])?;
 let mut q=c.prepare("SELECT position,work_key,title FROM smart_snapshot_entries WHERE snapshot_id=?1 ORDER BY position")?;let entries=q.query_map([&input.snapshot],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let added=crate::db::now();for(position,key,title)in entries{c.execute("INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES(?1,?2,?3,?4,?5,?6)",params![crate::db::id(),id,key,title,position,added])?;}
 let result=json!({"id":id,"revision":1,"count":total,"source_snapshot":input.snapshot});c.execute("INSERT INTO smart_requests(id,digest,result) VALUES(?1,?2,?3)",params![input.request_id,digest,result.to_string()])?;Ok(result)
}
pub async fn to_manual(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Convert>)->crate::Result<Value>{crate::auth(&a,&h)?;let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;let result=convert(&tx,&id,&input)?;tx.commit()?;Ok(Json(result))}
#[derive(Deserialize,Default)]pub struct Page{pub snapshot:Option<String>,pub after:Option<i64>}
pub fn page(c:&Connection,list:&str,q:&Page)->R<Value>{
 if q.after.is_some()&&q.snapshot.is_none(){return Err(ApiError("Snapshot required for continuation".into()));}if q.after.is_some_and(|v|v<0){return Err(ApiError("Invalid snapshot position".into()));}
 let current=crate::smart_lists::read_one(c,list)?;
 let catalog:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
 let state:Option<(i64,String)>=c.query_row("SELECT catalog_revision,error FROM smart_compute_state WHERE list_id=?1",[list],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
 let pending=current["deleted"]!=true&&(state.as_ref().is_none_or(|(revision,_)|*revision!=catalog)||latest_expired(c,list,crate::db::now())?);
 let update_error=state.map(|(_,error)|error).unwrap_or_default();
 let latest:Option<String>=c.query_row("SELECT id FROM smart_snapshots WHERE list_id=?1 ORDER BY rowid DESC LIMIT 1",[list],|r|r.get(0)).optional()?;
 let id=match &q.snapshot{Some(id)=>Some(id.clone()),None=>c.query_row("SELECT id FROM smart_snapshots WHERE list_id=?1 ORDER BY rowid DESC LIMIT 1",[list],|r|r.get::<_,String>(0)).optional()?};
 let Some(id)=id else{return Ok(json!({"computed":false,"needs_repair":current["needs_repair"],"update_pending":pending,"update_error":update_error,"entries":[]}));};
 let mut result=c.query_row("SELECT definition_revision,created,total,unknown_count,collection_count,external_count,input_digest FROM smart_snapshots WHERE id=?1 AND list_id=?2",params![id,list],|r|Ok(json!({"computed":true,"snapshot":id,"definition_revision":r.get::<_,i64>(0)?,"created":r.get::<_,i64>(1)?,"total":r.get::<_,i64>(2)?,"unknown_count":r.get::<_,i64>(3)?,"collection_count":r.get::<_,i64>(4)?,"external_count":r.get::<_,i64>(5)?,"input_digest":r.get::<_,String>(6)?})))?;
 require_cached(c,&id)?;
 c.execute("INSERT INTO smart_snapshot_cache(snapshot_id,lease_until) VALUES(?1,?2) ON CONFLICT(snapshot_id) DO UPDATE SET lease_until=excluded.lease_until WHERE lease_until<?2-60",params![id,crate::db::now()+1800])?;
 result["update_pending"]=json!(pending);result["update_error"]=json!(update_error);result["newer_available"]=json!(latest.as_deref()!=Some(id.as_str()));
 result["needs_repair"]=current["needs_repair"].clone();result["definition_changed"]=json!(result["definition_revision"]!=current["revision"]);
 let mut statement=c.prepare("SELECT position,work_key,title FROM smart_snapshot_entries WHERE snapshot_id=?1 AND position>?2 ORDER BY position LIMIT 51")?;let mut rows=statement.query_map(params![id,q.after.unwrap_or(-1)],|r|Ok(json!({"position":r.get::<_,i64>(0)?,"work_key":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;let more=rows.len()>50;rows.truncate(50);result["next_after"]=if more{rows.last().unwrap()["position"].clone()}else{Value::Null};result["entries"]=json!(rows);Ok(result)
}
static COMPUTE_GATE:std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>>=std::sync::OnceLock::new();
fn compute_unlocked(a:&crate::App,id:&str,input:&Build,cancelled:&dyn Fn()->bool,before_publish:&dyn Fn())->R<String>{
 {let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;if let Some(id)=replay(&c,id,input)?{return Ok(id);}}
 let catalog_path={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;c.path().ok_or_else(||ApiError("Catalog path unavailable".into()))?.to_owned()};
 let _claim=crate::smart_updates::claim(&catalog_path,id).ok_or_else(||ApiError("This smart list is already computing. Retry when it finishes.".into()))?;
 let (observed,roots)={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
  let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
  let mut q=c.prepare("SELECT id,path FROM roots")?;
  let roots=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;(revision,roots)};
 let online=roots.into_iter().map(|(id,path)|(id,std::fs::metadata(path).is_ok_and(|m|m.is_dir()))).collect();
 check_cancel(cancelled)?;
 let expected={let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
  let current:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
  if current!=observed{return Err(ApiError("Collection changed during computation. Retry to use the latest data.".into()));}
  crate::smart_updates::observe_roots(&c,&online).map_err(ApiError)?;
  c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0))?};
 let (revision,prepared)={
  let mut read=Connection::open_with_flags(a.state_dir.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let tx=read.transaction()?;
  let revision:i64=tx.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
  if revision!=expected{return Err(ApiError("Collection changed during computation. Retry to use the latest data.".into()));}
  let row=crate::smart_lists::read_one(&tx,id)?;
  if row["deleted"]==true||row["revision"].as_i64()!=Some(input.revision){return Err(ApiError("Smart list changed. Reload before computing.".into()));}
  let definition:Definition=serde_json::from_value(row["definition"].clone()).map_err(|e|ApiError(e.to_string()))?;
  let prepared=evaluate_scope(&tx,&definition,&online,cancelled,crate::db::now())?;(revision,prepared)
 };
 before_publish();check_cancel(cancelled)?;
 let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;
 if let Some(id)=replay(&tx,id,input)?{return Ok(id);}
 let current:i64=tx.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0))?;
 if current!=revision{return Err(ApiError("Collection changed during computation. Retry to use the latest data.".into()));}
 let now=crate::db::now();let snapshot=build_using(&tx,id,input,&online,cancelled,now,Some(prepared))?;
 tx.execute("INSERT INTO smart_compute_state(list_id,catalog_revision,last_attempt,error) VALUES(?1,?2,?3,'') ON CONFLICT(list_id) DO UPDATE SET catalog_revision=excluded.catalog_revision,last_attempt=excluded.last_attempt,error=''",params![id,revision,now])?;
 check_cancel(cancelled)?;tx.commit()?;Ok(snapshot)
}
pub async fn refresh(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Json(input):Json<Build>)->crate::Result<Value>{
 crate::auth(&a,&h)?;
 let permit=COMPUTE_GATE.get_or_init(||std::sync::Arc::new(tokio::sync::Semaphore::new(2))).clone().try_acquire_owned().map_err(|_|ApiError("Other computations are running. Retry in a moment.".into()))?;
 let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));let _guard=PreviewCancellation(cancelled.clone());
 let snapshot=tokio::task::spawn_blocking(move||{let _permit=permit;
  let started=std::time::Instant::now();crate::diagnostics::record("info","smart.manual.started","Manual computation started",None);
  let result=compute_unlocked(&a,&id,&input,&||cancelled.load(std::sync::atomic::Ordering::SeqCst),&||{});
  let code=if result.is_ok(){"smart.manual.completed"}else if cancelled.load(std::sync::atomic::Ordering::SeqCst){"smart.manual.cancelled"}else{"smart.manual.failed"};
  crate::diagnostics::record(if result.is_ok()||code=="smart.manual.cancelled"{"info"}else{"error"},code,&format!("Manual computation ended after {} ms",started.elapsed().as_millis()),None);result
 }).await.map_err(|e|ApiError(e.to_string()))??;
 Ok(Json(json!({"snapshot":snapshot})))
}

pub async fn read(State(a):State<crate::App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<Page>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;if who.role=="web"&&crate::smart_lists::read_one(&c,&id)?["deleted"]==true{return Err(ApiError("Desktop access required".into()));}Ok(Json(page(&c,&id,&q)?))}

#[cfg(test)]mod tests{
 use super::*;use crate::smart_rules::{Rule,Field,Predicate,SetMode};
 fn fixture()->(tempfile::TempDir,crate::db::Db){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let definition=Definition{schema_version:1,scope:Scope::Collection,sort:Sort::Title,descending:false,rule:Rule::Field{predicate:Predicate{field:Field::PersonalTags,mode:SetMode::Any,values:vec!["tag".into()],exclude:false,include_unknown:false}}};{let c=db.lock().unwrap();c.execute("INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag')",[]).unwrap();c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','Smart','',1,?1)",[serde_json::to_string(&definition).unwrap()]).unwrap();for n in 0..55{let id=format!("w{n:02}");c.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?1,?1)",[&id]).unwrap();c.execute("INSERT INTO custom_tag_works VALUES('tag',?1)",[&id]).unwrap();}}(t,db)}
 fn compute(db:&crate::db::Db,request:&str)->R<String>{let mut c=db.lock().unwrap();let tx=c.transaction()?;let id=build(&tx,"s",&Build{revision:1,request_id:request.into()},&BTreeMap::new())?;tx.commit()?;Ok(id)}
 #[test]fn retention_protects_readers_and_latest_results_without_replaying_expired_payloads(){
 let(_t,db)=fixture();let old=compute(&db,"old").ok().unwrap();let idle=compute(&db,"idle").ok().unwrap();
 for request in ["three","four","five"]{compute(&db,request).ok().unwrap();}
 let mut c=db.lock().unwrap();let now=crate::db::now();
 c.execute("UPDATE smart_snapshots SET created=?1",[now-4000]).unwrap();
 page(&c,"s",&Page{snapshot:Some(old.clone()),after:None}).ok().unwrap();
 {let tx=c.transaction().unwrap();assert_eq!(prune(&tx,now).ok().unwrap(),1);tx.commit().unwrap();}
 assert!(page(&c,"s",&Page{snapshot:Some(idle.clone()),after:None}).err().unwrap().0.contains("expired"));
 assert_eq!(page(&c,"s",&Page{snapshot:Some(old.clone()),after:Some(49)}).ok().unwrap()["entries"].as_array().unwrap().len(),5);
 assert_eq!(page(&c,"s",&Page::default()).ok().unwrap()["total"],55);
 let count=c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap();
 assert!(build(&c,"s",&Build{revision:1,request_id:"idle".into()},&BTreeMap::new()).err().unwrap().0.contains("expired"));
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),count);
 assert!(convert(&c,"s",&Convert{snapshot:idle,expected_total:55,name:"Expired".into(),request_id:"convert-expired".into()}).is_err());
 assert_eq!(c.query_row("SELECT count(*) FROM curated_lists",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 {let tx=c.transaction().unwrap();assert_eq!(prune(&tx,now+1900).ok().unwrap(),1);tx.commit().unwrap();}
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_entries",[],|r|r.get::<_,i64>(0)).unwrap(),165);
 assert!(page(&c,"s",&Page{snapshot:Some(old),after:Some(49)}).is_err());
 }
 #[test]fn manual_compute_discards_changes_and_replays_receipts_without_reevaluation(){
 let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().into()).unwrap();
 {let c=app.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title) VALUES('w','W','W')",[]).unwrap();let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('s','S','',1,?1)",[definition.to_string()]).unwrap();}
 let input=Build{revision:1,request_id:"manual".into()};
 assert!(compute_unlocked(&app,"s",&input,&||false,&||{app.db.try_lock().unwrap().execute("UPDATE works SET title='Changed'",[]).unwrap();}).err().unwrap().0.contains("Collection changed"));
 assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshot_requests",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 app.db.lock().unwrap().execute("UPDATE smart_catalog_state SET root_digest=''",[]).unwrap();
 let snapshot=compute_unlocked(&app,"s",&input,&||false,&||{crate::smart_updates::observe_roots(&app.db.try_lock().unwrap(),&BTreeMap::new()).unwrap();}).ok().unwrap();
 app.db.lock().unwrap().execute("UPDATE works SET status='playing'",[]).unwrap();
 assert_eq!(compute_unlocked(&app,"s",&input,&||false,&||panic!("Retry must not reevaluate")).ok().unwrap(),snapshot);
 assert!(compute_unlocked(&app,"s",&Build{revision:2,request_id:"manual".into()},&||false,&||{}).is_err());
 assert!(compute_unlocked(&app,"s",&Build{revision:1,request_id:"cancel".into()},&||true,&||{}).is_err());
 assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn cancellation_during_snapshot_writes_rolls_back_to_last_complete_result(){
 let(_t,db)=fixture();let old=compute(&db,"initial").ok().unwrap();let mut c=db.lock().unwrap();{let tx=c.transaction().unwrap();let cancellation=||tx.query_row("SELECT count(*) FROM smart_snapshot_entries WHERE snapshot_id<>?1",[&old],|r|r.get::<_,i64>(0)).unwrap()>=25;let result=build_cancellable(&tx,"s",&Build{revision:1,request_id:"cancelled".into()},&BTreeMap::new(),&cancellation);assert!(result.is_err());assert_eq!(tx.query_row("SELECT count(*) FROM smart_snapshot_entries WHERE snapshot_id<>?1",[&old],|r|r.get::<_,i64>(0)).unwrap(),25);}
 assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT count(*) FROM smart_snapshot_requests WHERE id='cancelled'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(page(&c,"s",&Page::default()).ok().unwrap()["snapshot"],old);
 }
 #[tokio::test]async fn preview_handler_uses_readonly_connection_and_bounds_work(){
 let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().into()).unwrap();
 app.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title) VALUES('w','Fixture','Fixture')",[]).unwrap();
 let definition:Definition=serde_json::from_value(json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}})).unwrap();
 let mut h=HeaderMap::new();h.insert("authorization",format!("Bearer {}",app.token).parse().unwrap());
 let before=app.db.lock().unwrap().total_changes();
 let result=preview(State(app.clone()),h.clone(),Json(definition.clone())).await.ok().unwrap().0;assert_eq!(result["total"],1);assert_eq!(app.db.lock().unwrap().total_changes(),before);
 let gate=PREVIEW_GATE.get().unwrap().clone();let _permits=gate.acquire_many_owned(2).await.unwrap();
 assert!(preview(State(app.clone()),h,Json(definition.clone())).await.err().unwrap().0.contains("Other previews"));
 let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));drop(PreviewCancellation(cancelled.clone()));
 assert!(preview_checked(&app.db.lock().unwrap(),&definition,&BTreeMap::new(),&||cancelled.load(std::sync::atomic::Ordering::SeqCst)).is_err());
 }
 #[test]fn unsaved_preview_matches_full_count_without_persisting_anything(){let(_t,db)=fixture();let c=db.lock().unwrap();let mut definition:Definition=serde_json::from_value(crate::smart_lists::read_one(&c,"s").ok().unwrap()["definition"].clone()).unwrap();let before=c.total_changes();let result=preview_definition(&c,&definition,&BTreeMap::new()).ok().unwrap();assert_eq!(result["total"],55);assert_eq!(result["sample"].as_array().unwrap().len(),10);assert_eq!(before,c.total_changes());definition.rule=Rule::Year{from:2000,to:2010,exclude:true,include_unknown:false};assert_eq!(preview_definition(&c,&definition,&BTreeMap::new()).ok().unwrap()["unknown_count"],55);assert_eq!(before,c.total_changes());}
 #[test]fn conversion_copies_complete_frozen_order_once_and_keeps_smart_definition(){
 let(_t,db)=fixture();let snapshot=compute(&db,"compute").ok().unwrap();let input=Convert{snapshot:snapshot.clone(),expected_total:55,name:"Frozen result".into(),request_id:"convert".into()};let result={let mut c=db.lock().unwrap();c.execute("UPDATE works SET title='Later title' WHERE id='w54'",[]).unwrap();c.execute("DELETE FROM custom_tag_works WHERE work_id='w54'",[]).unwrap();let tx=c.transaction().unwrap();let result=convert(&tx,"s",&input).ok().unwrap();tx.commit().unwrap();result};let c=db.lock().unwrap();assert_eq!(convert(&c,"s",&input).ok().unwrap(),result);let id=result["id"].as_str().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM list_entries WHERE list_id=?1",[id],|r|r.get::<_,i64>(0)).unwrap(),55);assert_eq!(c.query_row("SELECT title FROM list_entries WHERE list_id=?1 AND position=54",[id],|r|r.get::<_,String>(0)).unwrap(),"w54");assert_eq!(c.query_row("SELECT count(*) FROM smart_lists WHERE id='s'",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert!(convert(&c,"s",&Convert{snapshot,expected_total:54,name:"Wrong total".into(),request_id:"bad-count".into()}).is_err());assert_eq!(c.query_row("SELECT count(*) FROM curated_lists",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn conversion_failure_or_wrong_source_never_leaves_partial_manual_list(){
 let(_t,db)=fixture();let snapshot=compute(&db,"compute").ok().unwrap();let input=Convert{snapshot,expected_total:55,name:"Frozen".into(),request_id:"conversion".into()};let mut c=db.lock().unwrap();assert!(convert(&c,"other",&input).is_err());c.execute_batch("CREATE TRIGGER fail_manual BEFORE INSERT ON list_entries WHEN NEW.position=25 BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();{let tx=c.transaction().unwrap();assert!(convert(&tx,"s",&input).is_err());}assert_eq!(c.query_row("SELECT count(*) FROM curated_lists",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM list_entries",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM smart_requests",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn multi_page_snapshot_is_immutable_after_catalog_changes_and_reopen(){
 let(t,db)=fixture();let id=compute(&db,"first").ok().unwrap();assert_eq!(compute(&db,"first").ok().unwrap(),id);{let c=db.lock().unwrap();let first=page(&c,"s",&Page{snapshot:Some(id.clone()),after:None}).ok().unwrap();assert_eq!(first["total"],55);assert_eq!(first["entries"].as_array().unwrap().len(),50);assert_eq!(first["next_after"],49);c.execute("UPDATE works SET title='Changed',status='playing' WHERE id='w54'",[]).unwrap();c.execute("DELETE FROM custom_tag_works WHERE work_id='w54'",[]).unwrap();}
 let newer=compute(&db,"newer").ok().unwrap();assert_ne!(newer,id);{let c=db.lock().unwrap();let old=page(&c,"s",&Page{snapshot:Some(id.clone()),after:Some(49)}).ok().unwrap();assert_eq!(old["entries"].as_array().unwrap().len(),5);assert_eq!(old["entries"][4]["title"],"w54");assert_eq!(old["next_after"],Value::Null);assert!(page(&c,"s",&Page{snapshot:None,after:Some(49)}).is_err());assert_eq!(page(&c,"s",&Page::default()).ok().unwrap()["total"],54);assert!(page(&c,"other",&Page{snapshot:Some(id.clone()),after:None}).is_err());}
 let backup=crate::backup::export(&db,t.path()).unwrap();let dir=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&dir.join("library.sqlite")).unwrap();let old=page(&restored.lock().unwrap(),"s",&Page{snapshot:Some(id),after:Some(49)}).ok().unwrap();assert_eq!(old["entries"][4]["title"],"w54");
 }
 #[test]fn broken_dependencies_retain_old_snapshot_and_failed_compute_rolls_back(){
 let(_t,db)=fixture();let id=compute(&db,"old").ok().unwrap();db.lock().unwrap().execute("UPDATE custom_tags SET deleted=1 WHERE id='tag'",[]).unwrap();assert!(compute(&db,"broken").is_err());{let c=db.lock().unwrap();let old=page(&c,"s",&Page::default()).ok().unwrap();assert_eq!(old["snapshot"],id);assert_eq!(old["needs_repair"],true);c.execute("UPDATE custom_tags SET deleted=0 WHERE id='tag'",[]).unwrap();c.execute_batch("CREATE TRIGGER fail_snapshot BEFORE INSERT ON smart_snapshot_entries BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();}assert!(compute(&db,"failed").is_err());assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshots",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test]fn cached_external_reference_matches_without_becoming_a_collection_work(){
 let(_t,db)=fixture();let mut c=db.lock().unwrap();
 let definition=Definition{schema_version:1,scope:Scope::StoredReferences,sort:Sort::Title,descending:false,rule:Rule::Field{predicate:Predicate{field:Field::SourceTags,mode:SetMode::Any,values:vec!["g1".into()],exclude:false,include_unknown:false}}};
 c.execute("UPDATE smart_lists SET definition=?1",[serde_json::to_string(&definition).unwrap()]).unwrap();
 c.execute_batch("INSERT INTO curated_lists(id,name) VALUES('l','References'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('e','l','vndb:v2','Saved external title',0,1),('duplicate','l','vndb:v3','Uncached',1,1); INSERT INTO curated_lists(id,name) VALUES('l2','Also referenced'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('same','l2','vndb:v2','Other saved title',0,2);").unwrap();
 let now=crate::db::now();let cache=json!({"fetched_at":now,"vn":{"released":"2020-05","tags":[{"id":"g1","name":"Adventure","spoiler":0,"lie":false}]}});
 c.execute("INSERT INTO settings(key,value) VALUES('exploration.v4.work.v2.1',?1)",[cache.to_string()]).unwrap();
 let before=c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap();
 let snapshot={let tx=c.transaction().unwrap();let id=build(&tx,"s",&Build{revision:1,request_id:"external-cached".into()},&BTreeMap::new()).ok().unwrap();tx.commit().unwrap();id};
 let result=page(&c,"s",&Page{snapshot:Some(snapshot),after:None}).ok().unwrap();assert_eq!(result["total"],1);assert_eq!(result["external_count"],1);assert_eq!(result["collection_count"],0);assert_eq!(result["entries"][0]["work_key"],"vndb:v2");assert_eq!(result["entries"][0]["title"],"Saved external title");
 assert_eq!(c.query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),before);
 let facts=crate::smart_facts::reference(&c,"vndb:v2",now).unwrap();assert_eq!(facts.year,Some(2020));assert!(!facts.fields.contains_key(&Field::Status));assert!(!facts.fields.contains_key(&Field::SourceLocations));assert!(!facts.editions_complete);
 let stale=crate::smart_facts::reference(&c,"vndb:v2",now+86400).unwrap();assert_eq!(stale.year,None);assert!(!stale.fields[&Field::SourceTags].complete);assert!(stale.fields[&Field::SourceTags].values.contains("g1"));
 let mut local_only=definition;local_only.scope=Scope::Collection;c.execute("UPDATE smart_lists SET definition=?1",[serde_json::to_string(&local_only).unwrap()]).unwrap();
 assert_eq!(preview_definition(&c,&local_only,&BTreeMap::new()).ok().unwrap()["total"],0);

 }
 #[test]fn stored_reference_scope_deduplicates_known_identity_and_counts_unknown(){
 let(_t,db)=fixture();{let c=db.lock().unwrap();let mut definition:Definition=serde_json::from_value(crate::smart_lists::read_one(&c,"s").ok().unwrap()["definition"].clone()).unwrap();definition.scope=Scope::StoredReferences;c.execute("UPDATE smart_lists SET definition=?1",[serde_json::to_string(&definition).unwrap()]).unwrap();c.execute_batch("UPDATE works SET vndb_id='v1' WHERE id='w00'; INSERT INTO curated_lists(id,name) VALUES('l','Manual'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('a','l','vndb:v1','Owned',0,1),('b','l','vndb:v2','Unloaded',1,2);").unwrap();}compute(&db,"external").ok().unwrap();let result=page(&db.lock().unwrap(),"s",&Page::default()).ok().unwrap();assert_eq!(result["total"],55);assert_eq!(result["unknown_count"],1);assert_eq!(result["external_count"],0);
 }
}

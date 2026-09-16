//! Shared work projection and opt-in bounded traversal for catalog clients.
use axum::{extract::{Path,Query,State},http::HeaderMap,Json};
use rusqlite::{Connection,OptionalExtension,Row};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use crate::{App,ApiError};

const SELECT:&str="SELECT w.id,w.title,w.original_title,w.vndb_id,w.description,w.cover,w.developer,w.released,w.tags,w.status,w.favorite,w.notes,w.revision,(SELECT count(*) FROM resource_bindings r WHERE r.work_id=w.id AND EXISTS(SELECT 1 FROM resource_files rf WHERE rf.resource_id=r.resource_id)),w.aliases,(SELECT preferred_release_id FROM work_preferences WHERE work_id=w.id),w.rowid FROM works w WHERE w.merged_into IS NULL";
fn row(r:&Row<'_>)->rusqlite::Result<Value>{Ok(json!({
 "id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"original_title":r.get::<_,String>(2)?,"vndb_id":r.get::<_,Option<String>>(3)?,"description":r.get::<_,String>(4)?,"cover":r.get::<_,String>(5)?,"developer":r.get::<_,String>(6)?,"released":r.get::<_,String>(7)?,"tags":serde_json::from_str::<Value>(&r.get::<_,String>(8)?).unwrap_or(json!([])),"status":r.get::<_,String>(9)?,"favorite":r.get::<_,i64>(10)?==1,"notes":r.get::<_,String>(11)?,"revision":r.get::<_,i64>(12)?,"resources":r.get::<_,i64>(13)?,"aliases":serde_json::from_str::<Value>(&r.get::<_,String>(14)?).unwrap_or(json!([])),"preferred_release_id":r.get::<_,Option<String>>(15)?,"added_order":r.get::<_,i64>(16)?
}))}
pub fn all(c:&Connection)->rusqlite::Result<Vec<Value>>{
 let mut q=c.prepare(&format!("{SELECT} ORDER BY w.title"))?;let rows=q.query_map([],row)?.collect();rows
}
pub fn one(c:&Connection,id:&str)->rusqlite::Result<Option<Value>>{
 c.query_row(&format!("{SELECT} AND w.id=?1"),[id],row).optional()
}
#[derive(Default,Deserialize)]pub struct ListQuery{#[serde(default)]paged:bool,before:Option<String>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct Cursor{library:String,revision:i64,before:i64}
pub fn page(c:&Connection,before:Option<&str>)->Result<Value,ApiError>{
 let library=crate::context::library(c)?.id;
 let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
 let cursor=before.map(|raw|{
  if raw.len()>256{return Err(ApiError("Invalid collection cursor".into()));}
  let p:Cursor=serde_json::from_str(raw).map_err(|_|ApiError("Invalid collection cursor".into()))?;
  if p.before<=0||p.revision<=0{return Err(ApiError("Invalid collection cursor".into()));}
  if p.library!=library||p.revision!=revision{return Err(ApiError("Collection changed. Reload before continuing.".into()));}
  Ok(p)
 }).transpose()?;
 let total:i64=c.query_row("SELECT count(*) FROM works WHERE merged_into IS NULL",[],|r|r.get(0))?;
 let mut q=c.prepare(&format!("{SELECT} AND w.rowid<?1 ORDER BY w.rowid DESC LIMIT 61"))?;
 let mut items=q.query_map([cursor.as_ref().map_or(i64::MAX,|p|p.before)],row)?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=items.len()>60;items.truncate(60);
 let next=if more{Some(serde_json::to_string(&Cursor{library:library.clone(),revision,before:items.last().unwrap()["added_order"].as_i64().unwrap()}).map_err(|e|ApiError(e.to_string()))?)}else{None};
 Ok(json!({"items":items,"next":next,"total":total,"revision":revision,"library_id":library}))
}
pub async fn list(State(a):State<App>,h:HeaderMap,Query(q):Query<ListQuery>)->crate::Result<Value>{
 crate::auth(&a,&h)?;if q.before.is_some()&&!q.paged{return Err(ApiError("Collection cursor requires paged=true".into()));}
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
 Ok(Json(if q.paged{page(&c,q.before.as_deref())?}else{json!(all(&c)?)}))
}
pub async fn detail(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{
 crate::auth(&a,&h)?;let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
 Ok(Json(one(&c,&id)?.ok_or_else(||ApiError("Work not found or merged".into()))?))
}


#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct LookupQuery{ids:String,#[serde(default)]local:bool,revision:Option<i64>,library_id:Option<String>}
pub fn lookup_page(c:&Connection,q:&LookupQuery)->Result<Value,ApiError>{
 if q.ids.len()>16000{return Err(ApiError("Use at most 60 work identities".into()));}
 let ids:Vec<String>=serde_json::from_str(&q.ids).map_err(|_|ApiError("Invalid work identities".into()))?;
 if ids.len()>60||ids.iter().any(|id|id.is_empty()||id.len()>200||(!q.local&&(!id.starts_with('v')||id.len()<2||id.len()>14||!id[1..].bytes().all(|b|b.is_ascii_digit())))){return Err(ApiError("Use at most 60 valid work identities".into()));}
 if q.revision.is_some()!=q.library_id.is_some(){return Err(ApiError("Both collection revision and identity are required".into()));}
 let library=crate::context::library(c)?.id;let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
 if q.revision.is_some_and(|r|r!=revision)||q.library_id.as_ref().is_some_and(|id|id!=&library){return Err(ApiError("Collection changed. Reload before continuing.".into()));}
 let predicate=if q.local{"w.id=?1"}else{"w.vndb_id IS NOT NULL AND w.vndb_id!='' AND w.vndb_id=?1"};let mut statement=c.prepare(&format!("{SELECT} AND {predicate}"))?;
 let mut seen=std::collections::HashSet::new();let mut items=Vec::new();let mut missing=Vec::new();
 for id in ids{if !seen.insert(id.clone()){continue;}if let Some(work)=statement.query_row([&id],row).optional()?{items.push(work);}else{missing.push(id);}}
 Ok(json!({"items":items,"missing":missing,"revision":revision,"library_id":library}))
}
pub async fn lookup(State(a):State<App>,h:HeaderMap,Query(q):Query<LookupQuery>)->crate::Result<Value>{
 crate::auth(&a,&h)?;read_snapshot(a,move|c|lookup_page(c,&q)).await
}


pub fn work_assets(c:&Connection,id:&str)->Result<Value,ApiError>{
 let work=one(c,id)?.ok_or_else(||ApiError("Work not found or merged".into()))?;
 Ok(json!({"work":work,"resources":crate::resource_catalog::rows(c,Some(id))?,"editions":crate::editions::summaries_for_work(c,id)?,"library_id":crate::context::library(c)?.id,"revision":c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get::<_,i64>(0))?}))
}
pub async fn edition(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{
 crate::auth(&a,&h)?;read_snapshot(a,move|c|crate::editions::one(c,&id)?.ok_or_else(||ApiError("Edition not found".into()))).await
}
pub async fn assets(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->crate::Result<Value>{
 crate::auth(&a,&h)?;read_snapshot(a,move|c|work_assets(c,&id)).await
}
static READ_GATE:std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>>=std::sync::OnceLock::new();
struct CancelRead(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for CancelRead{fn drop(&mut self){self.0.store(true,std::sync::atomic::Ordering::Release);}}
pub(crate) async fn read_snapshot(a:App,action:impl FnOnce(&Connection)->Result<Value,ApiError>+Send+'static)->crate::Result<Value>{
 let permit=READ_GATE.get_or_init(||std::sync::Arc::new(tokio::sync::Semaphore::new(2))).clone().try_acquire_owned().map_err(|_|ApiError("Other work queries are running. Retry in a moment.".into()))?;
 // spawn_blocking outlives its awaiting future. Keep cancellation sticky so it
 // also interrupts SQL started after the request future has already gone away.
 let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));let _cancel=CancelRead(cancelled.clone());
 let value=tokio::task::spawn_blocking(move||{
  let _permit=permit;
  if cancelled.load(std::sync::atomic::Ordering::Acquire){return Err(ApiError("Read cancelled".into()));}
  let mut c=Connection::open_with_flags(a.state_dir.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  c.progress_handler(1000,Some(move||cancelled.load(std::sync::atomic::Ordering::Acquire)))?;
  let tx=c.transaction()?;action(&tx)
 }).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(value))
}

#[cfg(test)]mod tests{
 use super::*;
 use rusqlite::params;
 #[tokio::test]async fn http_disconnect_interrupts_snapshot_query(){
  use tokio::io::AsyncWriteExt;
  let temp=tempfile::tempdir().unwrap();let app=crate::initialize(temp.path().join("state")).unwrap();
  let(started_tx,started_rx)=tokio::sync::oneshot::channel();let(done_tx,mut done_rx)=tokio::sync::oneshot::channel();
  let signals=std::sync::Arc::new(std::sync::Mutex::new(Some((started_tx,done_tx))));
  let router=axum::Router::new().route("/slow",axum::routing::get(move||{
   let app=app.clone();let signals=signals.clone();async move{
    let(started,done)=signals.lock().unwrap().take().unwrap();
    read_snapshot(app,move|c|{
     let _=started.send(c.get_interrupt_handle());
     let result=c.query_row("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n",[],|r|r.get::<_,i64>(0));
     let _=done.send(result.as_ref().err().and_then(rusqlite::Error::sqlite_error_code));Ok(json!(result?))
    }).await
   }
  }));
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();
  let server=tokio::spawn(async move{axum::serve(listener,router).await.unwrap();});
  let mut client=tokio::net::TcpStream::connect(address).await.unwrap();
  client.write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
  let interrupt=tokio::time::timeout(std::time::Duration::from_secs(5),started_rx).await.unwrap().unwrap();
  drop(client);
  let result=tokio::time::timeout(std::time::Duration::from_secs(5),&mut done_rx).await;
  // Always stop the expensive fixture query if disconnect propagation fails.
  if result.is_err(){interrupt.interrupt();let _=tokio::time::timeout(std::time::Duration::from_secs(5),&mut done_rx).await;}
  server.abort();let _=server.await;
  assert_eq!(result.expect("HTTP disconnect did not cancel SQLite").unwrap(),Some(rusqlite::ErrorCode::OperationInterrupted));
 }
 #[tokio::test]async fn dropped_read_future_interrupts_sql_and_releases_worker(){
  let temp=tempfile::tempdir().unwrap();let app=crate::initialize(temp.path().join("state")).unwrap();
  let(started_tx,started_rx)=tokio::sync::oneshot::channel();let(done_tx,done_rx)=tokio::sync::oneshot::channel();
  let pending=tokio::spawn(read_snapshot(app.clone(),move|c|{
   let _=started_tx.send(());
   let result=c.query_row("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n",[],|r|r.get::<_,i64>(0));
   let _=done_tx.send(result.as_ref().err().and_then(rusqlite::Error::sqlite_error_code));
   Ok(json!(result?))
  }));
  tokio::time::timeout(std::time::Duration::from_secs(5),started_rx).await.unwrap().unwrap();
  pending.abort();assert!(pending.await.err().unwrap().is_cancelled());
  let code=tokio::time::timeout(std::time::Duration::from_secs(5),done_rx).await.unwrap().unwrap();assert_eq!(code,Some(rusqlite::ErrorCode::OperationInterrupted));
  // The separate writer and next read remain usable after the cancelled reader.
  app.db.lock().unwrap().execute("UPDATE works SET notes=notes",[]).unwrap();
  let value=read_snapshot(app,|c|Ok(json!(c.query_row("SELECT 42",[],|r|r.get::<_,i64>(0))?))).await.unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(value.0,42);
 }
 #[test]fn lookup_is_bounded_exact_and_rejects_changed_identity(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
  c.execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('local-a','Owned Alpha','','v1'),('local-b','Owned Beta','','v2'),('merged','Old','','v3');UPDATE works SET merged_into='local-a' WHERE id='merged';").unwrap();
  let q=LookupQuery{ids:json!(["v2","v1","v9","v3","v1"]).to_string(),local:false,revision:None,library_id:None};
  let result=lookup_page(&c,&q).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(result["items"].as_array().unwrap().iter().map(|w|w["id"].as_str().unwrap()).collect::<Vec<_>>(),["local-b","local-a"]);assert_eq!(result["missing"],json!(["v9","v3"]));
  let pinned=LookupQuery{ids:"[]".into(),local:false,revision:result["revision"].as_i64(),library_id:result["library_id"].as_str().map(str::to_owned)};assert!(lookup_page(&c,&pinned).is_ok());
  for ids in ["{}".to_owned(),json!(["bad"]).to_string(),json!((0..61).map(|i|format!("v{i}")).collect::<Vec<_>>()).to_string()]{assert!(lookup_page(&c,&LookupQuery{ids,local:false,revision:None,library_id:None}).is_err());}
  let local=lookup_page(&c,&LookupQuery{ids:json!(["local-a","absent"]).to_string(),local:true,revision:None,library_id:None}).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(local["items"][0],one(&c,"local-a").unwrap().unwrap());assert_eq!(local["missing"],json!(["absent"]));
  assert!(lookup_page(&c,&LookupQuery{ids:"[]".into(),local:false,revision:pinned.revision,library_id:Some("other-library".into())}).is_err());
  c.execute("UPDATE works SET title='Changed' WHERE id='local-a'",[]).unwrap();assert!(lookup_page(&c,&pinned).unwrap_err().0.contains("Collection changed"));
 }
 fn scoped_fixture(c:&Connection){
  c.execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('a','Selected','','v1'),('b','Physical primary','','v2'),('c','Unrelated','','v3');
   INSERT INTO roots(id,path,label) VALUES('root','generated-only','Generated root');
   INSERT INTO releases(id,label) VALUES('shared-edition','Shared edition'),('unrelated-edition','Unrelated edition');
   INSERT INTO work_releases(work_id,release_id) VALUES('a','shared-edition'),('b','shared-edition'),('c','unrelated-edition');
   INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('shared','root','shared','Shared files','archive','b'),('none','root','none','No edition','archive','a'),('empty','root','empty','No files','archive','a'),('other','root','other','Other files','archive','c');
   INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES('f1','root','f1','f1',3,'','dat','','present'),('f2','root','f2','f2',7,'','dat','','legacy-unknown'),('f3','root','f3','f3',2,'','dat','','missing'),('f4','root','f4','f4',99,'','dat','','present');
   INSERT INTO resource_files(resource_id,file_id) VALUES('shared','f1'),('shared','f2'),('none','f3'),('other','f4');
   INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES('shared','a','shared-edition','patch'),('shared','b','shared-edition','main'),('none','a',NULL,'main'),('empty','a',NULL,'extra'),('other','c','unrelated-edition','main');
   INSERT INTO work_preferences(work_id,preferred_release_id) VALUES('a','shared-edition');").unwrap();
 }
 #[test]fn scoped_assets_preserve_all_shared_bindings_primaries_and_unknown_state(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();scoped_fixture(&c);
  let value=work_assets(&c,"a").unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(value["work"],one(&c,"a").unwrap().unwrap());assert_eq!(value["work"]["resources"],2);assert_eq!(value["editions"].as_array().unwrap().len(),1);assert_eq!(value["editions"][0]["work_count"],2);assert_eq!(value["editions"][0]["work_id"],"a");assert!(value["editions"][0].get("work_ids").is_none());assert_eq!(crate::editions::one(&c,"shared-edition").unwrap().unwrap()["work_ids"],json!(["a","b"]));
  let rows=value["resources"].as_array().unwrap();assert_eq!(rows.len(),2);let shared=rows.iter().find(|r|r["id"]=="shared").unwrap();assert_eq!(shared["files"],2);assert_eq!(shared["bytes"],10);assert_eq!(shared["unknown"],1);assert_eq!(shared["bindings"].as_array().unwrap().len(),2);assert_eq!(shared["work_id"],"b");assert_eq!(shared["primary_work_title"],"Physical primary");assert_eq!(rows.iter().find(|r|r["id"]=="none").unwrap()["missing"],1);
  let legacy=crate::resource_catalog::rows(&c,None).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(legacy.len(),3);for row in rows{assert_eq!(row,legacy.iter().find(|r|r["id"]==row["id"]).unwrap());}assert_eq!(crate::editions::list(&c).unwrap().as_array().unwrap().len(),2);
  assert!(work_assets(&c,"absent").is_err());c.execute("UPDATE works SET merged_into='b' WHERE id='a'",[]).unwrap();assert!(work_assets(&c,"a").is_err());
 }
 #[tokio::test]async fn scoped_http_reads_require_auth_and_pin_lookup_revision(){
  let temp=tempfile::tempdir().unwrap();let app=crate::initialize(temp.path().join("state")).unwrap();scoped_fixture(&app.db.lock().unwrap());
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}/api",listener.local_addr().unwrap());let run=app.clone();let server=tokio::spawn(async move{axum::serve(listener,crate::router(run)).await.unwrap()});let client=reqwest::Client::new();
  for path in ["works/a/assets","works/lookup?ids=%5B%22v1%22%5D","releases/shared-edition"]{assert_eq!(client.get(format!("{base}/{path}")).send().await.unwrap().status(),reqwest::StatusCode::UNAUTHORIZED);let response=client.get(format!("{base}/{path}")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap();assert_eq!(response.headers()["cache-control"],"no-store");}
  let first:Value=client.get(format!("{base}/works/lookup")).query(&[("ids","[\"v1\"]")]).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(first["items"][0]["id"],"a");
  app.db.lock().unwrap().execute("UPDATE works SET favorite=1 WHERE id='a'",[]).unwrap();let response=client.get(format!("{base}/works/lookup")).query(&[("ids","[]".to_owned()),("revision",first["revision"].to_string()),("library_id",first["library_id"].as_str().unwrap().to_owned())]).bearer_auth(&app.token).send().await.unwrap();assert_eq!(response.status(),reqwest::StatusCode::BAD_REQUEST);
  let edition:Value=client.get(format!("{base}/releases/shared-edition")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(edition["work_ids"],json!(["a","b"]));
  let assets:Value=client.get(format!("{base}/works/a/assets")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(assets["work"]["favorite"],true);assert_eq!(assets["resources"].as_array().unwrap().len(),2);server.abort();let _=server.await;
 }

 #[tokio::test]async fn http_routes_enforce_auth_and_preserve_page_and_detail_contracts(){
  let temp=tempfile::tempdir().unwrap();let app=crate::initialize(temp.path().join("state")).unwrap();
  {let c=app.db.lock().unwrap();for i in 0..61{c.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?2,'原題')",params![format!("http-{i}"),format!("Work {i}")]).unwrap();}}
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}/api",listener.local_addr().unwrap());let server_app=app.clone();let server=tokio::spawn(async move{axum::serve(listener,crate::router(server_app)).await.unwrap()});let client=reqwest::Client::new();
  for path in ["works?paged=true","works/http-60"]{assert_eq!(client.get(format!("{base}/{path}")).send().await.unwrap().status(),reqwest::StatusCode::UNAUTHORIZED);}
  let first=client.get(format!("{base}/works?paged=true")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap();assert_eq!(first.headers()["cache-control"],"no-store");let first:Value=first.json().await.unwrap();assert_eq!(first["total"],61);assert_eq!(first["items"].as_array().unwrap().len(),60);
  let detail:Value=client.get(format!("{base}/works/http-60")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(detail,first["items"][0]);
  let cursor=first["next"].as_str().unwrap();let second:Value=client.get(format!("{base}/works")).query(&[("paged","true"),("before",cursor)]).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(second["items"].as_array().unwrap().len(),1);assert!(second["next"].is_null());assert_eq!(second["items"][0]["id"],"http-0");
  let legacy:Value=client.get(format!("{base}/works")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(legacy.as_array().unwrap().len(),61);
  app.db.lock().unwrap().execute("UPDATE works SET title='Updated' WHERE id='http-0'",[]).unwrap();
  assert_eq!(client.get(format!("{base}/works")).query(&[("paged","true"),("before",cursor)]).bearer_auth(&app.token).send().await.unwrap().status(),reqwest::StatusCode::BAD_REQUEST);
  for path in ["works/absent","works?before=bad","works?paged=true&before=bad"]{assert_eq!(client.get(format!("{base}/{path}")).bearer_auth(&app.token).send().await.unwrap().status(),reqwest::StatusCode::BAD_REQUEST);}
  server.abort();let _=server.await;
 }
 #[test]fn bounded_traversal_matches_single_work_projection_without_merged_duplicates(){
  let t=tempfile::tempdir().unwrap();let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=d.lock().unwrap();
  for i in 0..126{c.execute("INSERT INTO works(id,title,original_title,aliases,tags,notes) VALUES(?1,?2,'原題','[\"Alias\"]','[\"Drama\"]','Private note')",params![format!("w{i}"),format!("Title {i:03}")]).unwrap();}
  c.execute("UPDATE works SET merged_into='w0' WHERE id='w1'",[]).unwrap();
  let mut cursor=None;let mut got=vec![];let mut lengths=vec![];
  loop{let p=page(&c,cursor.as_deref()).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(p["total"],125);let items=p["items"].as_array().unwrap();lengths.push(items.len());for item in items{assert_eq!(&one(&c,item["id"].as_str().unwrap()).unwrap().unwrap(),item);got.push(item["id"].as_str().unwrap().to_owned());}cursor=p["next"].as_str().map(str::to_owned);if cursor.is_none(){break}}
  assert_eq!(lengths,[60,60,5]);assert_eq!(got.len(),125);assert_eq!(got.iter().collect::<std::collections::HashSet<_>>().len(),125);assert_eq!(got[0],"w125");assert_eq!(got.last().unwrap(),"w0");assert!(one(&c,"w1").unwrap().is_none());assert!(one(&c,"absent").unwrap().is_none());
  let legacy=all(&c).unwrap();assert_eq!(legacy.len(),125);assert_eq!(legacy[0]["title"],"Title 000");assert_eq!(legacy[0]["aliases"],json!(["Alias"]));assert_eq!(legacy[0]["notes"],"Private note");
 }
 #[test]fn cursor_rejects_changed_catalog_and_cross_library_reuse(){
  let t=tempfile::tempdir().unwrap();let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=d.lock().unwrap();
  for i in 0..61{c.execute("INSERT INTO works(id,title,original_title) VALUES(?1,'Title','')",[format!("w{i}")]).unwrap();}
  let p=page(&c,None).unwrap_or_else(|e|panic!("{}",e.0));let raw=p["next"].as_str().unwrap();
  let mut foreign:Value=serde_json::from_str(raw).unwrap();foreign["library"]=json!("another-library");assert!(page(&c,Some(&foreign.to_string())).is_err());
  for bad in ["{}","[]","not-json"]{assert!(page(&c,Some(bad)).is_err());}
  c.execute("UPDATE works SET title='Changed' WHERE id='w0'",[]).unwrap();assert!(page(&c,Some(raw)).unwrap_err().0.contains("Collection changed"));
  assert_eq!(page(&c,None).unwrap_or_else(|e|panic!("{}",e.0))["items"].as_array().unwrap().len(),60);
 }
 #[test]fn empty_catalog_has_no_cursor_or_invented_work(){
  let t=tempfile::tempdir().unwrap();let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=d.lock().unwrap();let p=page(&c,None).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(p["total"],0);assert_eq!(p["items"],json!([]));assert!(p["next"].is_null());
 }
}



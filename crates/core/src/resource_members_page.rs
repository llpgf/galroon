//! Bounded source members, pinned to collection identity and catalog revision.
use axum::{extract::{Path,Query,State},http::HeaderMap};
use rusqlite::{Connection,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{ApiError,App};

pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS member_catalog_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),revision INTEGER NOT NULL); INSERT OR IGNORE INTO member_catalog_state VALUES(1,1);")?;
 for table in ["resources","resource_files","files"]{for action in ["INSERT","UPDATE","DELETE"]{
  c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS member_dirty_{table}_{action} AFTER {action} ON {table} BEGIN UPDATE member_catalog_state SET revision=revision+1 WHERE singleton=1; END;"))?;
 }}Ok(())
}

#[derive(Default,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Options {#[serde(default)]query:String,before:Option<String>,catalog_revision:Option<i64>,library_id:Option<String>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
struct Cursor {version:u8,library:String,catalog_revision:i64,resource:String,query:String,offset:i64}

pub async fn read(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<Options>)->crate::Result<Value>{
 crate::auth(&a,&h)?;crate::work_catalog::read_snapshot(a,move|c|page(c,&id,&q)).await
}
fn page(c:&Connection,id:&str,q:&Options)->Result<Value,ApiError>{
 if id.len()>512||q.query.len()>512{return Err(ApiError("Invalid resource member query".into()));}
 let library=crate::context::library(c)?.id;
 let catalog_revision:i64=c.query_row("SELECT revision FROM member_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
 if q.catalog_revision.is_some()!=q.library_id.is_some(){return Err(ApiError("Both library and catalog revision are required".into()));}
 if q.catalog_revision.is_some_and(|r|r!=catalog_revision)||q.library_id.as_ref().is_some_and(|l|l!=&library){return Err(ApiError("Resource files changed. Restart from the first page.".into()));}
 let source=c.query_row("SELECT title,revision,root_id FROM resources WHERE id=?1",[id],|r|Ok(json!({"id":id,"title":r.get::<_,String>(0)?,"revision":r.get::<_,i64>(1)?,"root_id":r.get::<_,String>(2)?})))?;
 let offset=if let Some(raw)=&q.before{
  if raw.len()>4096{return Err(ApiError("Invalid member cursor".into()));}
  let cursor:Cursor=serde_json::from_str(raw).map_err(|_|ApiError("Invalid member cursor".into()))?;
  if cursor.version!=1||cursor.library!=library||cursor.catalog_revision!=catalog_revision||cursor.resource!=id||cursor.query!=q.query||cursor.offset<0{return Err(ApiError("Resource files changed. Restart from the first page.".into()));}
  cursor.offset
 }else{0};
 c.create_scalar_function("member_search_lower",1,rusqlite::functions::FunctionFlags::SQLITE_UTF8|rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,|ctx|Ok(ctx.get::<String>(0)?.to_lowercase()))?;
 let locale:icu_locale_core::Locale="en".parse().unwrap();let mut prefs:icu_collator::CollatorPreferences=locale.into();prefs.numeric_ordering=Some(icu_collator::preferences::CollationNumericOrdering::True);
 let collator=icu_collator::Collator::try_new(prefs,icu_collator::options::CollatorOptions::default()).map_err(|e|ApiError(e.to_string()))?;
 c.create_collation("member_path_natural",move|a,b|collator.compare(a,b))?;
 let search=q.query.to_lowercase();
 let filter=" FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 AND (?2='' OR instr(member_search_lower(f.relative_path),?2)>0)";
 let total:i64=c.query_row(&format!("SELECT count(*){filter}"),params![id,search],|r|r.get(0))?;
 let file_count:i64=c.query_row("SELECT count(*) FROM resource_files WHERE resource_id=?1",[id],|r|r.get(0))?;
 let mut statement=c.prepare(&format!("SELECT f.id,f.relative_path,f.size,f.availability{filter} ORDER BY f.relative_path COLLATE member_path_natural,f.id LIMIT 61 OFFSET ?3"))?;
 let mut items=statement.query_map(params![id,search,offset],|r|Ok(json!({"id":r.get::<_,String>(0)?,"relative":r.get::<_,String>(1)?,"size":r.get::<_,i64>(2)?,"availability":r.get::<_,String>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let more=items.len()>60;items.truncate(60);
 let next=if more{Some(serde_json::to_string(&Cursor{version:1,library:library.clone(),catalog_revision,resource:id.to_owned(),query:q.query.clone(),offset:offset+60}).map_err(|e|ApiError(e.to_string()))?)}else{None};
 Ok(json!({"source":source,"items":items,"total":total,"file_count":file_count,"offset":offset,"next":next,"library_id":library,"catalog_revision":catalog_revision}))
}

#[cfg(test)]mod tests{
 use super::*;
 fn seed(c:&Connection){c.execute_batch("INSERT INTO roots(id,path,label) VALUES('r','generated','Generated');INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('a','r','Game','A','directory'),('b','r','Other','B','directory');WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<124) INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) SELECT printf('f%03d',i),'r',printf('p%03d',i),printf('Game/%03d.bin',i),i,'','bin','' FROM n;INSERT INTO resource_files SELECT 'a',id FROM files;").unwrap();}
 #[test]fn pages_are_bounded_complete_and_reject_changed_scope(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();seed(&c);
  let mut q=Options::default();let mut found=vec![];
  loop{let result=page(&c,"a",&q).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(result["file_count"],125);let items=result["items"].as_array().unwrap();assert!(items.len()<=60);for f in items{assert!(f.get("path").is_none());found.push(f["id"].as_str().unwrap().to_owned());}q.before=result["next"].as_str().map(str::to_owned);if q.before.is_none(){break;}}
  assert_eq!(found,(0..125).map(|i|format!("f{i:03}")).collect::<Vec<_>>());
  let first=page(&c,"a",&Options::default()).ok().unwrap();let raw=first["next"].as_str().unwrap().to_owned();
  assert!(page(&c,"b",&Options{before:Some(raw.clone()),..Default::default()}).is_err());assert!(page(&c,"a",&Options{query:"x".into(),before:Some(raw.clone()),..Default::default()}).is_err());
  for field in ["library","offset"]{let mut cursor:Value=serde_json::from_str(&raw).unwrap();cursor[field]=if field=="offset"{json!(-1)}else{json!("other")};assert!(page(&c,"a",&Options{before:Some(cursor.to_string()),..Default::default()}).is_err());}
  for invalid in ["invalid".to_owned(),"x".repeat(4097)]{assert!(page(&c,"a",&Options{before:Some(invalid),..Default::default()}).is_err());}
  assert!(page(&c,"a",&Options{query:"x".repeat(513),..Default::default()}).is_err());assert!(page(&c,"absent",&Options::default()).is_err());
  c.execute("UPDATE files SET relative_path='資料/ÉCOLE.bin',availability='missing' WHERE id='f124'",[]).unwrap();assert!(page(&c,"a",&Options{before:Some(raw),..Default::default()}).is_err());
  let result=page(&c,"a",&Options{query:"école".into(),..Default::default()}).ok().unwrap();assert_eq!(result["total"],1);assert_eq!(result["items"][0]["id"],"f124");assert_eq!(result["items"][0]["availability"],"missing");assert_eq!(result["file_count"],125);
  assert!(page(&c,"a",&Options{query:"école".into(),catalog_revision:first["catalog_revision"].as_i64(),library_id:first["library_id"].as_str().map(str::to_owned),..Default::default()}).is_err());
  assert!(page(&c,"a",&Options{library_id:Some("other".into()),..Default::default()}).is_err());
  assert!(page(&c,"a",&Options{catalog_revision:result["catalog_revision"].as_i64(),library_id:Some("other".into()),..Default::default()}).is_err());
  assert_eq!(page(&c,"b",&Options::default()).ok().unwrap()["total"],0);
  c.execute("UPDATE files SET relative_path='Numbers/10.bin' WHERE id='f000'",[]).unwrap();c.execute("UPDATE files SET relative_path='Numbers/2.bin' WHERE id='f001'",[]).unwrap();
  let numeric=page(&c,"a",&Options{query:"Numbers/".into(),..Default::default()}).ok().unwrap();assert_eq!(numeric["items"][0]["id"],"f001");assert_eq!(numeric["items"][1]["id"],"f000");

 }
 #[test]fn root_observation_does_not_invalidate_members(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();seed(&c);
  let first=page(&c,"a",&Options::default()).ok().unwrap();let next=Options{before:first["next"].as_str().map(str::to_owned),..Default::default()};
  let before:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();
  crate::smart_updates::observe_roots(&c,&std::collections::BTreeMap::from([("r".to_string(),false)])).unwrap();
  assert_eq!(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),before+1);
  assert_eq!(page(&c,"a",&next).ok().unwrap()["items"][0]["id"],"f060");
  let mut legacy:Value=serde_json::from_str(next.before.as_ref().unwrap()).unwrap();legacy.as_object_mut().unwrap().remove("version");assert!(page(&c,"a",&Options{before:Some(legacy.to_string()),..Default::default()}).is_err());
  c.execute("UPDATE files SET size=size+1 WHERE id='f000'",[]).unwrap();assert!(page(&c,"a",&next).is_err());
 }
 #[test]fn migration42_preserves_members_and_reopen(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();{let c=db.lock().unwrap();seed(&c);for table in ["resources","resource_files","files"]{for action in ["INSERT","UPDATE","DELETE"]{c.execute_batch(&format!("DROP TRIGGER member_dirty_{table}_{action};")).unwrap();}}c.execute_batch("DROP TABLE member_catalog_state; PRAGMA user_version=41;").unwrap();}drop(db);
  let db=crate::db::open(&path).unwrap();let first={let c=db.lock().unwrap();assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),42);assert_eq!(c.query_row("SELECT count(*) FROM resource_files",[],|r|r.get::<_,i64>(0)).unwrap(),125);page(&c,"a",&Options::default()).ok().unwrap()};drop(db);
  let db=crate::db::open(&path).unwrap();assert_eq!(page(&db.lock().unwrap(),"a",&Options::default()).ok().unwrap(),first);assert_eq!(std::fs::read_dir(t.path().join("migration-backups")).unwrap().count(),1);
 }
 #[tokio::test]async fn http_requires_auth_and_serves_readonly_snapshot(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();seed(&app.db.lock().unwrap());crate::access::setup(&app.db,"Generated member page passphrase",false).unwrap();
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let run=app.clone();let server=tokio::spawn(async move{axum::serve(listener,crate::router(run)).await.unwrap()});
  let client=reqwest::Client::builder().no_proxy().build().unwrap();let url=format!("{base}/api/resources/a/members/page");assert_eq!(client.get(&url).send().await.unwrap().status(),401);
  let response=client.get(&url).bearer_auth(&app.token).send().await.unwrap();assert_eq!(response.status(),200);assert_eq!(response.headers()["cache-control"],"no-store");let first:Value=response.json().await.unwrap();assert_eq!(first["items"].as_array().unwrap().len(),60);
  let login=client.post(format!("{base}/api/access/login")).header("Origin",&base).json(&json!({"password":"Generated member page passphrase"})).send().await.unwrap();let cookie=login.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
  let web:Value=client.get(&url).header("Cookie",&cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(web,first);
  app.db.lock().unwrap().execute("DELETE FROM resource_files WHERE file_id='f124'",[]).unwrap();assert_eq!(client.get(&url).query(&[("before",first["next"].as_str().unwrap())]).header("Cookie",&cookie).send().await.unwrap().status(),400);
  assert_eq!(client.post(format!("{base}/api/resources/a/regroup")).header("Cookie",&cookie).json(&json!({})).send().await.unwrap().status(),403);server.abort();let _=server.await;
 }
}

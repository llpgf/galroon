//! Reviewed catalog-only file membership. Resource IDs are independent of directory boundaries.
use crate::{db::{Db,id,now},editions,roots};
use rusqlite::{Connection,params};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{collections::{BTreeSet,BTreeMap},path::{Path,PathBuf}};
type R<T>=Result<T,String>;
fn sql<T>(r:rusqlite::Result<T>)->R<T>{r.map_err(|e|e.to_string())}
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 let has:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('resources') WHERE name='manual_group')",[],|r|r.get(0))?;
 if !has{
  // db::open disables FK enforcement only around its atomic schema migration and checks all FKs before commit.
  c.execute_batch("CREATE TABLE resources_v8(id TEXT PRIMARY KEY,root_id TEXT NOT NULL REFERENCES roots(id),relative_path TEXT NOT NULL,title TEXT NOT NULL,kind TEXT NOT NULL,work_id TEXT,release_label TEXT NOT NULL DEFAULT 'Unclassified edition',revision INTEGER NOT NULL DEFAULT 1,manual_group INTEGER NOT NULL DEFAULT 0 CHECK(manual_group IN (0,1)));
  INSERT INTO resources_v8(id,root_id,relative_path,title,kind,work_id,release_label,revision) SELECT id,root_id,relative_path,title,kind,work_id,release_label,revision FROM resources;
  DROP TABLE resources; ALTER TABLE resources_v8 RENAME TO resources;
  CREATE UNIQUE INDEX resource_auto_path ON resources(root_id,relative_path) WHERE manual_group=0;")?;
 }
 c.execute_batch("CREATE TABLE IF NOT EXISTS membership_history(id TEXT PRIMARY KEY,source TEXT NOT NULL,target TEXT NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,created INTEGER NOT NULL);")
}
#[derive(Clone,Serialize,Deserialize)]pub struct Selection{pub revision:i64,pub file_ids:Vec<String>,pub target_id:Option<String>,pub target_revision:Option<i64>,#[serde(default)]pub title:String}
#[derive(Deserialize)]pub struct Confirmation{#[serde(flatten)]pub selection:Selection,pub digest:String}
fn visit_files(c:&Connection,rid:&str,mut visit:impl FnMut(Value)->R<()>)->R<()>{
 let mut statement=sql(c.prepare("SELECT f.id,f.root_id,f.relative_path,f.path,f.size,f.mtime,f.availability FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 ORDER BY f.id"))?;
 let mut rows=sql(statement.query([rid]))?;
 while let Some(row)=sql(rows.next())?{
  let file=sql((||->rusqlite::Result<Value>{Ok(json!({"id":row.get::<_,String>(0)?,"root_id":row.get::<_,String>(1)?,"relative":row.get::<_,String>(2)?,"path":row.get::<_,String>(3)?,"size":row.get::<_,i64>(4)?,"mtime":row.get::<_,String>(5)?,"availability":row.get::<_,String>(6)?}))})())?;
  visit(file)?;
 }
 Ok(())
}
fn files(c:&Connection,rid:&str)->R<Vec<Value>>{let mut result=Vec::new();visit_files(c,rid,|file|{result.push(file);Ok(())})?;Ok(result)}
fn resource_metadata(c:&Connection,rid:&str)->R<Value>{let mut v=sql(c.query_row("SELECT root_id,title,revision,relative_path,work_id,release_label,manual_group FROM resources WHERE id=?1",[rid],|r|Ok(json!({"id":rid,"root_id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?,"relative":r.get::<_,String>(3)?,"work_id":r.get::<_,Option<String>>(4)?,"release":r.get::<_,String>(5)?,"manual":r.get::<_,bool>(6)?}))))?;v["bindings"]=json!(editions::bindings(c,rid)?);for binding in v["bindings"].as_array_mut().unwrap(){use rusqlite::OptionalExtension;let wid=binding["work_id"].as_str().unwrap();binding["work_title"]=json!(sql(c.query_row("SELECT title FROM works WHERE id=?1",[wid],|r|r.get::<_,String>(0)).optional())?);binding["edition_label"]=if let Some(eid)=binding["release_id"].as_str(){json!(sql(c.query_row("SELECT label FROM releases WHERE id=?1",[eid],|r|r.get::<_,String>(0)).optional())?)}else{Value::Null};}Ok(v)}
fn resource(c:&Connection,rid:&str)->R<Value>{let mut v=resource_metadata(c,rid)?;v["files"]=Value::Array(files(c,rid)?);Ok(v)}
pub fn members(db:&Db,rid:&str)->R<Value>{resource(&*db.lock().map_err(|e|e.to_string())?,rid)}
fn ids(c:&Connection,query:&str,rid:&str,target:&str)->R<Vec<String>>{let mut s=sql(c.prepare(query))?;let rows=sql(sql(s.query_map(params![rid,target],|r|r.get(0)))?.collect::<rusqlite::Result<Vec<_>>>())?;Ok(rows)}
// Serialize directly into SHA-256; avoid retaining a second full JSON byte buffer.
fn snapshot_digest(value:&impl Serialize)->R<String>{
 struct HashWriter(Sha256);
 impl std::io::Write for HashWriter{
  fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{self.0.update(bytes);Ok(bytes.len())}
  fn flush(&mut self)->std::io::Result<()>{Ok(())}
 }
 let mut writer=HashWriter(Sha256::new());
 serde_json::to_writer(&mut writer,value).map_err(|e|e.to_string())?;
 Ok(format!("{:x}",writer.0.finalize()))
}
// Internal building block for the bounded preview path. `parts` contains the
// already validated snapshot metadata, with file arrays and selected omitted.
// The caller must hold one database read transaction throughout validation/hash.
fn stream_snapshot_digest(c:&Connection,rid:&str,selection:&Selection,parts:&Value)->R<String>{
 use serde::ser::{SerializeMap,SerializeSeq,Error};
 struct FileRows<'a>{c:&'a Connection,rid:&'a str,selected:Option<&'a BTreeSet<&'a str>>}
 impl Serialize for FileRows<'_>{fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
  let mut sequence=serializer.serialize_seq(None)?;
  visit_files(self.c,self.rid,|file|{if self.selected.is_none_or(|ids|ids.contains(file["id"].as_str().unwrap())){sequence.serialize_element(&file).map_err(|e|e.to_string())?;}Ok(())}).map_err(S::Error::custom)?;
  sequence.end()
 }}
 struct ResourceRows<'a>{c:&'a Connection,metadata:&'a Value}
 impl Serialize for ResourceRows<'_>{fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
  let metadata=self.metadata.as_object().ok_or_else(||S::Error::custom("Missing resource metadata"))?;
  if metadata.contains_key("files"){return Err(S::Error::custom("Stream metadata must omit files"));}
  let rid=metadata.get("id").and_then(Value::as_str).ok_or_else(||S::Error::custom("Missing resource identity"))?;
  let mut keys=metadata.keys().map(String::as_str).collect::<BTreeSet<_>>();keys.insert("files");
  let mut map=serializer.serialize_map(Some(keys.len()))?;
  for key in keys{if key=="files"{map.serialize_entry(key,&FileRows{c:self.c,rid,selected:None})?;}else{map.serialize_entry(key,&metadata[key])?;}}
  map.end()
 }}
 struct SnapshotRows<'a>{c:&'a Connection,rid:&'a str,parts:&'a Value,selected:BTreeSet<&'a str>}
 impl Serialize for SnapshotRows<'_>{fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
  let parts=self.parts.as_object().ok_or_else(||S::Error::custom("Missing snapshot metadata"))?;
  if parts.contains_key("digest")||parts.contains_key("selected"){return Err(S::Error::custom("Stream metadata must omit digest and selected"));}
  let mut keys=parts.keys().map(String::as_str).collect::<BTreeSet<_>>();keys.insert("selected");
  let mut map=serializer.serialize_map(Some(keys.len()))?;
  for key in keys{match key{
   "selected"=>map.serialize_entry(key,&FileRows{c:self.c,rid:self.rid,selected:Some(&self.selected)})?,
   "source"=>map.serialize_entry(key,&ResourceRows{c:self.c,metadata:&parts[key]})?,
   "target" if !parts[key].is_null()=>map.serialize_entry(key,&ResourceRows{c:self.c,metadata:&parts[key]})?,
   _=>map.serialize_entry(key,&parts[key])?,
  }}map.end()
 }}
 snapshot_digest(&SnapshotRows{c,rid,parts,selected:selection.file_ids.iter().map(String::as_str).collect()})
}
fn snapshot_plan_guards(c:&Connection,rid:&str,v:&Selection)->R<(Vec<String>,Vec<String>)>{
 let tid=v.target_id.as_deref().unwrap_or("");
 let blocked=ids(c,"SELECT p.id FROM plans p WHERE p.state IN ('approved','executing','partial') AND (EXISTS(SELECT 1 FROM plan_locations l WHERE l.plan_id=p.id AND l.resource_id IN (?1,?2)) OR EXISTS(SELECT 1 FROM json_each(p.items) i JOIN resource_files rf ON rf.file_id=json_extract(i.value,'$.file_id') WHERE rf.resource_id IN (?1,?2)))",rid,tid)?;
 if !blocked.is_empty(){return Err("Finish the approved or partially executed file plans for these resources before regrouping".into());}
 let retired_plans=ids(c,"SELECT p.id FROM plans p WHERE p.state='ready' AND (EXISTS(SELECT 1 FROM plan_locations l WHERE l.plan_id=p.id AND l.resource_id IN (?1,?2)) OR EXISTS(SELECT 1 FROM json_each(p.items) i JOIN resource_files rf ON rf.file_id=json_extract(i.value,'$.file_id') WHERE rf.resource_id IN (?1,?2))) ORDER BY p.id",rid,tid)?;
 let retired_jobs=ids(c,"SELECT id FROM jobs WHERE kind='acquire' AND (json_extract(spec,'$.resource_id') IN (?1,?2) OR EXISTS(SELECT 1 FROM json_each(jobs.spec,'$.files') i JOIN resource_files rf ON rf.file_id=json_extract(i.value,'$.id') WHERE rf.resource_id IN (?1,?2))) AND state NOT IN ('completed','superseded') ORDER BY id",rid,tid)?;
 Ok((retired_plans,retired_jobs))
}
pub(crate) fn snapshot(c:&Connection,rid:&str,v:&Selection)->R<Value>{
 roots::require_idle(c)?;let source=resource(c,rid)?;if source["revision"]!=v.revision{return Err("Source resource changed. Reload its files.".into());}
 let distinct:BTreeSet<_>=v.file_ids.iter().collect();if distinct.is_empty()||distinct.len()!=v.file_ids.len(){return Err("Choose distinct files to regroup".into());}
 let target=if let Some(t)=&v.target_id{if t==rid{return Err("Choose another resource".into());}let t=resource(c,t)?;if Some(t["revision"].as_i64().unwrap())!=v.target_revision{return Err("Destination resource changed. Reload the preview.".into());}if t["root_id"]!=source["root_id"]{return Err("Resources must belong to the same source. Moving files is a separate reviewed operation.".into());}Some(t)}else{if v.title.trim().is_empty()||v.title.chars().count()>200{return Err("Enter a resource title of 1 to 200 characters".into());}None};
 // Read ownership once for the requested IDs, rather than scanning membership for every selected file.
 let mut ownership=sql(c.prepare("SELECT file_id,count(*) FROM resource_files WHERE file_id IN (SELECT value FROM json_each(?1)) GROUP BY file_id"))?;
 let counts:BTreeMap<String,i64>=sql(sql(ownership.query_map([json!(v.file_ids).to_string()],|r|Ok((r.get(0)?,r.get(1)?))))?.collect())?;
 let all=source["files"].as_array().unwrap();let mut selected=Vec::with_capacity(distinct.len());for fid in &distinct{let index=all.binary_search_by(|f|f["id"].as_str().unwrap().cmp(fid.as_str())).map_err(|_|"File membership changed. Reload the preview.")?;let f=&all[index];if counts.get(fid.as_str())!=Some(&1){return Err("A file has ambiguous resource ownership".into());}selected.push(f.clone());}
 let combined=target.as_ref().into_iter().flat_map(|t|t["files"].as_array().unwrap().iter()).chain(selected.iter());
 if combined.clone().any(|f|f["root_id"]!=source["root_id"]){return Err("Finish restoring or moving these resource files before regrouping".into());}
 let mut base=PathBuf::from(combined.clone().next().unwrap()["relative"].as_str().unwrap());base.pop();for f in combined{let p=Path::new(f["relative"].as_str().unwrap());if p.is_absolute()||p.components().any(|x|!matches!(x,std::path::Component::Normal(_)|std::path::Component::CurDir)){return Err("File has an unsafe source-relative path".into());}while !p.starts_with(&base){if !base.pop(){return Err("Files do not share a source folder".into());}}}
 let (retired_plans,retired_jobs)=snapshot_plan_guards(c,rid,v)?;
 let mut result=json!({"target_title":target.as_ref().map(|t|t["title"].as_str().unwrap()).unwrap_or(v.title.trim()),"target_relative":base.to_string_lossy()});
 // Move owned JSON trees into the snapshot instead of serializing borrowed Values again.
 let object=result.as_object_mut().unwrap();
 object.insert("source".into(),source);
 object.insert("target".into(),target.unwrap_or(Value::Null));
 object.insert("selected".into(),Value::Array(selected));
 object.insert("retired_plans".into(),Value::Array(retired_plans.into_iter().map(Value::String).collect()));
 object.insert("retired_jobs".into(),Value::Array(retired_jobs.into_iter().map(Value::String).collect()));
 result["digest"]=json!(snapshot_digest(&result)?);Ok(result)
}
// The HTTP caller holds one read transaction across validation, digest and page query.
// Retain selected IDs and metadata only; source/target file rows are consumed individually.
fn streamed_preview(c:&Connection,rid:&str,v:&Selection)->R<(Value,usize,i64)>{
 roots::require_idle(c)?;
 let source=resource_metadata(c,rid)?;
 if source["revision"]!=v.revision{return Err("Source resource changed. Reload its files.".into());}
 let distinct:BTreeSet<&str>=v.file_ids.iter().map(String::as_str).collect();
 if distinct.is_empty()||distinct.len()!=v.file_ids.len(){return Err("Choose distinct files to regroup".into());}
 let target=if let Some(tid)=&v.target_id{
  if tid==rid{return Err("Choose another resource".into());}
  let t=resource_metadata(c,tid)?;
  if Some(t["revision"].as_i64().unwrap())!=v.target_revision{return Err("Destination resource changed. Reload the preview.".into());}
  if t["root_id"]!=source["root_id"]{return Err("Resources must belong to the same source. Moving files is a separate reviewed operation.".into());}Some(t)
 }else{if v.title.trim().is_empty()||v.title.chars().count()>200{return Err("Enter a resource title of 1 to 200 characters".into());}None};
 let encoded=serde_json::to_string(&v.file_ids).map_err(|e|e.to_string())?;
 let ambiguous:bool=sql(c.query_row("SELECT EXISTS(SELECT file_id FROM resource_files WHERE file_id IN (SELECT value FROM json_each(?1)) GROUP BY file_id HAVING count(*)<>1)",[&encoded],|r|r.get(0)))?;
 if ambiguous{return Err("A file has ambiguous resource ownership".into());}
 let mut base:Option<PathBuf>=None;
 let mut validate=|file:&Value|->R<()>{
  if file["root_id"]!=source["root_id"]{return Err("Finish restoring or moving these resource files before regrouping".into());}
  let path=Path::new(file["relative"].as_str().unwrap());
  if path.is_absolute()||path.components().any(|x|!matches!(x,std::path::Component::Normal(_)|std::path::Component::CurDir)){return Err("File has an unsafe source-relative path".into());}
  let parent=base.get_or_insert_with(||{let mut p=path.to_path_buf();p.pop();p});
  while !path.starts_with(&*parent){if !parent.pop(){return Err("Files do not share a source folder".into());}}Ok(())
 };
 let mut target_count=0usize;
 if let Some(t)=&target{visit_files(c,t["id"].as_str().unwrap(),|f|{target_count+=1;validate(&f)})?;}
 let(mut source_count,mut found,mut bytes)=(0usize,0usize,0i64);
 visit_files(c,rid,|f|{source_count+=1;if distinct.contains(f["id"].as_str().unwrap()){
  found+=1;validate(&f)?;bytes=bytes.checked_add(f["size"].as_i64().unwrap()).ok_or("Selected file size exceeds supported total")?;
 }Ok(())})?;
 if found!=distinct.len(){return Err("File membership changed. Reload the preview.".into());}
 let (retired_plans,retired_jobs)=snapshot_plan_guards(c,rid,v)?;
 let mut parts=json!({"target_title":target.as_ref().map(|t|t["title"].as_str().unwrap()).unwrap_or(v.title.trim()),"target_relative":base.unwrap().to_string_lossy(),"retired_plans":retired_plans,"retired_jobs":retired_jobs});
 let object=parts.as_object_mut().unwrap();object.insert("source".into(),source);object.insert("target".into(),target.unwrap_or(Value::Null));
 let digest=stream_snapshot_digest(c,rid,v,&parts)?;parts["digest"]=json!(digest);
 parts["source"]["file_count"]=json!(source_count);if !parts["target"].is_null(){parts["target"]["file_count"]=json!(target_count);}
 Ok((parts,found,bytes))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct PreviewPageRequest{pub selection:Selection,#[serde(default)]pub offset:usize,pub digest:Option<String>,pub library_id:Option<String>}
pub(crate) fn preview_page(c:&Connection,rid:&str,q:&PreviewPageRequest)->R<Value>{
 if q.digest.is_some()!=q.library_id.is_some()||q.offset>0&&q.digest.is_none(){return Err("Preview pages require the original digest and library identity".into());}
 let library=crate::context::library(c).map_err(|e|e.to_string())?.id;
 if q.library_id.as_ref().is_some_and(|id|id!=&library){return Err("Preview library changed. Review again.".into());}
 let (mut full,total,bytes)=streamed_preview(c,rid,&q.selection)?;
 if q.digest.as_ref().is_some_and(|d|Some(d.as_str())!=full["digest"].as_str()){return Err("The grouping preview changed. Review it again.".into());}
 if q.offset>=total||q.offset%60!=0{return Err("Invalid preview page offset".into());}
 let locale:icu_locale_core::Locale="en".parse().unwrap();let mut prefs:icu_collator::CollatorPreferences=locale.into();prefs.numeric_ordering=Some(icu_collator::preferences::CollationNumericOrdering::True);
 let collator=icu_collator::Collator::try_new(prefs,icu_collator::options::CollatorOptions::default()).map_err(|e|e.to_string())?;
 sql(c.create_collation("preview_path_natural",move|a,b|collator.compare(a,b)))?;
 let selected_ids=serde_json::to_string(&q.selection.file_ids).map_err(|e|e.to_string())?;
 let mut statement=sql(c.prepare("SELECT f.id,f.relative_path,f.size,f.availability FROM resource_files rf JOIN files f ON f.id=rf.file_id WHERE rf.resource_id=?1 AND f.id IN (SELECT value FROM json_each(?2)) ORDER BY f.relative_path COLLATE preview_path_natural,f.id LIMIT 60 OFFSET ?3"))?;
 let items=sql(sql(statement.query_map(params![rid,selected_ids,q.offset as i64],|r|Ok(json!({"id":r.get::<_,String>(0)?,"relative":r.get::<_,String>(1)?,"size":r.get::<_,i64>(2)?,"availability":r.get::<_,String>(3)?}))))?.collect::<rusqlite::Result<Vec<_>>>())?;
 let object=full.as_object_mut().unwrap();object.remove("selected");
 for name in ["retired_plans","retired_jobs"]{let count=object.remove(name).unwrap().as_array().unwrap().len();object.insert(format!("{name}_count"),json!(count));}
 object.insert("items".into(),Value::Array(items));object.insert("selected_count".into(),json!(total));object.insert("selected_bytes".into(),json!(bytes));object.insert("offset".into(),json!(q.offset));object.insert("next_offset".into(),json!(if q.offset+60<total{Some(q.offset+60)}else{None}));object.insert("library_id".into(),json!(library));Ok(full)
}
pub fn preview(db:&Db,rid:&str,v:&Selection)->R<Value>{snapshot(&*db.lock().map_err(|e|e.to_string())?,rid,v)}
pub fn apply(db:&Db,rid:&str,v:&Selection,digest:&str)->R<Value>{
 let mut c=db.lock().map_err(|e|e.to_string())?;let tx=sql(c.transaction())?;let before=snapshot(&tx,rid,v)?;if before["digest"].as_str()!=Some(digest){return Err("The grouping preview changed. Review it again.".into());}
 let tid=v.target_id.clone().unwrap_or_else(id);
 if v.target_id.is_none(){sql(tx.execute("INSERT INTO resources(id,root_id,relative_path,title,kind,work_id,release_label,manual_group) SELECT ?2,root_id,?3,?4,'directory',work_id,release_label,1 FROM resources WHERE id=?1",params![rid,tid,before["target_relative"].as_str(),v.title.trim()]))?;sql(tx.execute("INSERT INTO resource_bindings(resource_id,work_id,release_id,role) SELECT ?2,work_id,release_id,role FROM resource_bindings WHERE resource_id=?1",params![rid,tid]))?;}
 for fid in &v.file_ids{sql(tx.execute("UPDATE resource_files SET resource_id=?2 WHERE resource_id=?1 AND file_id=?3",params![rid,tid,fid]))?;}
 sql(tx.execute("UPDATE resources SET revision=revision+1 WHERE id=?1",[rid]))?;
 sql(tx.execute("UPDATE resources SET revision=revision+1,manual_group=1,relative_path=?2,kind='directory' WHERE id=?1",params![tid,before["target_relative"].as_str()]))?;
 sql(tx.execute("UPDATE works SET revision=revision+1 WHERE id IN (SELECT work_id FROM resource_bindings WHERE resource_id IN (?1,?2))",params![rid,tid]))?;
 for p in before["retired_plans"].as_array().unwrap(){sql(tx.execute("UPDATE plans SET state='superseded',approved=NULL WHERE id=?1",[p.as_str()]))?;}
 for j in before["retired_jobs"].as_array().unwrap(){sql(tx.execute("UPDATE jobs SET state='superseded',message='Resource grouping changed. Start a new task; previous staging is retained.',updated=?2 WHERE id=?1",params![j.as_str(),now()]))?;}
 let after=Value::Object(serde_json::Map::from_iter([("source".into(),resource(&tx,rid)?),("target".into(),resource(&tx,&tid)?)]));let hid=id();sql(tx.execute("INSERT INTO membership_history(id,source,target,before_json,after_json,created) VALUES(?1,?2,?3,?4,?5,?6)",params![hid,rid,tid,before.to_string(),after.to_string(),now()]))?;sql(tx.commit())?;Ok(json!({"id":tid,"history_id":hid}))
}
#[cfg(test)]mod tests{
 use super::*;use crate::{db,scan,plans};use std::fs;
 fn fixture()->(tempfile::TempDir,Db,String){let t=tempfile::tempdir().unwrap();let root=t.path().join("source");fs::create_dir_all(root.join("Game/extras")).unwrap();for f in ["Game/a.bin","Game/b.iso","Game/extras/readme.txt"]{fs::write(root.join(f),f.as_bytes()).unwrap();}let db=db::open(&t.path().join("state/library.sqlite")).unwrap();db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'Fixture')",[root.to_str().unwrap()]).unwrap();rescan(&db);let rid:String=db.lock().unwrap().query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();{let c=db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,notes) VALUES('w','Game','Original','Keep notes')",[]).unwrap();c.execute("UPDATE resources SET work_id='w'",[]).unwrap();c.execute("INSERT INTO resource_bindings(resource_id,work_id,role) VALUES(?1,'w','main')",[&rid]).unwrap();}(t,db,rid)}
 fn rescan(db:&Db){let j=scan::create(db,scan::ScanSpec{root_id:"r".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(db.clone(),j.clone());assert_eq!(db.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[j],|r|r.get::<_,String>(0)).unwrap(),"completed");}
 fn select(db:&Db,rid:&str,suffix:&str)->Selection{let r=members(db,rid).unwrap();let f=r["files"].as_array().unwrap().iter().find(|f|f["relative"].as_str().unwrap().ends_with(suffix)).unwrap();Selection{revision:r["revision"].as_i64().unwrap(),file_ids:vec![f["id"].as_str().unwrap().into()],target_id:None,target_revision:None,title:"Disc copy".into()}}
 fn commit(db:&Db,rid:&str,v:&Selection)->String{let p=preview(db,rid,v).unwrap();apply(db,rid,v,p["digest"].as_str().unwrap()).unwrap()["id"].as_str().unwrap().into()}
 #[tokio::test(flavor="multi_thread",worker_threads=2)]async fn detached_preview_matches_digest_without_main_database_lock(){
  let(t,db,rid)=fixture();let selection=select(&db,&rid,"b.iso");let expected=preview(&db,&rid,&selection).unwrap();drop(db);
  let app=crate::initialize(t.path().join("state")).unwrap();
  let mut headers=axum::http::HeaderMap::new();headers.insert("authorization",format!("Bearer {}",app.token).parse().unwrap());
  let held=app.db.lock().unwrap();
  let result=tokio::time::timeout(std::time::Duration::from_secs(2),crate::preview_regroup(axum::extract::State(app.clone()),headers,axum::extract::Path(rid.clone()),axum::Json(selection.clone()))).await;
  drop(held);
  let actual=result.expect("Preview must not wait for the main database lock").unwrap_or_else(|e|panic!("{}",e.0)).0;
  assert_eq!(actual,expected);
  app.db.lock().unwrap().execute("UPDATE files SET size=size+1 WHERE id=?1",[&selection.file_ids[0]]).unwrap();
  assert!(apply(&app.db,&rid,&selection,actual["digest"].as_str().unwrap()).unwrap_err().contains("preview changed"));
  assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM membership_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn streamed_digest_matches_legacy_bytes_for_large_escaped_snapshots(){
  let(_t,db,rid)=fixture();let v=select(&db,&rid,"b.iso");
  let mut value=preview(&db,&rid,&v).unwrap();let actual=value.as_object_mut().unwrap().remove("digest").unwrap();
  assert_eq!(actual,json!(format!("{:x}",Sha256::digest(serde_json::to_vec(&value).unwrap()))));
  value["selected"]=json!((0..6539).map(|i|json!({"id":format!("f{i}"),"relative":"資料/é/\"quoted\"\\line\n.bin","size":i64::MAX,"availability":"missing","extra":null})).collect::<Vec<_>>());
  assert_eq!(snapshot_digest(&value).unwrap(),format!("{:x}",Sha256::digest(serde_json::to_vec(&value).unwrap())));
  let large_digest=snapshot_digest(&value).unwrap();value["selected"][6538]["size"]=json!(0);assert_ne!(snapshot_digest(&value).unwrap(),large_digest);
 }
 #[test]fn preview_pages_preserve_digest_bound_members_and_reject_changed_snapshots(){
  let(_t,db,rid)=fixture();let mut selection=select(&db,&rid,"b.iso");selection.file_ids.clear();
  {let c=db.lock().unwrap();for i in 0..125{let fid=format!("num{i:03}");c.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES(?1,'r',?2,?2,7,'generated','bin','generated')",params![fid,format!("Numbers/{}.bin",(i*47)%125)]).unwrap();c.execute("INSERT INTO resource_files(resource_id,file_id) VALUES(?1,?2)",params![rid,fid]).unwrap();selection.file_ids.push(fid);}}
  selection.file_ids.reverse();let old=preview(&db,&rid,&selection).unwrap();
  let mut q=PreviewPageRequest{selection:selection.clone(),offset:0,digest:None,library_id:None};let c=db.lock().unwrap();let first=preview_page(&c,&rid,&q).unwrap();assert_eq!(first["digest"],old["digest"]);assert_eq!(first["selected_count"],125);assert_eq!(first["selected_bytes"],875);assert_eq!(first["source"]["file_count"],128);assert!(first["source"].get("files").is_none());assert!(first.get("selected").is_none());assert!(first["items"][0].get("path").is_none());
  let mut collected=Vec::new();q.digest=first["digest"].as_str().map(str::to_owned);q.library_id=first["library_id"].as_str().map(str::to_owned);
  for offset in [0,60,120]{q.offset=offset;let page=preview_page(&c,&rid,&q).unwrap();assert_eq!(page["digest"],old["digest"]);assert!(page["items"].as_array().unwrap().len()<=60);collected.extend(page["items"].as_array().unwrap().iter().map(|f|f["id"].as_str().unwrap().to_owned()));if offset==120{assert!(page["next_offset"].is_null());}}
  let mut ordered=(0..125).collect::<Vec<_>>();ordered.sort_by_key(|i|(i*47)%125);assert_eq!(collected,ordered.into_iter().map(|i|format!("num{i:03}")).collect::<Vec<_>>());
  q.offset=125;assert!(preview_page(&c,&rid,&q).is_err());q.offset=1;assert!(preview_page(&c,&rid,&q).is_err());q.offset=60;q.digest=None;assert!(preview_page(&c,&rid,&q).is_err());q.digest=first["digest"].as_str().map(str::to_owned);q.library_id=Some("foreign".into());assert!(preview_page(&c,&rid,&q).is_err());q.library_id=first["library_id"].as_str().map(str::to_owned);
  c.execute("UPDATE files SET size=8 WHERE id='num124'",[]).unwrap();assert!(preview_page(&c,&rid,&q).unwrap_err().contains("preview changed"));drop(c);
  assert!(apply(&db,&rid,&selection,old["digest"].as_str().unwrap()).is_err());let fresh=preview(&db,&rid,&selection).unwrap();apply(&db,&rid,&selection,fresh["digest"].as_str().unwrap()).unwrap();
 }
 #[test]fn file_visitor_stops_on_consumer_error_and_releases_statement(){
  let(_t,db,rid)=fixture();let c=db.lock().unwrap();let mut visited=0;
  assert_eq!(visit_files(&c,&rid,|_|{visited+=1;Err("consumer stopped".into())}).unwrap_err(),"consumer stopped");assert_eq!(visited,1);
  c.execute("UPDATE files SET availability=availability",[]).unwrap();
  let expected=files(&c,&rid).unwrap();let mut streamed=Vec::new();visit_files(&c,&rid,|file|{streamed.push(file);Ok(())}).unwrap();assert_eq!(streamed,expected);assert_eq!(streamed.len(),3);
  let metadata=resource_metadata(&c,&rid).unwrap();assert!(metadata.get("files").is_none());let mut complete=resource(&c,&rid).unwrap();complete.as_object_mut().unwrap().remove("files");assert_eq!(metadata,complete);
  assert_eq!(c.query_row("SELECT count(*) FROM membership_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn streamed_database_rows_match_full_snapshot_digest_for_split_and_merge(){
  let(_t,db,rid)=fixture();let first=select(&db,&rid,"b.iso");let target=commit(&db,&rid,&first);
  for merge in [false,true]{
   let mut selection=select(&db,&rid,"readme.txt");if merge{selection.target_id=Some(target.clone());selection.target_revision=members(&db,&target).unwrap()["revision"].as_i64();}
   let full=preview(&db,&rid,&selection).unwrap();let mut parts=full.clone();parts.as_object_mut().unwrap().remove("digest");parts.as_object_mut().unwrap().remove("selected");for key in ["source","target"]{if let Some(obj)=parts[key].as_object_mut(){obj.remove("files");}}
   let c=db.lock().unwrap();assert_eq!(stream_snapshot_digest(&c,&rid,&selection,&parts).unwrap(),full["digest"].as_str().unwrap());
   let page=preview_page(&c,&rid,&PreviewPageRequest{selection:selection.clone(),offset:0,digest:None,library_id:None}).unwrap();assert_eq!(page["digest"],full["digest"]);assert_eq!(page["target_relative"],full["target_relative"]);assert_eq!(page["target_title"],full["target_title"]);assert_eq!(page["source"]["file_count"],full["source"]["files"].as_array().unwrap().len());if merge{assert_eq!(page["target"]["file_count"],full["target"]["files"].as_array().unwrap().len());}
   c.execute("UPDATE files SET mtime=?2 WHERE id=?1",params![selection.file_ids[0],format!("changed-{merge}\n資料")]).unwrap();assert_ne!(stream_snapshot_digest(&c,&rid,&selection,&parts).unwrap(),full["digest"].as_str().unwrap());
   assert!(stream_snapshot_digest(&c,&rid,&selection,&full).is_err());
  }
 }
 #[test]fn streamed_preview_rejects_invalid_selection_and_catalog_state(){
  let(_t,db,rid)=fixture();let good=select(&db,&rid,"b.iso");let c=db.lock().unwrap();
  let rejects=|v:&Selection|{assert!(snapshot(&c,&rid,v).is_err());assert!(preview_page(&c,&rid,&PreviewPageRequest{selection:v.clone(),offset:0,digest:None,library_id:None}).is_err());};
  let mut v=good.clone();v.revision+=1;rejects(&v);
  v=good.clone();v.file_ids.clear();rejects(&v);
  v=good.clone();v.file_ids.push(v.file_ids[0].clone());rejects(&v);
  v=good.clone();v.file_ids=vec!["foreign".into()];rejects(&v);
  v=good.clone();v.title=" ".into();rejects(&v);
  v=good.clone();v.target_id=Some(rid.clone());v.target_revision=Some(v.revision);rejects(&v);
  for change in [
   "UPDATE files SET relative_path='../unsafe.bin'",
   "INSERT INTO roots(id,path,label) VALUES('other','other','Other'); UPDATE files SET root_id='other'",
   "INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('ambiguous','r','other','Other','directory'); INSERT INTO resource_files SELECT 'ambiguous',file_id FROM resource_files",
   "INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('busy','scan','running','{}',1,1)",
  ]{c.execute_batch("SAVEPOINT invalid_case").unwrap();c.execute_batch(change).unwrap();rejects(&good);c.execute_batch("ROLLBACK TO invalid_case; RELEASE invalid_case").unwrap();}
  assert_eq!(c.query_row("SELECT count(*) FROM membership_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn selection_order_is_stable_and_ambiguous_ownership_still_rejects(){
  let(_t,db,rid)=fixture();let mut v=select(&db,&rid,"b.iso");v.file_ids=members(&db,&rid).unwrap()["files"].as_array().unwrap().iter().map(|f|f["id"].as_str().unwrap().to_owned()).collect();
  let original=preview(&db,&rid,&v).unwrap();v.file_ids.reverse();assert_eq!(original,preview(&db,&rid,&v).unwrap());
  {let c=db.lock().unwrap();c.execute("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('ambiguous','r','elsewhere','Other','directory')",[]).unwrap();c.execute("INSERT INTO resource_files(resource_id,file_id) VALUES('ambiguous',?1)",[&v.file_ids[0]]).unwrap();}
  assert!(preview(&db,&rid,&v).unwrap_err().contains("ambiguous resource ownership"));
  assert!(apply(&db,&rid,&v,original["digest"].as_str().unwrap()).is_err());
  assert_eq!(db.lock().unwrap().query_row("SELECT count(*) FROM membership_history",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn preview_labels_are_exact_and_changes_invalidate_confirmation(){let(_t,db,rid)=fixture();let v=select(&db,&rid,"b.iso");let p=preview(&db,&rid,&v).unwrap();assert_eq!(p["source"]["bindings"][0]["work_title"],"Game");assert!(p["source"]["bindings"][0]["edition_label"].is_null());db.lock().unwrap().execute("UPDATE works SET title='Renamed generated work' WHERE id='w'",[]).unwrap();assert!(apply(&db,&rid,&v,p["digest"].as_str().unwrap()).is_err());let fresh=preview(&db,&rid,&v).unwrap();assert_eq!(fresh["source"]["bindings"][0]["work_title"],"Renamed generated work");apply(&db,&rid,&v,fresh["digest"].as_str().unwrap()).unwrap();}
 #[test]fn split_keeps_identity_bindings_bytes_and_rescan_membership(){let(t,db,rid)=fixture();let v=select(&db,&rid,"b.iso");let before=members(&db,&rid).unwrap();let p=preview(&db,&rid,&v).unwrap();assert_eq!(before,members(&db,&rid).unwrap());let target=apply(&db,&rid,&v,p["digest"].as_str().unwrap()).unwrap()["id"].as_str().unwrap().to_owned();assert_eq!(members(&db,&target).unwrap()["bindings"],before["bindings"]);assert_eq!(members(&db,&target).unwrap()["files"][0]["id"],v.file_ids[0]);assert_eq!(members(&db,&target).unwrap()["relative"],"Game");assert!(apply(&db,&rid,&v,p["digest"].as_str().unwrap()).is_err());fs::write(t.path().join("source/Game/new.bin"),b"new").unwrap();rescan(&db);assert_eq!(members(&db,&target).unwrap()["files"].as_array().unwrap().len(),1);assert_eq!(members(&db,&rid).unwrap()["files"].as_array().unwrap().len(),3);for f in before["files"].as_array().unwrap(){assert_eq!(fs::read(f["path"].as_str().unwrap()).unwrap(),f["relative"].as_str().unwrap().replace('\\',"/").as_bytes());}assert_eq!(db.lock().unwrap().query_row("SELECT notes FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Keep notes");}
 #[test]fn combine_preserves_destination_references_and_detects_stale_preview(){let(_t,db,rid)=fixture();let target=commit(&db,&rid,&select(&db,&rid,"b.iso"));let mut v=select(&db,&rid,"readme.txt");v.target_id=Some(target.clone());v.target_revision=members(&db,&target).unwrap()["revision"].as_i64();let p=preview(&db,&rid,&v).unwrap();db.lock().unwrap().execute("UPDATE files SET mtime='changed' WHERE id=?1",[&v.file_ids[0]]).unwrap();assert!(apply(&db,&rid,&v,p["digest"].as_str().unwrap()).unwrap_err().contains("preview changed"));assert_eq!(commit(&db,&rid,&v),target);assert_eq!(members(&db,&target).unwrap()["files"].as_array().unwrap().len(),2);assert_eq!(members(&db,&target).unwrap()["work_id"],"w");v.revision=members(&db,&rid).unwrap()["revision"].as_i64().unwrap();v.target_id=Some(rid.clone());assert!(preview(&db,&rid,&v).is_err());}
 #[test]fn approved_plans_block_and_ready_plans_and_paused_transfers_are_retired(){let(t,db,rid)=fixture();let dest=t.path().join("managed");fs::create_dir(&dest).unwrap();let plan=plans::organize(&db,&rid,dest.to_str().unwrap()).unwrap();let v=select(&db,&rid,"b.iso");{let c=db.lock().unwrap();c.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('old','acquire','paused',?1,1,1)",[json!({"resource_id":rid}).to_string()]).unwrap();}let p=preview(&db,&rid,&v).unwrap();assert_eq!(p["retired_plans"],json!([plan["id"]]));assert_eq!(p["retired_jobs"],json!(["old"]));plans::approve(&db,plan["id"].as_str().unwrap(),plan["digest"].as_str().unwrap()).unwrap();assert!(preview(&db,&rid,&v).unwrap_err().contains("approved"));db.lock().unwrap().execute("UPDATE plans SET state='ready',approved=NULL",[]).unwrap();commit(&db,&rid,&v);let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT state FROM plans",[],|r|r.get::<_,String>(0)).unwrap(),"superseded");assert_eq!(c.query_row("SELECT state FROM jobs WHERE id='old'",[],|r|r.get::<_,String>(0)).unwrap(),"superseded");assert_eq!(c.query_row("SELECT count(*) FROM membership_history",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
 #[test]fn split_group_organizes_and_undoes_beside_original_group(){let(t,db,rid)=fixture();let target=commit(&db,&rid,&select(&db,&rid,"b.iso"));let dest=t.path().join("managed");fs::create_dir(&dest).unwrap();let p=plans::organize(&db,&target,dest.to_str().unwrap()).unwrap();assert_eq!(p["items"].as_array().unwrap().len(),1);let pid=p["id"].as_str().unwrap();plans::approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&db,pid).unwrap()["failed"],0);let undo=crate::undo::prepare(&db,pid).unwrap();let uid=undo["id"].as_str().unwrap();plans::approve(&db,uid,undo["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&db,uid).unwrap()["failed"],0);assert_eq!(fs::read(t.path().join("source/Game/b.iso")).unwrap(),b"Game/b.iso");assert!(t.path().join("source/Game/a.bin").exists());rescan(&db);assert_eq!(members(&db,&target).unwrap()["files"].as_array().unwrap().len(),1);}
 #[test]fn unsafe_cross_root_duplicate_and_foreign_members_are_rejected(){let(_t,db,rid)=fixture();let mut v=select(&db,&rid,"b.iso");v.file_ids.push(v.file_ids[0].clone());assert!(preview(&db,&rid,&v).is_err());v.file_ids=vec!["foreign".into()];assert!(preview(&db,&rid,&v).is_err());v=select(&db,&rid,"b.iso");{let c=db.lock().unwrap();c.execute_batch("INSERT INTO roots(id,path,label) VALUES('other','other','other');INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('different','other','x','Different','file');").unwrap();}v.target_id=Some("different".into());v.target_revision=Some(1);assert!(preview(&db,&rid,&v).unwrap_err().contains("same source"));}
 #[test]fn v7_migration_retains_resource_foreign_keys_and_checks_future_writes(){let(t,db,rid)=fixture();let path=t.path().join("state/library.sqlite");drop(db);let c=Connection::open(&path).unwrap();c.execute_batch("PRAGMA foreign_keys=OFF;CREATE TABLE resources_old(id TEXT PRIMARY KEY,root_id TEXT NOT NULL REFERENCES roots(id),relative_path TEXT NOT NULL,title TEXT NOT NULL,kind TEXT NOT NULL,work_id TEXT,release_label TEXT NOT NULL DEFAULT 'Unclassified edition',revision INTEGER NOT NULL DEFAULT 1,UNIQUE(root_id,relative_path));INSERT INTO resources_old SELECT id,root_id,relative_path,title,kind,work_id,release_label,revision FROM resources;DROP TABLE resources;ALTER TABLE resources_old RENAME TO resources;PRAGMA user_version=7;").unwrap();drop(c);let db=db::open(&path).unwrap();let v=select(&db,&rid,"b.iso");commit(&db,&rid,&v);let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("PRAGMA foreign_keys",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert!(c.execute("INSERT INTO resource_files(resource_id,file_id) VALUES('invalid','invalid')",[]).is_err());assert!(t.path().join("state/migration-backups").is_dir());}
 #[test]fn manual_group_moves_only_its_members_to_a_separate_managed_root(){let(t,db,rid)=fixture();let target=commit(&db,&rid,&select(&db,&rid,"b.iso"));let nested=t.path().join("source/Game/Managed");fs::create_dir(&nested).unwrap();assert!(plans::organize(&db,&target,nested.to_str().unwrap()).unwrap_err().contains("separate"));let dest=t.path().join("Managed");fs::create_dir(&dest).unwrap();let p=plans::organize(&db,&target,dest.to_str().unwrap()).unwrap();assert_eq!(p["items"].as_array().unwrap().len(),1);let pid=p["id"].as_str().unwrap();plans::approve(&db,pid,p["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&db,pid).unwrap()["failed"],0);assert!(t.path().join("source/Game/a.bin").exists());assert!(t.path().join("source/Game/extras/readme.txt").exists());let reverse=crate::undo::prepare(&db,pid).unwrap();let uid=reverse["id"].as_str().unwrap();plans::approve(&db,uid,reverse["digest"].as_str().unwrap()).unwrap();assert_eq!(plans::execute(&db,uid).unwrap()["failed"],0);assert_eq!(fs::read(t.path().join("source/Game/b.iso")).unwrap(),b"Game/b.iso");}

}

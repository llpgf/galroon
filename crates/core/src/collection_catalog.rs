//! Bounded collection responses with global filtering, counts and locale sorting.
use axum::{extract::{Query as HttpQuery, State}, http::HeaderMap, Json};
use crate::{App, ApiError, collection_query::{self as filter, Entry, Query}};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request { query: Option<String>, before: Option<String> }
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor { library: String, revision: i64, query: String, roots: String, offset: usize }
type Roots = BTreeMap<String, bool>;
struct View { ids:Vec<String>, metadata:Value, bytes:usize, created:std::time::Instant }
// Only compact result identities and facets are retained, never complete Work bodies.
static VIEWS:OnceLock<Mutex<std::collections::VecDeque<(String,Arc<View>)>>>=OnceLock::new();
const CACHE_BYTES:usize=32*1024*1024;
fn cached(key:&str)->Option<Arc<View>> {
    let mut cache=VIEWS.get_or_init(Default::default).lock().ok()?;
    cache.retain(|(_,view)|view.created.elapsed().as_secs()<60);
    let position=cache.iter().position(|(k,_)|k==key)?;
    let item=cache.remove(position)?;let view=item.1.clone();cache.push_back(item);Some(view)
}
fn retain(key:String,view:Arc<View>) {
    if view.bytes>CACHE_BYTES/2 {return;}
    if let Ok(mut cache)=VIEWS.get_or_init(Default::default).lock() {
        cache.retain(|(k,v)|k!=&key&&v.created.elapsed().as_secs()<60);
        while cache.len()>=8||cache.iter().map(|(_,v)|v.bytes).sum::<usize>()+view.bytes>CACHE_BYTES {cache.pop_front();}
        cache.push_back((key,view));
    }
}
fn check(cancelled:&dyn Fn()->bool)->Result<(),ApiError>{if cancelled(){Err(ApiError("Collection request cancelled".into()))}else{Ok(())}}
struct BuildTrace<'a>{start:std::time::Instant,cancelled:&'a dyn Fn()->bool,complete:bool}
impl<'a> BuildTrace<'a>{fn start(cancelled:&'a dyn Fn()->bool)->Self{crate::diagnostics::record("info","collection.view.started","Collection view started",None);Self{start:std::time::Instant::now(),cancelled,complete:false}}}
impl Drop for BuildTrace<'_>{fn drop(&mut self){crate::diagnostics::record("info",if self.complete{"collection.view.completed"}else if (self.cancelled)(){"collection.view.cancelled"}else{"collection.view.failed"},&format!("Collection view finished in {} ms",self.start.elapsed().as_millis()),None);}}
fn digest<T: Serialize>(value: &T) -> Result<String, ApiError> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value).map_err(|e|ApiError(e.to_string()))?)))
}
fn read_entries(c: &Connection, roots: &Roots,cancelled:&dyn Fn()->bool) -> Result<Vec<Entry>, ApiError> {
    // O(collection) compact metadata only: descriptions, notes and full resource/release payloads stay out.
    let mut statement=c.prepare("SELECT id,title,original_title,developer,released,tags,status,favorite,aliases,rowid FROM works WHERE merged_into IS NULL ORDER BY title")?;
    let mut entries=statement.query_map([],|r| {
        let title:String=r.get(1)?;let original:String=r.get(2)?;let studio:String=r.get(3)?;let released:String=r.get(4)?;
        let tags=filter::tag_names(&serde_json::from_str::<Value>(&r.get::<_,String>(5)?).unwrap_or_default());
        let aliases=serde_json::from_str::<Vec<String>>(&r.get::<_,String>(8)?).unwrap_or_default();
        let search=filter::fold(&[vec![title.clone(),original.clone(),studio.clone()],aliases,tags.clone()].concat().join(" "));
        let year=if released.as_bytes().get(..4).is_some_and(|s|s[0]>=b'1'&&s[0]<=b'9'&&s.iter().all(u8::is_ascii_digit)) { released[..4].to_owned() } else { String::new() };
        Ok(Entry{id:r.get(0)?,title,original,studio:studio.trim().to_owned(),year,tags,status:r.get(6)?,favorite:r.get::<_,i64>(7)?==1,search,added:r.get(9)?,..Default::default()})
    })?.map(|row|{check(cancelled)?;row.map_err(ApiError::from)}).collect::<Result<Vec<_>,ApiError>>()?;
    let index=entries.iter().enumerate().map(|(i,e)|(e.id.clone(),i)).collect::<BTreeMap<_,_>>();
    let mut statement=c.prepare("SELECT w.work_id,e.languages FROM work_releases w JOIN releases e ON e.id=w.release_id")?;
    for pair in statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))? {
        check(cancelled)?;let (id,raw)=pair?;if let Some(i)=index.get(&id) { for lang in serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default() { let lang=filter::normalized(&lang);if !lang.is_empty() { entries[*i].languages.insert(lang); } } }
    }
    // Match legacy resources: only groups containing indexed files are exposed to the collection.
    let mut statement=c.prepare("SELECT b.work_id,r.root_id,count(f.id),sum(f.availability='missing'),sum(f.availability='unverified'),sum(f.availability NOT IN ('present','missing','unverified')) FROM resource_bindings b JOIN resources r ON r.id=b.resource_id JOIN roots rt ON rt.id=r.root_id JOIN resource_files rf ON rf.resource_id=r.id JOIN files f ON f.id=rf.file_id GROUP BY b.work_id,r.id")?;
    for row in statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?)))? {
        check(cancelled)?;let (id,root,count,missing,unverified,unknown)=row?;if let Some(i)=index.get(&id) { let states=&mut entries[*i].states;
            match roots.get(&root) { Some(false)=>{states.insert("offline".into());},None=>{states.insert("unknown".into());},_=>{} }
            if missing>0 { states.insert("missing".into()); } if unverified>0 { states.insert("unverified".into()); }
            if count==0||unknown>0 { states.insert("unknown".into()); } else if roots.get(&root)==Some(&true)&&missing==0&&unverified==0 { states.insert("available".into()); }
        }
    }
    for e in &mut entries { if e.states.is_empty() { e.states.insert("no_resources".into()); } }
    let mut statement=c.prepare("WITH RECURSIVE members(tag,id) AS (SELECT tw.tag_id,tw.work_id FROM custom_tag_works tw JOIN custom_tags t ON t.id=tw.tag_id WHERE t.deleted=0 UNION SELECT m.tag,w.merged_into FROM members m JOIN works w ON w.id=m.id WHERE w.merged_into IS NOT NULL) SELECT DISTINCT m.tag,w.id FROM members m JOIN works w ON w.id=m.id WHERE w.merged_into IS NULL")?;
    for row in statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))? { check(cancelled)?;let (tag,id)=row?;if let Some(i)=index.get(&id) { entries[*i].custom_tags.insert(format!("custom:{tag}")); } }
    Ok(entries)
}
fn collator(locale: &str, numeric: bool, primary: bool) -> Result<icu_collator::CollatorBorrowed<'static>, ApiError> {
    let locale:icu_locale_core::Locale=locale.parse().map_err(|_|ApiError("Invalid collection locale".into()))?;
    let mut prefs:icu_collator::CollatorPreferences=locale.into();
    prefs.numeric_ordering=Some(if numeric {icu_collator::preferences::CollationNumericOrdering::True} else {icu_collator::preferences::CollationNumericOrdering::False});
    let mut options=icu_collator::options::CollatorOptions::default();
    if primary {options.strength=Some(icu_collator::options::Strength::Primary);}
    icu_collator::Collator::try_new(prefs,options).map_err(|e|ApiError(e.to_string()))
}
fn sorted<'a>(entries: &'a [Entry], q: &Query) -> Result<Vec<&'a Entry>, ApiError> {
    let order=collator(&q.locale,true,true)?;let folded_query=filter::fold(&q.query);
    let mut visible=entries.iter().filter(|e|filter::matches(e,q,&folded_query)).collect::<Vec<_>>();
    visible.sort_by(|a,b| {
        let primary=match q.sort.as_str() {"added"=>b.added.cmp(&a.added),"newest"=>b.year.parse::<u16>().unwrap_or(0).cmp(&a.year.parse().unwrap_or(0)),"oldest"=>a.year.parse::<u16>().unwrap_or(9999).cmp(&b.year.parse().unwrap_or(9999)),_=>std::cmp::Ordering::Equal};
        primary.then_with(||order.compare(a.title(&q.title_mode),b.title(&q.title_mode))).then_with(||a.id.cmp(&b.id))
    });Ok(visible)
}
fn view_token(c:&Connection, roots:&Roots, q:&Query)->Result<String,ApiError> {
    let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
    digest(&(crate::context::library(c)?.id,revision,q,roots))
}
pub fn page(c: &Connection, roots: &Roots, q: &Query, before: Option<&str>) -> Result<Value, ApiError> {
    page_cached(c,roots,q,before,None,&||false)
}
fn render(c:&Connection,view:&View,offset:usize,next:Option<String>,cancelled:&dyn Fn()->bool)->Result<Value,ApiError> {
    if offset>0&&offset>=view.ids.len() {return Err(ApiError("Invalid collection cursor".into()));}
    let mut items=Vec::new();for id in view.ids.iter().skip(offset).take(60) {check(cancelled)?;items.push(crate::work_catalog::one(c,id)?.ok_or_else(||ApiError("Collection changed. Reload before continuing.".into()))?);}
    let mut value=view.metadata.clone();value["items"]=json!(items);value["next"]=json!(next);value["offset"]=json!(offset);Ok(value)
}
fn page_cached(c:&Connection,roots:&Roots,q:&Query,before:Option<&str>,instance:Option<&str>,cancelled:&dyn Fn()->bool)->Result<Value,ApiError> {
    check(cancelled)?;
    q.validate().map_err(ApiError)?;
    let labels=collator(&q.locale,false,false)?;
    let library=crate::context::library(c)?.id;
    let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get(0))?;
    let query_hash=digest(q)?;let root_hash=digest(roots)?;
    let offset=if let Some(raw)=before {
        if raw.len()>512 { return Err(ApiError("Invalid collection cursor".into())); }
        let cursor:Cursor=serde_json::from_str(raw).map_err(|_|ApiError("Invalid collection cursor".into()))?;
        if cursor.library!=library||cursor.revision!=revision||cursor.query!=query_hash||cursor.roots!=root_hash { return Err(ApiError("Collection or filters changed. Reload before continuing.".into())); }
        if cursor.offset==0||cursor.offset%60!=0 { return Err(ApiError("Invalid collection cursor".into())); }cursor.offset
    } else {0};
    let token=view_token(c,roots,q)?;let cache_key=instance.map(|id|format!("{id}:{token}"));
    let continuation=|count:usize|->Result<Option<String>,ApiError>{if count.saturating_sub(offset)>60 {Ok(Some(serde_json::to_string(&Cursor{library:library.clone(),revision,query:query_hash.clone(),roots:root_hash.clone(),offset:offset+60}).map_err(|e|ApiError(e.to_string()))?))}else{Ok(None)}};
    if let Some(view)=cache_key.as_deref().and_then(cached) {return render(c,&view,offset,continuation(view.ids.len())?,cancelled);}
    let mut trace=BuildTrace::start(cancelled);
    let entries=read_entries(c,roots,cancelled)?;
    let total=entries.len();
    let mut facets:BTreeMap<&str,BTreeMap<String,String>>=["studio","year","language","tag"].into_iter().map(|k|(k,BTreeMap::new())).collect();
    let mut tag_counts=BTreeMap::<String,usize>::new();let mut statuses=BTreeMap::<String,usize>::new();
    for e in &entries {
        check(cancelled)?;
        *statuses.entry(e.status.clone()).or_default()+=1;
        for (key,values) in [("studio",if e.studio.is_empty(){vec![]}else{vec![e.studio.as_str()]}),("year",if e.year.is_empty(){vec![]}else{vec![e.year.as_str()]}),("language",e.languages.iter().map(String::as_str).collect()),("tag",e.tags.iter().map(String::as_str).collect())] {
            for value in values {facets.get_mut(key).unwrap().entry(format!("value:{}",filter::normalized(value))).or_insert_with(||value.to_owned());}
        }
        for tag in e.tags.iter().map(|t|format!("value:{}",filter::normalized(t))).chain(e.custom_tags.iter().cloned()).collect::<BTreeSet<_>>() { *tag_counts.entry(tag).or_default()+=1; }
    }
    let mut options=serde_json::Map::new();
    for (key,values) in facets {let mut values=values.into_iter().collect::<Vec<_>>();values.sort_by(|a,b|if key=="year" {b.0.cmp(&a.0)}else{labels.compare(&a.1,&b.1).then(a.0.cmp(&b.0))});options.insert(key.into(),json!(values.into_iter().map(|(value,label)|json!({"value":value,"label":label})).collect::<Vec<_>>()));}
    let visible=sorted(&entries,q)?;
    check(cancelled)?;let filtered=visible.len();
    statuses.insert("all".into(),total);
    let custom_tags={let mut s=c.prepare("SELECT id,name FROM custom_tags WHERE deleted=0 ORDER BY name_key")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let ids=visible.iter().map(|e|e.id.clone()).collect::<Vec<_>>();
    let metadata=json!({"total":total,"filtered":filtered,"revision":revision,"library_id":library,"view_token":token,"options":options,"tag_counts":tag_counts,"status_counts":statuses,"custom_tags":custom_tags});
    let bytes=ids.iter().map(|id|id.capacity()+std::mem::size_of::<String>()).sum::<usize>()+metadata.to_string().len()*2;
    let view=Arc::new(View{ids,metadata,bytes,created:std::time::Instant::now()});
    let result=render(c,&view,offset,continuation(filtered)?,cancelled)?;if let Some(key)=cache_key {retain(key,view);}trace.complete=true;Ok(result)
}
fn observed<T>(app:&App, action:impl FnOnce(&Connection,&Roots)->Result<T,ApiError>)->Result<T,ApiError> {
    let mut read=Connection::open_with_flags(app.state_dir.join("library.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let c=read.transaction()?;
    let paths={let mut s=c.prepare("SELECT id,path FROM roots ORDER BY id")?;let rows=s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let roots=paths.iter().map(|(id,path)|(id.clone(),std::fs::metadata(path).map(|m|m.is_dir()).unwrap_or(false))).collect();
    action(&c,&roots)
}
static QUERY_GATE:OnceLock<Arc<tokio::sync::Semaphore>>=OnceLock::new();
struct Cancellation(Arc<AtomicBool>);
impl Drop for Cancellation {fn drop(&mut self){self.0.store(true,Ordering::SeqCst);}}
async fn run_observed(app:App,action:impl FnOnce(&Connection,&Roots,&dyn Fn()->bool)->Result<Value,ApiError>+Send+'static)->crate::Result<Value> {
    let permit=QUERY_GATE.get_or_init(||Arc::new(tokio::sync::Semaphore::new(2))).clone().try_acquire_owned().map_err(|_|ApiError("Other collection queries are running. Retry in a moment.".into()))?;
    let cancelled=Arc::new(AtomicBool::new(false));let _guard=Cancellation(cancelled.clone());
    let result=tokio::task::spawn_blocking(move||{let _permit=permit;observed(&app,|c,roots|action(c,roots,&||cancelled.load(Ordering::SeqCst)))}).await.map_err(|e|ApiError(e.to_string()))??;
    Ok(Json(result))
}
pub async fn get(State(app): State<App>, headers: HeaderMap, HttpQuery(request): HttpQuery<Request>) -> crate::Result<Value> {
    crate::auth(&app,&headers)?;
    let query=if let Some(raw)=request.query { if raw.len()>32768 {return Err(ApiError("Collection query is too long".into()));}serde_json::from_str::<Query>(&raw).map_err(|_|ApiError("Invalid collection query".into()))? }else{Query::default()};
    query.validate().map_err(ApiError)?;
    // Filesystem checks and collation run off the async executor. Never hold the catalog lock during I/O.
    let instance=app.instance_id.clone();run_observed(app,move|c,roots,cancelled|page_cached(c,roots,&query,request.before.as_deref(),Some(&instance),cancelled)).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection { query: Query, view_token: String, ids: Option<Vec<String>> }
#[cfg(test)]fn members(c:&Connection, roots:&Roots, request:&Selection)->Result<Value,ApiError> {
    members_cached(c,roots,request,None,&||false)
}
fn members_cached(c:&Connection,roots:&Roots,request:&Selection,instance:Option<&str>,cancelled:&dyn Fn()->bool)->Result<Value,ApiError> {
    request.query.validate().map_err(ApiError)?;
    if request.view_token!=view_token(c,roots,&request.query)? {return Err(ApiError("Collection or filters changed. Reload before selecting works.".into()));}
    let chosen=if let Some(ids)=&request.ids {
        if ids.is_empty()||ids.len()>10000||ids.iter().any(|id|id.len()>128)||ids.iter().collect::<BTreeSet<_>>().len()!=ids.len() {return Err(ApiError("Select 1–10000 distinct works".into()));}
        ids.clone()
    } else if let Some(view)=instance.and_then(|id|cached(&format!("{id}:{}",request.view_token))) {
        if view.ids.len()>10000 {return Err(ApiError("Select 1–10000 works; refine the filters before continuing".into()));}view.ids.clone()
    } else {let entries=read_entries(c,roots,cancelled)?;sorted(&entries,&request.query)?.iter().map(|e|e.id.clone()).collect()};
    if chosen.is_empty()||chosen.len()>10000 {return Err(ApiError("Select 1–10000 works; refine the filters before continuing".into()));}
    let mut rows=Vec::new();let mut s=c.prepare_cached("SELECT title,original_title FROM works WHERE id=?1 AND merged_into IS NULL")?;
    for id in chosen {check(cancelled)?;let (display,original)=s.query_row([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|_|ApiError("A selected work changed. Reload the collection and try again.".into()))?;
        let title=if request.query.title_mode=="original" {if original.is_empty(){display}else{original}}else if display.is_empty(){original}else{display};rows.push(json!({"work_key":format!("local:{id}"),"title":title}));
    }
    Ok(json!({"members":rows}))
}
pub async fn selection(State(app):State<App>,headers:HeaderMap,Json(request):Json<Selection>)->crate::Result<Value> {
    crate::auth(&app,&headers)?;
    let instance=app.instance_id.clone();run_observed(app,move|c,roots,cancelled|members_cached(c,roots,&request,Some(&instance),cancelled)).await
}

#[cfg(test)] mod tests {
    use super::*;
    use rusqlite::params;
    fn read(c:&Connection,roots:&Roots,q:&Query,before:Option<&str>)->Value {page(c,roots,q,before).unwrap_or_else(|e|panic!("{}",e.0))}
    #[test] fn filtering_and_counts_cover_the_whole_collection_and_cursors_bind_view_and_sources() {
        let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
        for i in 0..125 {c.execute("INSERT INTO works(id,title,original_title,tags,status,developer,released) VALUES(?1,?2,'原題',?3,?4,'Key','2004-04-28')",params![format!("w{i:03}"),format!("Title {i}"),if i%2==0 {"[\"Drama\",\"Ｄｒａｍａ\"]"}else{"[\"Comedy\"]"},if i%2==0 {"backlog"}else{"completed"}]).unwrap();}
        c.execute("UPDATE works SET aliases='[\"OnlyFirstWork\"]' WHERE id='w000'",[]).unwrap();
        let roots=Roots::new();let q=Query::default();let first=read(&c,&roots,&q,None);
        assert_eq!(first["total"],125);assert_eq!(first["filtered"],125);assert_eq!(first["items"].as_array().unwrap().len(),60);assert_eq!(first["tag_counts"]["value:drama"],63);assert_eq!(first["status_counts"]["completed"],62);
        let second=read(&c,&roots,&q,first["next"].as_str());let third=read(&c,&roots,&q,second["next"].as_str());assert_eq!(third["items"].as_array().unwrap().len(),5);assert!(third["next"].is_null());
        let ids=[&first,&second,&third].iter().flat_map(|p|p["items"].as_array().unwrap()).map(|w|w["id"].as_str().unwrap()).collect::<BTreeSet<_>>();assert_eq!(ids.len(),125);
        let mut filtered=q.clone();filtered.query="OnlyFirstWork".into();let found=read(&c,&roots,&filtered,None);assert_eq!(found["items"][0]["id"],"w000");assert_eq!(found["total"],125);assert_eq!(found["filtered"],1);assert_eq!(found["tag_counts"],first["tag_counts"]);
        filtered.query.clear();filtered.filters.tags=vec!["value:drama".into()];let selected=read(&c,&roots,&filtered,None);assert_eq!(selected["filtered"],63);assert_eq!(read(&c,&roots,&filtered,selected["next"].as_str())["items"].as_array().unwrap().len(),3);
        let selection=Selection{query:filtered.clone(),view_token:selected["view_token"].as_str().unwrap().into(),ids:None};
        assert_eq!(members(&c,&roots,&selection).unwrap_or_else(|e|panic!("{}",e.0))["members"].as_array().unwrap().len(),63);
        let selection=Selection{ids:Some(vec!["w124".into(),"w064".into(),"w004".into()]),..selection};
        let resolved=members(&c,&roots,&selection).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(resolved["members"][0]["work_key"],"local:w124");assert_eq!(resolved["members"][2]["work_key"],"local:w004");
        assert!(page(&c,&roots,&filtered,first["next"].as_str()).is_err());
        assert!(page(&c,&Roots::from([("new".into(),false)]),&q,first["next"].as_str()).is_err());
        c.execute("UPDATE works SET title='New title' WHERE id='w000'",[]).unwrap();assert!(page(&c,&roots,&q,first["next"].as_str()).is_err());assert!(members(&c,&roots,&selection).is_err());
    }
    #[test] fn shared_languages_multi_copy_availability_and_merged_custom_tags_keep_identity() {
        let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
        c.execute_batch("INSERT INTO works(id,title,original_title,tags) VALUES('a','A','','[\"Drama\"]'),('b','B','','[]'),('old','Old','','[]'),('none','None','','[]'); UPDATE works SET merged_into='b' WHERE id='old';
        INSERT INTO custom_tags(id,name,name_key) VALUES('personal','Drama','drama'); INSERT INTO custom_tag_works VALUES('personal','old');
        INSERT INTO roots(id,path,label) VALUES('online','generated-online','Online'),('offline','generated-offline','Offline');
        INSERT INTO releases(id,label,languages) VALUES('shared','Shared','[\" JA \",\"en\",\"ja\"]'); INSERT INTO work_releases VALUES('a','shared'),('b','shared');
        INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('good','online','good','Good','archive'),('bad','offline','bad','Bad','archive');
        INSERT INTO resource_bindings(resource_id,work_id,role) VALUES('good','a','main'),('good','b','patch'),('bad','a','main');
        INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES('f1','online','good','good',1,'1','zip','fixture','present'),('f2','offline','bad','bad',1,'1','zip','fixture','missing'); INSERT INTO resource_files VALUES('good','f1'),('bad','f2');").unwrap();
        let roots=Roots::from([("online".into(),true),("offline".into(),false)]);let mut q=Query::default();
        let p=read(&c,&roots,&q,None);assert_eq!(p["total"],3);assert_eq!(p["tag_counts"]["custom:personal"],1);assert_eq!(p["tag_counts"]["value:drama"],1);
        for state in ["available","offline","missing"] {q.filters.availability=state.into();let p=read(&c,&roots,&q,None);assert!(p["items"].as_array().unwrap().iter().any(|w|w["id"]=="a"));}
        q.filters.availability="no_resources".into();assert_eq!(read(&c,&roots,&q,None)["items"][0]["id"],"none");
        q.filters.availability.clear();q.filters.language="value:ja".into();assert_eq!(read(&c,&roots,&q,None)["filtered"],2);
        q.filters.tags=vec!["custom:personal".into()];assert_eq!(read(&c,&roots,&q,None)["items"][0]["id"],"b");q.filters.tags=vec!["value:drama".into()];assert_eq!(read(&c,&roots,&q,None)["items"][0]["id"],"a");
        c.execute("UPDATE files SET availability='legacy-unknown' WHERE id='f1'",[]).unwrap();q.filters.tags.clear();q.filters.availability="available".into();assert_eq!(read(&c,&roots,&q,None)["filtered"],0);q.filters.availability="unknown".into();assert_eq!(read(&c,&roots,&q,None)["filtered"],2);
    }
    #[test] fn natural_locale_sort_and_original_title_mode_are_deterministic() {
        let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
        c.execute_batch("INSERT INTO works(id,title,original_title,released) VALUES('one','Episode 10','Episode 2',''),('two','Episode 2','Episode 10','2001'),('three','Café','Z','2002');").unwrap();
        let mut q=Query::default();q.sort="title".into();let roots=Roots::new();
        let ids=|p:Value|p["items"].as_array().unwrap().iter().map(|w|w["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        assert_eq!(ids(read(&c,&roots,&q,None)),["three","two","one"]);q.title_mode="original".into();assert_eq!(ids(read(&c,&roots,&q,None)),["one","two","three"]);
        q.sort="oldest".into();assert_eq!(ids(read(&c,&roots,&q,None)),["two","three","one"]);q.sort="newest".into();assert_eq!(ids(read(&c,&roots,&q,None)),["three","two","one"]);
        q.locale="not a locale".into();assert!(page(&c,&roots,&q,None).is_err());
    }
    #[test] fn cache_keeps_only_identities_and_cancelled_reads_never_publish_partial_views() {
        let temp=tempfile::tempdir().unwrap();let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
        for i in 0..125 {c.execute("INSERT INTO works(id,title,original_title,description) VALUES(?1,'Generated','','Large description must not be cached')",[format!("w{i}")]).unwrap();}
        let roots=Roots::new();let q=Query::default();let instance=crate::db::id();
        let first=page_cached(&c,&roots,&q,None,Some(&instance),&||false).unwrap_or_else(|e|panic!("{}",e.0));
        let key=format!("{instance}:{}",first["view_token"].as_str().unwrap());let view=cached(&key).unwrap();assert_eq!(view.ids.len(),125);assert!(view.metadata.get("items").is_none());assert!(!view.metadata.to_string().contains("Large description"));
        let second=page_cached(&c,&roots,&q,first["next"].as_str(),Some(&instance),&||false).unwrap_or_else(|e|panic!("{}",e.0));assert_eq!(second["offset"],60);assert_eq!(second["items"].as_array().unwrap().len(),60);
        assert!(page_cached(&c,&roots,&q,first["next"].as_str(),Some(&instance),&||true).is_err());
        let fresh=crate::db::id();let checks=std::cell::Cell::new(0);
        assert!(page_cached(&c,&roots,&q,None,Some(&fresh),&||{checks.set(checks.get()+1);checks.get()>10}).is_err());assert!(cached(&format!("{fresh}:{}",first["view_token"].as_str().unwrap())).is_none());
        c.execute("UPDATE works SET title='Changed' WHERE id='w0'",[]).unwrap();assert!(page_cached(&c,&roots,&q,first["next"].as_str(),Some(&instance),&||false).is_err());
    }
    #[tokio::test] async fn http_readonly_pages_and_owner_selection_enforce_scope_without_catalog_writes() {
        let temp=tempfile::tempdir().unwrap();let app=crate::initialize(temp.path().join("state")).unwrap();crate::access::setup(&app.db,"Generated collection test password",false).unwrap();
        {let c=app.db.lock().unwrap();for i in 0..61{c.execute("INSERT INTO works(id,title,original_title) VALUES(?1,?2,'')",params![format!("http-{i}"),format!("Title {i}")]).unwrap();}}
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let server_app=app.clone();let server=tokio::spawn(async move{axum::serve(listener,crate::router(server_app)).await.unwrap()});let client=reqwest::Client::new();
        assert_eq!(client.get(format!("{base}/api/collection")).send().await.unwrap().status(),reqwest::StatusCode::UNAUTHORIZED);
        let login=client.post(format!("{base}/api/access/login")).header("Origin",&base).json(&json!({"password":"Generated collection test password","name":"Generated read-only collection"})).send().await.unwrap().error_for_status().unwrap();let raw=login.headers()["set-cookie"].to_str().unwrap().to_owned();let cookie=raw.split(';').next().unwrap();
        let first:Value=client.get(format!("{base}/api/collection")).header("Cookie",cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(first["items"].as_array().unwrap().len(),60);assert_eq!(first["filtered"],61);
        let selection=json!({"query":Query::default(),"view_token":first["view_token"],"ids":null});
        assert_eq!(client.post(format!("{base}/api/collection/selection")).header("Cookie",cookie).json(&selection).send().await.unwrap().status(),reqwest::StatusCode::FORBIDDEN);
        let result:Value=client.post(format!("{base}/api/collection/selection")).bearer_auth(&app.token).json(&selection).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(result["members"].as_array().unwrap().len(),61);
        assert_eq!(app.db.lock().unwrap().query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),first["revision"].as_i64().unwrap());
        let second:Value=client.get(format!("{base}/api/collection")).query(&[("before",first["next"].as_str().unwrap())]).header("Cookie",cookie).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();assert_eq!(second["items"].as_array().unwrap().len(),1);
        app.db.lock().unwrap().execute("UPDATE works SET title='Changed' WHERE id='http-0'",[]).unwrap();assert_eq!(client.post(format!("{base}/api/collection/selection")).bearer_auth(&app.token).json(&selection).send().await.unwrap().status(),reqwest::StatusCode::BAD_REQUEST);
        server.abort();let _=server.await;
    }
}


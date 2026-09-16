pub mod resource_page;
pub mod resource_members_page;
pub mod edition_options;
pub mod checkpoint;
pub mod file_audit;
pub mod job_history;
pub mod job_summary;
pub mod work_catalog;
pub mod resource_catalog;
pub mod collection_query;
pub mod collection_catalog;
mod archive_process;
mod managed_root;
mod smart_organization;
pub mod archive_manifest;
pub mod relation_editor;
pub mod manual_profile;
pub mod relation_correction_index;
pub mod relation_projection;
pub mod relation_correction_store;
pub mod relation_corrections;
pub mod artwork_cache;
pub mod artwork;
mod artwork_queue;
mod artwork_references;
pub mod smart_relations;
pub mod smart_updates;
pub mod smart_snapshots;
pub mod smart_lists;
pub mod smart_facts;
pub mod smart_rules;
pub mod split_private;
pub mod lists;
mod timeline;
mod acquire_issues;
mod source_issues;
pub mod automatch;
pub mod issues;
pub mod diagnostics;
pub mod issue_scan;
pub mod match_history;
pub mod matching;
pub mod context;
pub mod membership;
pub mod db;
pub mod scan;
pub mod plans;
pub mod undo;
pub mod acquire;
pub mod metadata;
pub mod exploration;
pub mod discovery;
pub mod discovery_relations;
pub mod custom_tags;
pub mod entity_tags;
pub mod relation_overrides;
pub mod entity_overrides;
pub mod grouping;
pub mod backup;
pub mod roots;
pub mod editions;
pub mod edition_membership;
pub mod multipart;
pub mod hashing;
pub mod watching;
pub mod access;
#[cfg(windows)] pub mod local_core;
use axum::{routing::{get,post},Router,extract::{State,Path,Query},Json,http::{StatusCode,HeaderMap},response::{IntoResponse,Response}};
use serde_json::{Value,json};
use rusqlite::{params,OptionalExtension};
use std::path::PathBuf;
use tower_http::cors::{CorsLayer,AllowOrigin};

#[derive(Clone)] pub struct App {pub device:context::Device,pub instance_id:String,pub db:db::Db,pub token:String,pub state_dir:PathBuf,pub process_lock:std::sync::Arc<std::fs::File>,pub mutation_lock:std::sync::Arc<std::sync::Mutex<()>>,pub login_gate:std::sync::Arc<tokio::sync::Semaphore>}
pub struct ApiError(pub String);
impl IntoResponse for ApiError {fn into_response(self)->Response{diagnostics::record("error","api.error",&self.0,None);(if self.0=="Authentication required"{StatusCode::UNAUTHORIZED}else if self.0=="Local owner access required"||self.0=="Desktop access required"{StatusCode::FORBIDDEN}else{StatusCode::BAD_REQUEST},Json(json!({"error":self.0}))).into_response()}}
impl From<rusqlite::Error> for ApiError {fn from(e:rusqlite::Error)->Self{Self(e.to_string())}}
impl From<String> for ApiError {fn from(e:String)->Self{Self(e)}}
type Result<T> = std::result::Result<Json<T>,ApiError>;
fn auth(a:&App,h:&HeaderMap)->std::result::Result<(),ApiError>{access::authenticate(a,h)?;Ok(())}
pub fn initialize(dir:PathBuf)->std::result::Result<App,String>{
    let device_dir=dir.clone();initialize_with_device(dir,&device_dir)
}
pub fn initialize_with_device(dir:PathBuf,device_dir:&std::path::Path)->std::result::Result<App,String>{
    let device=context::device(device_dir)?;
    std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let lock=std::fs::OpenOptions::new().create(true).read(true).write(true).open(dir.join("core.lock")).map_err(|e|e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_|"This collection is already open in another Core")?;
    let db=db::open(&dir.join("library.sqlite")).map_err(|e|e.to_string())?;
    db.lock().map_err(|e|e.to_string())?.execute("DELETE FROM settings WHERE key='local_token'",[]).map_err(|e|e.to_string())?;
    let token=format!("{}{}",db::id(),db::id());
    db.lock().map_err(|e|e.to_string())?.execute("UPDATE plans SET state='partial' WHERE state='executing'",[]).map_err(|e|e.to_string())?;
    Ok(App{device,instance_id:db::id(),db,token,state_dir:dir,process_lock:std::sync::Arc::new(lock),mutation_lock:Default::default(),login_gate:std::sync::Arc::new(tokio::sync::Semaphore::new(1))})
}
pub fn router(app:App)->Router {
    Router::new().route("/api/health",get(||async{Json(json!({"name":"Galroon","version":env!("CARGO_PKG_VERSION")}))}))
    .route("/api/context",get(context::get).post(context::update))
    .route("/api/access/status",get(access::status)).route("/api/access/me",get(access::me))
    .route("/api/access/setup",post(access::configure)).route("/api/access/password",post(access::change_password))
    .route("/api/access/login",post(access::web_login)).route("/api/access/logout",post(access::logout))
    .route("/api/access/pairing",post(access::pairing)).route("/api/access/pair",post(access::pair))
    .route("/api/access/devices",get(access::devices)).route("/api/access/devices/{id}/revoke",post(access::revoke))
    .route("/api/dashboard",get(dashboard)).route("/api/roots",get(roots).post(add_root))
    .route("/api/roots/{id}/watch",get(watch_status).post(configure_watch))
    .route("/api/roots/{id}/remap/preview",post(preview_root_remap)).route("/api/roots/{id}/remap",post(remap_root))
    .route("/api/scans",post(start_scan)).route("/api/jobs",get(job_history::list)).route("/api/jobs/{id}",get(job_history::detail)).route("/api/timeline",get(timeline::list)).route("/api/issues/{id}/task",get(acquire_issues::task)).route("/api/jobs/{id}/{action}",post(control_job))
    .route("/api/resources/page",get(resource_page::read)).route("/api/resources",get(resources)).route("/api/works",get(work_catalog::list).post(create_work))
    .route("/api/works/lookup",get(work_catalog::lookup))
    .route("/api/works/{id}/assets",get(work_catalog::assets))
    .route("/api/collection",get(collection_catalog::get))
    .route("/api/collection/selection",post(collection_catalog::selection))
    .route("/api/issues",get(issues::list)).route("/api/issues/{id}",post(issues::edit))
    .route("/api/diagnostics",get(diagnostics::read))
    .route("/api/vns/{id}/relationship-decisions",get(relation_overrides::get).post(relation_overrides::edit))
    .route("/api/vns/{id}/relationship-history",get(relation_overrides::history))
    .route("/api/vns/{id}/relationship-corrections",get(relation_editor::get).post(relation_editor::commit))
    .route("/api/vns/{id}/relationship-corrections/preview",axum::routing::post(relation_editor::preview))
    .route("/api/vns/{id}/relationship-corrections/history",get(relation_editor::history))
    .route("/api/relationship-candidates/{id}",get(relation_editor::candidate))
    .route("/api/entities/{id}/manual-works",get(manual_profile::get))
    .route("/api/entities/{id}/field-decisions",get(entity_overrides::get).post(entity_overrides::edit))
    .route("/api/entities/{id}/field-history",get(entity_overrides::history))
    .route("/api/issues/{id}/retry",post(issues::retry))
    .route("/api/issues/{id}/scan-preview",post(issue_scan::preview)).route("/api/issues/{id}/scan",post(issue_scan::confirm))
    .route("/api/works/{id}/preferred-edition",post(prefer_edition)).route("/api/works/{id}",get(work_catalog::detail).post(update_work)).route("/api/resources/{id}/bind",post(bind_resource))
    .route("/api/works/{id}/edition-options",get(edition_options::read))
    .route("/api/releases/labels",get(edition_options::labels))
    .route("/api/releases",get(list_editions).post(create_edition)).route("/api/releases/{id}",get(work_catalog::edition).post(edit_edition))
    .route("/api/releases/{id}/membership",get(edition_membership::read).post(edition_membership::write))
    .route("/api/resources/{id}/references",post(edit_references))
    .route("/api/resources/{id}/members",get(resource_members))
    .route("/api/resources/{id}/members/page",get(resource_members_page::read))
    .route("/api/resources/{id}/matching-history",get(matching_history))
    .route("/api/resources/{id}/regroup/preview",post(preview_regroup))
    .route("/api/resources/{id}/regroup/preview/page",post(preview_regroup_page))
    .route("/api/resources/{id}/regroup",post(regroup_resource))
    .route("/api/vndb/search",post(vndb_search))
    .route("/api/matching",get(matching_status)).route("/api/matching/start",post(start_matching)).route("/api/matching/refresh",post(refresh_matching))
    .route("/api/works/{id}/metadata",get(get_metadata).post(edit_metadata))
    .route("/api/works/{id}/refresh",post(refresh_metadata))
    .route("/api/works/{id}/exploration",get(exploration::work))
    .route("/api/people/{id}",get(exploration::person))
    .route("/api/characters/{id}",get(exploration::character))
    .route("/api/companies/{id}",get(exploration::company))
    .route("/api/discover",get(discovery::search))
    .route("/api/discover/manual",get(discovery_relations::manual))
    .route("/api/artwork",get(artwork::read))
    .route("/api/artwork/cache",get(artwork::stats).post(artwork::clear))
    .route("/api/vns/{id}/exploration",get(exploration::reference_work))
    .route("/api/lists",get(lists::index)).route("/api/lists/{id}",get(lists::read).post(lists::edit))
    .route("/api/vns/{id}/related-tags",get(entity_tags::reference_reasons))
    .route("/api/works/{id}/related-tags",get(entity_tags::work_reasons))
    .route("/api/entity-tags/{id}",get(entity_tags::read).post(entity_tags::edit))
    .route("/api/custom-tags",get(custom_tags::list).post(custom_tags::create)).route("/api/custom-tags/{id}",post(custom_tags::edit)).route("/api/custom-tags/{id}/membership",get(custom_tags::membership))
    .route("/api/smart-lists/options",get(smart_relations::options)).route("/api/smart-lists/preview",post(smart_snapshots::preview)).route("/api/smart-lists/{id}/to-manual",post(smart_snapshots::to_manual)).route("/api/smart-lists/{id}/results",get(smart_snapshots::read)).route("/api/smart-lists/{id}/compute",post(smart_snapshots::refresh))
    .route("/api/smart-lists",get(smart_lists::index)).route("/api/smart-lists/{id}",get(smart_lists::read).post(smart_lists::edit))
    .route("/api/works/{id}/split-preview",get(split_preview)).route("/api/works/{id}/merge",post(merge_work)).route("/api/works/{id}/split",post(split_work))
    .route("/api/backups",get(backup_history).post(export_backup)).route("/api/backups/inspect",post(inspect_backup))
    .route("/api/plans",get(list_plans)).route("/api/plans/organize",post(organize_plan))
    .route("/api/plans/{id}",get(get_plan)).route("/api/plans/{id}/undo",post(undo_plan)).route("/api/plans/{id}/approve",post(approve_plan)).route("/api/plans/{id}/execute",post(execute_plan))
    .route("/api/duplicates",post(find_duplicates)).route("/api/duplicate-checks/{id}",get(duplicate_results)).route("/api/quarantine",get(list_quarantine).post(isolate_plan))
    .route("/api/quarantine/{id}/restore",post(restore_plan))
    .route("/api/acquire",post(start_acquire)).route("/api/resources/{id}/acquire-preview",get(acquire_preview)).route("/api/archive/capabilities",get(archive_capabilities))
    .fallback_service(tower_http::services::ServeDir::new(std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("web"))).unwrap_or_default()))
    .layer(axum::middleware::from_fn_with_state(app.clone(),access::guard))
    .layer(axum::middleware::from_fn(diagnostics::requests))
    .layer(axum::middleware::from_fn(access::response_headers))
    .layer(CorsLayer::new().allow_credentials(true).allow_origin(AllowOrigin::list(["http://localhost:1420","http://127.0.0.1:1420","http://tauri.localhost","tauri://localhost"].into_iter().map(|s|s.parse().unwrap()))).allow_methods([axum::http::Method::GET,axum::http::Method::POST]).allow_headers([axum::http::header::CONTENT_TYPE,axum::http::header::AUTHORIZATION,axum::http::HeaderName::from_static("x-galroon-library"),axum::http::HeaderName::from_static("x-galroon-device")]).expose_headers([axum::http::HeaderName::from_static("x-galroon-library"),axum::http::HeaderName::from_static("x-galroon-device")]))
    .with_state(app)
}
pub async fn serve(app:App,port:u16)->std::result::Result<(),String>{let listener=tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,port)).await.map_err(|e|e.to_string())?;serve_listener(app,listener).await}
pub async fn serve_listener(app:App,listener:tokio::net::TcpListener)->std::result::Result<(),String>{let _watching=watching::start(app.clone())?;let _matching=automatch::start(app.db.clone());let _smart=smart_updates::start(app.db.clone());axum::serve(listener,router(app)).await.map_err(|e|e.to_string())}
async fn dashboard(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let c=a.db.lock().unwrap();let count=|table:&str|c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get::<_,i64>(0)).unwrap_or(0);let bytes:i64=c.query_row("SELECT coalesce(sum(size),0) FROM files WHERE availability='present'",[],|r|r.get(0))?;let unassigned:i64=c.query_row("SELECT count(*) FROM resources r WHERE work_id IS NULL AND EXISTS(SELECT 1 FROM resource_files rf WHERE rf.resource_id=r.id)",[],|r|r.get(0))?;Ok(Json(json!({"works":c.query_row("SELECT count(*) FROM works WHERE merged_into IS NULL",[],|r|r.get::<_,i64>(0))?,"revision":c.query_row("SELECT revision FROM smart_catalog_state WHERE singleton=1",[],|r|r.get::<_,i64>(0))?,"files":count("files"),"resources":c.query_row("SELECT count(*) FROM resources r WHERE EXISTS(SELECT 1 FROM resource_files rf WHERE rf.resource_id=r.id)",[],|r|r.get::<_,i64>(0))?,"roots":count("roots"),"bytes":bytes,"unassigned":unassigned})))}
async fn roots(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let mut rows={let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT rt.id,rt.path,rt.label,rt.role,(SELECT count(*) FROM files f WHERE f.root_id=rt.id AND f.availability='missing'),(SELECT count(*) FROM files f WHERE f.root_id=rt.id AND f.availability='unverified') FROM roots rt ORDER BY label")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"path":r.get::<_,String>(1)?,"label":r.get::<_,String>(2)?,"role":r.get::<_,String>(3)?,"missing":r.get::<_,i64>(4)?,"unverified":r.get::<_,i64>(5)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;rows};for row in &mut rows{row["online"]=json!(std::fs::metadata(row["path"].as_str().unwrap()).map(|m|m.is_dir()).unwrap_or(false));}Ok(Json(json!(rows)))}
async fn add_root(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let p=v["path"].as_str().ok_or(ApiError("Choose a source folder".into()))?;let p=scan::canonical(std::path::Path::new(p))?;if !p.is_dir(){return Err(ApiError("Source must be a folder".into()));}let label=v["label"].as_str().filter(|x|!x.is_empty()).unwrap_or("Source");let c=a.db.lock().unwrap();let rid=db::id();c.execute("INSERT INTO roots(id,path,label) VALUES(?1,?2,?3)",params![rid,p.to_string_lossy(),label])?;watching::ensure(&c)?;Ok(Json(json!({"id":rid})))}
async fn start_scan(State(a):State<App>,h:HeaderMap,Json(spec):Json<scan::ScanSpec>)->Result<Value>{auth(&a,&h)?;let jid={let _lock=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;scan::create(&a.db,spec)?};let db=a.db.clone();let run=jid.clone();tokio::task::spawn_blocking(move||scan::run(db,run));Ok(Json(json!({"id":jid})))}

#[derive(serde::Deserialize)]struct HistoryQuery{before:Option<i64>,#[serde(default)]attempts:bool}
async fn matching_history(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Query(q):Query<HistoryQuery>)->Result<Value>{
 let who=access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}
 let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(if q.attempts{match_history::attempts(&c,&id,q.before)?}else{match_history::page(&c,&id,q.before)?}))
}
async fn control_job(State(a):State<App>,h:HeaderMap,Path((jid,action)):Path<(String,String)>,Json(body):Json<Value>)->Result<Value>{auth(&a,&h)?;{let mut c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;let tx=c.transaction()?;timeline::control(&tx,&jid,&action)?;tx.commit()?;}if action=="resume"{let kind:String=a.db.lock().unwrap().query_row("SELECT kind FROM jobs WHERE id=?1",[&jid],|r|r.get(0))?;let db=a.db.clone();let run=jid.clone();tokio::task::spawn_blocking(move||{if kind=="match"{}else if kind=="acquire"{acquire::run(db,run,body["password"].as_str().map(str::to_owned))}else if kind=="hash"{hashing::run(db,run)}else{scan::run(db,run)}});}Ok(Json(json!({"id":jid})))}
async fn resources(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let c=a.db.lock().unwrap();Ok(Json(json!(resource_catalog::rows(&c,None)?)))}
async fn create_work(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let title=v["title"].as_str().filter(|s|!s.trim().is_empty()).ok_or(ApiError("A title is required".into()))?;let wid=db::id();let c=a.db.lock().unwrap();if let Some(external)=v["vndb_id"].as_str(){use rusqlite::OptionalExtension;let existing:Option<String>=c.query_row("SELECT id FROM works WHERE vndb_id=?1",[external],|r|r.get(0)).optional()?;if let Some(mut current)=existing{loop{let merged:Option<String>=c.query_row("SELECT merged_into FROM works WHERE id=?1",[&current],|r|r.get(0))?;if let Some(next)=merged{current=next;}else{return Ok(Json(json!({"id":current})));}}}}c.execute("INSERT INTO works(id,title,original_title,vndb_id,description,cover,developer,released,tags,source_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![wid,title,v["original_title"].as_str().unwrap_or(title),v["vndb_id"].as_str(),v["description"].as_str().unwrap_or(""),v["cover"].as_str().unwrap_or(""),v["developer"].as_str().unwrap_or(""),v["released"].as_str().unwrap_or(""),v.get("tags").unwrap_or(&json!([])).to_string(),v.to_string()])?;Ok(Json(json!({"id":wid})))}
async fn update_work(State(a):State<App>,h:HeaderMap,Path(wid):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let mut connection=a.db.lock().unwrap();let c=connection.transaction()?;let revision=v["revision"].as_i64().ok_or(ApiError("Revision is required".into()))?;let old:i64=c.query_row("SELECT revision FROM works WHERE id=?1",[&wid],|r|r.get(0))?;if old!=revision{return Err(ApiError("This work changed. Reload before editing.".into()));}for field in ["title","notes","status"]{if let Some(value)=v[field].as_str(){if field=="status" && !["backlog","playing","completed","on_hold","dropped"].contains(&value){return Err(ApiError("Unknown status".into()));}c.execute(&format!("UPDATE works SET {field}=?2 WHERE id=?1"),params![wid,value])?;}}if let Some(f)=v["favorite"].as_bool(){c.execute("UPDATE works SET favorite=?2 WHERE id=?1",params![wid,f])?;}c.execute("UPDATE works SET revision=revision+1,overrides=json_patch(overrides,?2) WHERE id=?1",params![wid,v.to_string()])?;c.commit()?;Ok(Json(json!({"id":wid,"revision":old+1})))}
async fn bind_resource(State(a):State<App>,h:HeaderMap,Path(rid):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let revision=v["revision"].as_i64().ok_or(ApiError("Resource revision is required".into()))?;Ok(Json(editions::bind_primary(&a.db,&rid,&text(&v,"work_id")?,v["release"].as_str().unwrap_or("Unclassified edition"),revision)?))}
async fn list_editions(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;Ok(Json(editions::list(&a.db.lock().unwrap())?))}
async fn create_edition(State(a):State<App>,h:HeaderMap,Json(v):Json<editions::EditionInput>)->Result<Value>{auth(&a,&h)?;Ok(Json(editions::save(&a.db,None,v)?))}
async fn edit_edition(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<editions::EditionInput>)->Result<Value>{auth(&a,&h)?;Ok(Json(editions::save(&a.db,Some(&id),v)?))}
async fn edit_references(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<editions::EditBindings>)->Result<Value>{auth(&a,&h)?;Ok(Json(editions::set_bindings(&a.db,&id,v)?))}
async fn matching_status(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;Ok(Json(automatch::status(&a.db)?))}
async fn start_matching(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||automatch::kick(&a.db,false)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"job_id":result})))}
async fn refresh_matching(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||automatch::kick(&a.db,true)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"job_id":result})))}
async fn vndb_search(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{
    auth(&a,&h)?;
    let query=v["query"].as_str().ok_or(ApiError("Search text required".into()))?;
    let mut hints=Vec::new();
    if let Some(id)=v["resource_id"].as_str(){
        let c=a.db.lock().unwrap();
        let path:String=c.query_row("SELECT relative_path FROM resources WHERE id=?1",[id],|r|r.get(0))?;
        hints.push(path);
        let mut stmt=c.prepare("SELECT f.relative_path FROM files f JOIN resource_files rf ON rf.file_id=f.id WHERE rf.resource_id=?1 ORDER BY length(f.relative_path) DESC,f.relative_path LIMIT 32")?;
        hints.extend(stmt.query_map([id],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?);
    }
    let input=matching::prepare(query,&hints)?;
    Ok(Json(matching::search(input).await?))
}

fn text(v:&Value,key:&str)->std::result::Result<String,ApiError>{v[key].as_str().filter(|s|!s.is_empty()).map(str::to_owned).ok_or(ApiError(format!("{key} is required")))}
async fn get_metadata(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{auth(&a,&h)?;Ok(Json(metadata::snapshot(&a.db,&id)?))}
async fn preview_root_remap(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let path=text(&v,"path")?;let result=tokio::task::spawn_blocking(move||roots::preview(&a.db,&id,std::path::Path::new(&path))).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!(result)))}
async fn remap_root(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let path=text(&v,"path")?;let digest=text(&v,"digest")?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;roots::remap(&a.db,&id,std::path::Path::new(&path),&digest)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!(result)))}
async fn backup_history(State(a):State<App>,h:HeaderMap)->Result<Value>{let who=access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(Json(backup::history(&a.db)?))}
async fn export_backup(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let destination=text(&v,"destination")?;let path=tokio::task::spawn_blocking(move||backup::export(&a.db,std::path::Path::new(&destination))).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"path":path})))}
async fn inspect_backup(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let path=text(&v,"path")?;let manifest=tokio::task::spawn_blocking(move||backup::inspect(std::path::Path::new(&path))).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(serde_json::to_value(manifest).map_err(|e|ApiError(e.to_string()))?))}
async fn merge_work(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let r=v["revision"].as_i64().ok_or(ApiError("Revision required".into()))?;let tr=v["target_revision"].as_i64().ok_or(ApiError("Destination revision required".into()))?;Ok(Json(grouping::merge(&a.db,&id,&text(&v,"target_id")?,r,tr)?))}
async fn split_preview(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{if access::authenticate(&a,&h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(split_private::preview(&c,&id)?))}
async fn split_work(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let r=v["revision"].as_i64().ok_or(ApiError("Revision required".into()))?;let members:Vec<String>=serde_json::from_value(v["resources"].clone()).map_err(|e|ApiError(e.to_string()))?;let private:Option<split_private::Selection>=serde_json::from_value(v["private"].clone()).map_err(|e|ApiError(e.to_string()))?;Ok(Json(grouping::split_reviewed(&a.db,&id,r,&members,&text(&v,"title")?,private.as_ref())?))}
async fn edit_metadata(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let revision=v["revision"].as_i64().ok_or(ApiError("Revision required".into()))?;let reset:Vec<String>=serde_json::from_value(v.get("reset").cloned().unwrap_or(json!([]))).map_err(|e|ApiError(e.to_string()))?;let revision=metadata::apply(&a.db,&id,revision,v.get("patch").unwrap_or(&json!({})),&reset,None)?;Ok(Json(json!({"revision":revision})))}
async fn refresh_metadata(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let revision=v["revision"].as_i64().ok_or(ApiError("Revision required".into()))?;let external:Option<String>=a.db.lock().unwrap().query_row("SELECT vndb_id FROM works WHERE id=?1",[&id],|r|r.get(0))?;let source=metadata::fetch(&external.ok_or(ApiError("This work has no VNDB source".into()))?).await?;let revision=metadata::apply(&a.db,&id,revision,&json!({}),&[],Some(source))?;Ok(Json(json!({"revision":revision})))}
async fn organize_plan(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let rid=text(&v,"resource_id")?;let dest=text(&v,"destination")?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;plans::organize(&a.db,&rid,&dest)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
async fn undo_plan(State(a):State<App>,h:HeaderMap,Path(pid):Path<String>)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;undo::prepare(&a.db,&pid)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
async fn approve_plan(State(a):State<App>,h:HeaderMap,Path(pid):Path<String>,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;plans::approve(&a.db,&pid,&text(&v,"digest")?)?;Ok(Json(json!({"id":pid,"state":"approved"})))}
async fn execute_plan(State(a):State<App>,h:HeaderMap,Path(pid):Path<String>)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;plans::execute(&a.db,&pid)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
async fn find_duplicates(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let root_ids=if let Some(ids)=v.get("root_ids"){serde_json::from_value::<Vec<String>>(ids.clone()).map_err(|e|ApiError(e.to_string()))?}else{vec![text(&v,"root_id")?]};let jid={let _lock=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;hashing::create(&a.db,hashing::Spec{root_ids})?};let run=jid.clone();let db=a.db.clone();tokio::task::spawn_blocking(move||hashing::run(db,run));Ok(Json(json!({"job_id":jid})))}
async fn duplicate_results(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||hashing::results(&a.db,&id)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
async fn isolate_plan(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let fid=text(&v,"file_id")?;let keep=text(&v,"retained_id")?;let dest=text(&v,"destination")?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;plans::isolate(&a.db,&fid,&keep,&dest)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
async fn restore_plan(State(a):State<App>,h:HeaderMap,Path(qid):Path<String>)->Result<Value>{auth(&a,&h)?;let result=tokio::task::spawn_blocking(move||{let _lock=a.mutation_lock.lock().map_err(|e|e.to_string())?;plans::restore(&a.db,&qid)}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
#[derive(serde::Deserialize)]struct PlanQuery{before:Option<String>}
fn plan_details(c:&rusqlite::Connection,pid:&str)->std::result::Result<Value,rusqlite::Error>{
 let mut p=c.query_row("SELECT id,kind,state,items,digest,created FROM plans WHERE id=?1",[pid],|r|Ok(json!({"id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"state":r.get::<_,String>(2)?,"items":serde_json::from_str::<Value>(&r.get::<_,String>(3)?).unwrap_or(json!([])),"digest":r.get::<_,String>(4)?,"created":r.get::<_,i64>(5)?})))?;
 let mut s=c.prepare("SELECT id,state,error FROM operations WHERE plan_id=?1")?;let ops=s.query_map([pid],|r|Ok(json!({"id":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"error":r.get::<_,String>(2)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;p["operations"]=json!(ops);
 p["has_origin"]=json!(c.query_row("SELECT EXISTS(SELECT 1 FROM plan_origins WHERE plan_id=?1)",[pid],|r|r.get::<_,bool>(0))?);
 p["undo_of"]=c.query_row("SELECT original_plan_id FROM plan_reversals WHERE plan_id=?1",[pid],|r|r.get::<_,String>(0)).optional()?.map_or(Value::Null,Value::String);
 p["reversed_by"]=c.query_row("SELECT r.plan_id FROM plan_reversals r JOIN plans p ON p.id=r.plan_id WHERE r.original_plan_id=?1 AND p.state='completed' ORDER BY p.rowid DESC LIMIT 1",[pid],|r|r.get::<_,String>(0)).optional()?.map_or(Value::Null,Value::String);Ok(p)
}
async fn list_plans(State(a):State<App>,h:HeaderMap,Query(q):Query<PlanQuery>)->Result<Value>{auth(&a,&h)?;let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT id FROM plans WHERE (?1 IS NULL OR rowid<(SELECT rowid FROM plans WHERE id=?1)) ORDER BY rowid DESC LIMIT 100")?;let ids=s.query_map([q.before],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;let rows=ids.iter().map(|id|plan_details(&c,id)).collect::<std::result::Result<Vec<_>,_>>()?;Ok(Json(json!(rows)))}
async fn get_plan(State(a):State<App>,h:HeaderMap,Path(pid):Path<String>)->Result<Value>{auth(&a,&h)?;Ok(Json(plan_details(&a.db.lock().unwrap(),&pid)?))}

async fn list_quarantine(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;let c=a.db.lock().unwrap();let mut s=c.prepare("SELECT id,original,current,state FROM quarantine ORDER BY rowid DESC")?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"original":r.get::<_,String>(1)?,"current":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;Ok(Json(json!(rows)))}
async fn start_acquire(State(a):State<App>,h:HeaderMap,Json(v):Json<Value>)->Result<Value>{auth(&a,&h)?;let rid=text(&v,"resource_id")?;let destination=text(&v,"destination")?;let jid={let _lock=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;acquire::create_selected(&a.db,&rid,&destination,v["extract"].as_bool().unwrap_or(false),Some(&serde_json::from_value::<acquire::Selection>(v.clone()).map_err(|_|ApiError("Review and select the files before starting a transfer".into()))?))?};let db=a.db.clone();let run=jid.clone();let password=v["password"].as_str().map(str::to_owned);tokio::task::spawn_blocking(move||acquire::run(db,run,password));Ok(Json(json!({"id":jid})))}
async fn acquire_preview(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{auth(&a,&h)?;Ok(Json(acquire::preview(&a.db,&id)?))}
async fn archive_capabilities(State(a):State<App>,h:HeaderMap)->Result<Value>{auth(&a,&h)?;Ok(Json(acquire::capability()))}







async fn watch_status(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{auth(&a,&h)?;let c=a.db.lock().unwrap();Ok(Json(watching::status(&c,&id)?))}
async fn configure_watch(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<watching::Edit>)->Result<Value>{auth(&a,&h)?;let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(watching::configure(&a.db,&id,v)?))}

async fn resource_members(State(a):State<App>,h:HeaderMap,Path(id):Path<String>)->Result<Value>{auth(&a,&h)?;Ok(Json(membership::members(&a.db,&id)?))}
async fn preview_regroup(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<membership::Selection>)->Result<Value>{auth(&a,&h)?;work_catalog::read_snapshot(a,move|c|membership::snapshot(c,&id,&v).map_err(ApiError)).await}
async fn preview_regroup_page(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<membership::PreviewPageRequest>)->Result<Value>{auth(&a,&h)?;work_catalog::read_snapshot(a,move|c|membership::preview_page(c,&id,&v).map_err(ApiError)).await}
async fn regroup_resource(State(a):State<App>,h:HeaderMap,Path(id):Path<String>,Json(v):Json<membership::Confirmation>)->Result<Value>{auth(&a,&h)?;let _gate=a.mutation_lock.lock().map_err(|e|ApiError(e.to_string()))?;Ok(Json(membership::apply(&a.db,&id,&v.selection,&v.digest)?))}

#[derive(serde::Deserialize)]struct EditionPreference{revision:i64,#[serde(deserialize_with="explicit_preference")]release_id:Option<String>}
fn explicit_preference<'de,D:serde::Deserializer<'de>>(d:D)->std::result::Result<Option<String>,D::Error>{<Option<String> as serde::Deserialize>::deserialize(d)}
async fn prefer_edition(State(a):State<App>,h:HeaderMap,Path(wid):Path<String>,Json(v):Json<EditionPreference>)->Result<Value>{auth(&a,&h)?;Ok(Json(editions::prefer(&a.db,&wid,v.revision,v.release_id.as_deref())?))}













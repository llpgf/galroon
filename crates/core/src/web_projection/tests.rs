use super::*;
use crate::{db,Transport};
use axum::{body::to_bytes,http::Request};
use rusqlite::params;
use sha2::{Digest,Sha256};
use tower::ServiceExt;
const MARKER:&str="ZZ-SECRET-ROOT-7f3a";
pub(crate) fn fixture()->(tempfile::TempDir,App,String,String){
 let t=tempfile::tempdir().unwrap();let source=t.path().join(MARKER);std::fs::create_dir(&source).unwrap();
 let mut a=crate::initialize(t.path().join("state")).unwrap();a.network=Some(crate::network::NetworkSettings::for_test("192.168.50.154".parse().unwrap(),14830));
 let web="w".repeat(72);let desktop="d".repeat(72);let cookie=format!("galroon_sid_{}={web}",&hex::encode(Sha256::digest(a.state_dir.to_string_lossy().as_bytes()))[..16]);
 {let c=a.db.lock().unwrap();
 for (role,token) in [("web",&web),("desktop",&desktop)]{c.execute("INSERT INTO sessions(token_hash,role,created,id,name,expires) VALUES(?1,?2,0,?2,?2,?3)",params![hex::encode(Sha256::digest(token.as_bytes())),role,db::now()+3600]).unwrap();}
 c.execute("INSERT INTO roots(id,path,label) VALUES('root',?1,'Source'),('unc',?2,'UNC source')",params![source.to_string_lossy(),format!(r"\\server\{MARKER}\share")]).unwrap();
 c.execute_batch("INSERT INTO works(id,title,original_title,vndb_id,cover) VALUES('work','Game','Game','v1','https://t.vndb.org/cv/01/123.jpg'); INSERT INTO releases(id,label) VALUES('edition','Edition'); INSERT INTO work_releases VALUES('work','edition'); INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('resource','root','Game','Game','directory','work'); INSERT INTO resource_bindings VALUES('resource','work','edition','main'); INSERT INTO work_preferences VALUES('work','edition');
 INSERT INTO custom_tags(id,name,name_key) VALUES('tag','Tag','tag'); INSERT INTO custom_tag_works VALUES('tag','work'); INSERT INTO custom_tag_entities VALUES('tag','s1'); INSERT INTO curated_lists(id,name) VALUES('list','List'); INSERT INTO list_entries(id,list_id,work_key,title,position,added) VALUES('entry','list','local:work','Game',1,1);
 INSERT INTO relation_overrides VALUES('v1',1,'{}'); INSERT INTO relation_corrections VALUES('v1',1,'[]'); INSERT INTO entity_overrides VALUES('s1',1,'{}');").unwrap();
 c.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES('file','root',?1,'Game/game.bin',1,'1','bin','')",[source.join("Game/game.bin").to_string_lossy()]).unwrap();c.execute("INSERT INTO resource_files VALUES('resource','file')",[]).unwrap();
 c.execute("INSERT INTO jobs(id,kind,state,spec,current_path,message,created,updated) VALUES('job','scan','failed','{}',?1,?2,1,1)",params![source.join("Game/game.bin").to_string_lossy(),format!("Cannot scan {}",source.display())]).unwrap();
 c.execute("INSERT INTO match_items(job_id,resource_id,revision,state) VALUES('job','resource',1,'review')",[]).unwrap();
 c.execute("INSERT INTO plans(id,kind,state,items,digest,created) VALUES('plan','organize','preview',?1,'digest',1)",[json!([{"source":source.join("Game"),"target":t.path().join(MARKER).join("organize") }]).to_string()]).unwrap();
 c.execute("INSERT INTO quarantine(id,plan_id,original,current) VALUES('quarantine','plan',?1,?2)",params![source.join("Game/game.bin").to_string_lossy(),t.path().join(MARKER).join("quarantine/game.bin").to_string_lossy()]).unwrap();
 c.execute("INSERT INTO plan_locations VALUES('plan','resource',?1,'Game')",[source.to_string_lossy()]).unwrap();
 c.execute("INSERT INTO issues(seq,root_id,category,kind,reason,evidence_key,created,updated) VALUES(1,'root','source','scan_failed',?1,'fixture',1,1)",[format!("Cannot scan {}",source.display())]).unwrap();
 crate::watching::ensure(&c).unwrap();
 c.execute("UPDATE source_watch SET message=?1 WHERE root_id='root'",[format!("Cannot watch {}",source.display())]).unwrap();
 let definition=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"status","mode":"any","values":["backlog"]}}});c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('smart','Smart','',1,?1)",[definition.to_string()]).unwrap();
 c.execute("INSERT INTO smart_compute_state VALUES('smart',0,0,?1)",[format!("Computation failed: /unknown/{MARKER}/data")]).unwrap();
 crate::smart_snapshots::build(&c,"smart",&crate::smart_snapshots::Build{revision:1,request_id:"fixture".into()},&Default::default()).map_err(|e|e.0).unwrap();
 }
 for (kind,id,version) in [("work","v1",4),("person","s1",3),("character","c1",3),("company","p1",3)]{
 let entity=json!({"id":id,"name":"Name","title":"Game","developers":[],"staff":[],"va":[],"relations":[],"vns":[]});let mut value=json!({"fetched_at":db::now(),"works":[],"characters":[],"more":false});value[if kind=="work"{"vn"}else{kind}]=entity;
 crate::exploration::save(&a,&format!("exploration.v{version}.{kind}.{id}.1"),&value).unwrap();
 }
 let search=crate::discovery::Search{kind:"title".into(),value:"Game".into(),target:"works".into(),page:1,spoilers:false,revision:None};let(endpoint,body)=crate::discovery::request(&search).unwrap();crate::exploration::save(&a,&format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("{endpoint}:{body}").as_bytes()))),&json!({"results":[],"more":false,"page":1,"fetched_at":db::now()})).unwrap();
 crate::artwork_cache::Cache::open(&a.state_dir).unwrap().put("https://t.vndb.org/cv/01/123.jpg",b"\xff\xd8\xfffixture").unwrap();
 (t,a,desktop,cookie)
}
pub(crate) fn url(a:&App,route:&str)->String{
 let id=if route.starts_with("/api/roots/"){"root"}else if route.starts_with("/api/works/"){"work"}else if route.starts_with("/api/resources/"){"resource"}else if route.starts_with("/api/releases/"){"edition"}else if route.starts_with("/api/jobs/")||route.starts_with("/api/duplicate-checks/"){"job"}else if route.starts_with("/api/plans/"){"plan"}else if route.starts_with("/api/lists/"){"list"}else if route.starts_with("/api/smart-lists/"){"smart"}else if route.starts_with("/api/custom-tags/"){"tag"}else if route.starts_with("/api/vns/"){"v1"}else if route.starts_with("/api/characters/"){"c1"}else if route.starts_with("/api/companies/"){"p1"}else if route.starts_with("/api/issues/"){"1"}else{"s1"};
 let path=route.replace("{id}",id);if route=="/api/releases/{id}/membership"{let c=a.db.lock().unwrap();let revision:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();return format!("{path}?ids=%5B%22work%22%5D&revision=1&collection_revision={revision}&library_id={}",crate::context::library(&c).unwrap().id);}match route{"/api/discover"|"/api/discover/manual"=>format!("{path}?kind=title&value=Game&target=works"),"/api/releases/labels"=>format!("{path}?ids=%5B%22edition%22%5D"),"/api/works/lookup"=>format!("{path}?ids=%5B%22work%22%5D&local=true"),"/api/artwork"=>format!("{path}?url=https://t.vndb.org/cv/01/123.jpg"),_=>path}
}
pub(crate) async fn request(a:&App,transport:Transport,role:&str,credential:&str,method:&str,url:&str)->(StatusCode,Vec<u8>){
 let mut req=Request::builder().method(method).uri(url).header("Host",if transport==Transport::Lan{"192.168.50.154:14830"}else{"localhost"});
 req=if role=="web"{req.header("Cookie",credential)}else{req.header("Authorization",format!("Bearer {credential}"))};
 let response=crate::router_for(a.clone(),transport).oneshot(req.body(Body::empty()).unwrap()).await.unwrap();let status=response.status();let bytes=to_bytes(response.into_body(),32*1024*1024).await.unwrap().to_vec();(status,bytes)
}
#[test]fn classification_covers_the_actual_router(){
 // Read the actual .route registrations, including chained .get/.head methods. No second
 // hand-maintained list of readable endpoints can silently drift with the router.
 let source=include_str!("../lib.rs");let mut actual=std::collections::BTreeSet::new();let readable=regex::Regex::new(r"\b(get|head)\s*\(").unwrap();
 for registration in source.split(".route(").skip(1){let path=registration.split('"').nth(1).unwrap();let handler=registration.split(".route(").next().unwrap().split(".fallback_service").next().unwrap();if readable.is_match(handler){actual.insert(path);}}
 let listed=WEB_ROUTES.iter().map(|(path,_)|*path).collect::<std::collections::BTreeSet<_>>();assert_eq!(WEB_ROUTES.len(),listed.len());assert_eq!(actual,listed);
}
#[tokio::test]async fn every_route_is_path_safe_for_web_on_both_transports(){
 let(_t,a,desktop,cookie)=fixture();
 for transport in [Transport::Loopback,Transport::Lan]{for(route,access)in WEB_ROUTES{for method in ["GET","HEAD"]{
  let(status,body)=request(&a,transport,"web",&cookie,method,&url(&a,route)).await;let body=String::from_utf8_lossy(&body);assert!(!body.contains(MARKER),"{transport:?} {method} {route}: {body}");
  if *access==WebAccess::Deny&&method=="GET"{assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["error"],"Desktop access required");}
  assert_eq!(status,if *access==WebAccess::Deny{StatusCode::FORBIDDEN}else{StatusCode::OK},"{transport:?} {method} {route}: {body}");
 }}}
 for role in ["owner","desktop"]{let token=if role=="owner"{&a.token}else{&desktop};for path in ["/api/roots","/api/resources","/api/resources/page","/api/works/work/assets","/api/resources/resource/members","/api/jobs","/api/jobs/job","/api/jobs?paged=true","/api/plans","/api/plans/plan","/api/quarantine"]{
  let(status,body)=request(&a,Transport::Loopback,role,token,"GET",path).await;assert_eq!(status,200,"{role} {path}");assert!(String::from_utf8_lossy(&body).contains(MARKER),"{role} {path}");
 }}
 let(_,members)=request(&a,Transport::Loopback,"web",&cookie,"GET","/api/resources/resource/members").await;let members:Value=serde_json::from_slice(&members).unwrap();assert_eq!(members["files"][0]["path"],"Game/game.bin");
 let(_,jobs)=request(&a,Transport::Loopback,"web",&cookie,"GET","/api/jobs?paged=true").await;assert!(!String::from_utf8_lossy(&jobs).contains(MARKER));
 let(_,owner)=request(&a,Transport::Loopback,"owner",&a.token,"GET","/api/resources/page").await;let mut owner:Value=serde_json::from_slice(&owner).unwrap();for item in owner["items"].as_array_mut().unwrap(){assert!(item.as_object_mut().unwrap().remove("root_path").is_some());}
 let(_,web)=request(&a,Transport::Loopback,"web",&cookie,"GET","/api/resources/page").await;assert_eq!(serde_json::from_slice::<Value>(&web).unwrap(),owner);
}
#[tokio::test]async fn api_errors_and_nested_text_redact_unknown_unix_drive_and_unc_paths(){
 let(_t,a,_,cookie)=fixture();let owner=a.token.clone();
 let router=axum::Router::new().route("/api/jobs/{id}",axum::routing::get(||async{crate::ApiError(format!("Cannot open /unknown/{MARKER}/a file")).into_response()})).layer(axum::middleware::from_fn_with_state(a.clone(),crate::access::guard)).layer(axum::middleware::from_fn_with_state((a,Transport::Loopback),crate::access::transport_host));
 let response=router.clone().oneshot(Request::builder().uri("/api/jobs/job").header("Host","localhost").header("Cookie",cookie).body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),400);let bytes=to_bytes(response.into_body(),4096).await.unwrap();assert!(!String::from_utf8_lossy(&bytes).contains(MARKER));
 let full=router.oneshot(Request::builder().uri("/api/jobs/job").header("Host","localhost").header("Authorization",format!("Bearer {owner}")).body(Body::empty()).unwrap()).await.unwrap();let full=to_bytes(full.into_body(),4096).await.unwrap();assert!(String::from_utf8_lossy(&full).contains(MARKER));
 for path in [format!("/unknown/{MARKER}/file"),format!(r"X:\{MARKER}\a file"),format!(r"\\server\{MARKER}\a file")]{let mut value=json!({"message":format!("Failed: {path}"),"nested":[{"reason":path}],"metadata":{path.clone():"value"}});scrub(&mut value,&[]);assert!(!value.to_string().contains(MARKER));}
 let mut relative=json!(["Game/game.bin","Fate/stay night","https://t.vndb.org/cv/01/123.jpg"]);let before=relative.clone();scrub(&mut relative,&[]);assert_eq!(relative,before);
}

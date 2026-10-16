use super::*;
use super::remote::{Collection,Subject,User};
use std::sync::{Arc,Mutex};
use axum::{Router,routing::get,extract::{Path,Query},http::StatusCode,response::IntoResponse};
use std::collections::BTreeMap;
use tower::ServiceExt;

fn subject(id:i64)->Subject{Subject{id,name:format!("Game {id}"),name_cn:String::new(),images:json!({"large":"https://lain.bgm.tv/pic/cover/l/12/ab/12345_XYz.jpg"}),date:Some("2020-01-01".into()),summary:"A game".into(),kind:4}}
fn collection(id:i64,status:i64)->Collection{Collection{subject_id:id,subject_type:4,status,subject:subject(id)}}
fn fixture()->(tempfile::TempDir,App,Account){
    let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();
    let account=Account{user:User{id:9,username:"fixture".into(),nickname:"Fixture".into()},credential:"not-a-real-credential".into(),mode:"full".into(),automatic:false,initialized:false,last_sync:None,preview:"preview".into(),error:String::new()};
    store::save_account(&app.db.lock().unwrap(),&account).unwrap();(t,app,account)
}
fn link_fixture(a:&App,status:&str,base:Option<i64>){let c=a.db.lock().unwrap();c.execute("INSERT INTO works(id,title,original_title,status,notes) VALUES('w','Local','Original',?1,'Keep private notes')",[status]).unwrap();c.execute("INSERT INTO bangumi_links(work_id,subject_id,base_status) VALUES('w',1,?1)",[base]).unwrap();}
fn choice(a:&App,id:i64,action:&str)->Choice{let row=store::rows(&a.db.lock().unwrap(),"full").unwrap().into_iter().find(|r|r.subject_id==id).unwrap();Choice{subject_id:id,fingerprint:row.fingerprint,action:action.into(),work_id:None}}
fn status(a:&App)->String{a.db.lock().unwrap().query_row("SELECT status FROM works WHERE id='w'",[],|r|r.get(0)).unwrap()}
#[derive(Clone,Default)]struct Mock{items:Arc<Mutex<BTreeMap<i64,Collection>>>,writes:Arc<Mutex<Vec<Value>>>,fail:Arc<Mutex<Option<i64>>>,edit:Arc<Mutex<Option<crate::db::Db>>>}
impl Mock{fn new(rows:Vec<Collection>)->Self{Self{items:Arc::new(Mutex::new(rows.into_iter().map(|r|(r.subject_id,r)).collect())),..Default::default()}}}
async fn list(State(mock):State<Mock>,Path(user):Path<String>,Query(q):Query<BTreeMap<String,String>>)->Json<Value>{
    assert_eq!(user,"fixture");assert_eq!(q["subject_type"],"4");let offset=q["offset"].parse::<usize>().unwrap();let rows=mock.items.lock().unwrap();Json(json!({"total":rows.len(),"data":rows.values().skip(offset).take(100).collect::<Vec<_>>()}))
}
async fn remote_get(State(mock):State<Mock>,Path((_user,id)):Path<(String,i64)>)->axum::response::Response{
    match mock.items.lock().unwrap().get(&id){Some(row)=>Json(json!({"type":row.status})).into_response(),None=>StatusCode::NOT_FOUND.into_response()}
}
async fn remote_set(State(mock):State<Mock>,Path((_user,id)):Path<(String,i64)>,Json(body):Json<Value>)->StatusCode{
    if *mock.fail.lock().unwrap()==Some(id){return StatusCode::SERVICE_UNAVAILABLE;}
    assert!(body.get("comment").is_none()&&body.get("rate").is_none()&&body.get("tags").is_none());
    mock.writes.lock().unwrap().push(body.clone());mock.items.lock().unwrap().insert(id,collection(id,body["type"].as_i64().unwrap()));
    if let Some(db)=mock.edit.lock().unwrap().take(){db.lock().unwrap().execute("UPDATE works SET status='dropped',revision=revision+1 WHERE id='w'",[]).unwrap();}
    StatusCode::NO_CONTENT
}
async fn server(mock:Mock)->(remote::Client,tokio::task::JoinHandle<()>){
    let router=Router::new().route("/v0/subjects/{id}",get(|Path(id):Path<i64>|async move{Json(subject(id))})).route("/v0/users/{user}/collections",get(list))
        .route("/v0/users/{user}/collections/{id}",get(remote_get).patch(remote_set).post(remote_set)).with_state(mock);
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
    let task=tokio::spawn(async move{axum::serve(listener,router).await.unwrap();});(remote::Client::at("generated-test-token".into(),base).unwrap(),task)
}
#[test]fn three_way_status_comparison_handles_both_directions_and_conflicts(){
    assert_eq!(store::classify(2,Some(3),Some(3)),"push");
    assert_eq!(store::classify(3,Some(2),Some(3)),"pull");
    assert_eq!(store::classify(4,Some(2),Some(3)),"conflict");
    assert_eq!(store::classify(2,Some(2),Some(3)),"same");
    assert_eq!(store::classify(2,Some(3),None),"initial");
    assert_eq!(store::classify(2,None,Some(3)),"missing");
    for number in 1..=5{assert_eq!(store::remote_status(store::local_status(number).unwrap()).unwrap(),number);}
}
#[test]fn imported_card_has_status_and_artwork_without_fake_resources_and_is_idempotent(){
    let(_t,a,_)=fixture();let mut c=a.db.lock().unwrap();store::replace_remote(&c,&[collection(1,2)]).unwrap();
    let tx=c.transaction().unwrap();let id=store::import(&tx,1).unwrap();tx.commit().unwrap();
    let work=crate::work_catalog::one(&c,&id).unwrap().unwrap();assert_eq!(work["status"],"completed");assert_eq!(work["resources"],0);assert_eq!(work["bangumi_id"],1);assert!(work["cover"].as_str().unwrap().contains("lain.bgm.tv"));
    for table in ["resources","releases","files","jobs"]{assert_eq!(c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get::<_,i64>(0)).unwrap(),0);}
    assert_eq!(store::rows(&c,"full").unwrap()[0].kind,"same");
    let tx=c.transaction().unwrap();assert!(store::import(&tx,1).is_err());drop(tx);assert_eq!(crate::work_catalog::all(&c).unwrap().len(),1);
    let mut q=crate::collection_query::Query::default();q.filters.availability="no_resources".into();assert_eq!(crate::collection_catalog::page(&c,&BTreeMap::new(),&q,None).map_err(|e|e.0).unwrap()["filtered"],1);
}
#[test]fn matches_require_confirmation_and_existing_mode_keeps_unmatched_subjects(){
    let(_t,a,_)=fixture();let c=a.db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('w','Game 1','Game 1')").unwrap();
    store::replace_remote(&c,&[collection(1,3),collection(2,2)]).unwrap();let rows=store::rows(&c,"existing").unwrap();assert_eq!(rows[0].kind,"match");assert_eq!(rows[0].candidates[0].id,"w");assert_eq!(rows[1].kind,"skipped");
    assert!(automatic_choices(rows).is_empty());assert_eq!(crate::work_catalog::all(&c).unwrap().len(),1);
}
#[test]fn matches_ignore_punctuation_and_spacing_differences_but_not_sequels(){
    let(_t,a,_)=fixture();let c=a.db.lock().unwrap();
    c.execute_batch("INSERT INTO works(id,title,original_title) VALUES('saku','Sakura no Uta','サクラノ詩－櫻の森の上を舞う－'),('zan','Zannen','残念な俺達の青春事情。'),('ryu','Ryuuki','竜姫ぐーたらいふ'),('exact','x','Game 3'),('loose','y','Game-3')").unwrap();
    let named=|id:i64,name:&str|{let mut row=collection(id,2);row.subject.name=name.into();row};
    store::replace_remote(&c,&[named(1,"サクラノ詩 —櫻の森の上を舞う—"),named(2,"残念な俺達の青春事情"),named(4,"竜姫ぐーたらいふ２"),collection(3,2)]).unwrap();
    let rows=store::rows(&c,"full").unwrap();let row=|id:i64|rows.iter().find(|r|r.subject_id==id).unwrap();
    assert_eq!(row(1).kind,"match");assert_eq!(row(1).candidates[0].id,"saku");
    assert_eq!(row(2).kind,"match");assert_eq!(row(2).candidates[0].id,"zan");
    assert_eq!(row(4).kind,"import");
    assert_eq!(row(3).candidates.iter().map(|c|c.id.as_str()).collect::<Vec<_>>(),["exact","loose"]);
    // Only a sole exact or folded candidate is pre-selected for review.
    assert_eq!(row(1).suggested.as_deref(),Some("saku"));assert_eq!(row(2).suggested.as_deref(),Some("zan"));assert_eq!(row(3).suggested,None);assert_eq!(row(4).suggested,None);
    c.execute("INSERT INTO bangumi_ignored VALUES(1)",[]).unwrap();let ignored=store::rows(&c,"full").unwrap().into_iter().find(|r|r.subject_id==1).unwrap();assert_eq!((ignored.kind.as_str(),ignored.suggested),("ignored",None));
    assert!(automatic_choices(store::rows(&c,"full").unwrap()).iter().all(|choice|choice.action!="match"));
}
#[test]fn matches_tolerate_spacing_readings_and_same_day_subtitles_but_keep_sequels_apart(){
    let(_t,a,_)=fixture();let c=a.db.lock().unwrap();
    c.execute_batch("INSERT INTO works(id,title,original_title,released) VALUES
        ('blue','Memory Blue','Memory Blue','2023-02-24'),
        ('ore','Ore no Sugata','俺の姿が、透明に!? 不可視の薬と数奇な運命','2020-03-27'),
        ('ghost','Kyonyuu','巨乳幽霊さんに癒やされる社畜さんの性活！～えっちなサポートは私におまかせ！～','2022-09-30'),
        ('kaifuku','Kaifuku','奥さまの回復術','2022-05-27'),
        ('elder','Elder','年上彼女','2024-11-29')").unwrap();
    let dated=|id:i64,name:&str,date:&str|{let mut row=collection(id,2);row.subject.name=name.into();row.subject.date=Some(date.into());row};
    store::replace_remote(&c,&[dated(1,"MemoryBlue","2023-02-24"),dated(2,"俺の姿が、透明に！？ 不可視の薬（インビジブル）と数奇な運命","2020-03-27"),
        dated(3,"巨乳幽霊さんに癒やされる社畜さんの性活！","2022-09-30"),dated(4,"奥さまの回復術 純愛編","2022-05-28"),dated(5,"年上彼女２","2024-11-29")]).unwrap();
    let rows=store::rows(&c,"full").unwrap();let found=|id:i64|rows.iter().find(|r|r.subject_id==id).unwrap().candidates.iter().map(|c|c.id.clone()).collect::<Vec<_>>();
    assert_eq!(found(1),["blue"]);assert_eq!(found(2),["ore"]);assert_eq!(found(3),["ghost"]);
    assert!(rows.iter().all(|r|r.suggested.is_none()),"compact and same-day matches are never pre-selected");
    // A subtitle-only difference needs the same release day, and a trailing number is a sequel.
    assert!(found(4).is_empty());assert!(found(5).is_empty());
}
#[tokio::test]async fn real_sync_flow_pushes_local_edits_and_pulls_remote_edits(){
    let(_t,a,_)=fixture();link_fixture(&a,"completed",Some(3));let mock=Mock::new(vec![collection(1,3)]);let(client,task)=server(mock.clone()).await;
    sync_client(&a,&client).await.unwrap();assert_eq!(mock.items.lock().unwrap()[&1].status,2);assert_eq!(mock.writes.lock().unwrap()[0],json!({"type":2}));assert_eq!(status(&a),"completed");
    mock.items.lock().unwrap().get_mut(&1).unwrap().status=4;sync_client(&a,&client).await.unwrap();assert_eq!(status(&a),"on_hold");assert_eq!(mock.writes.lock().unwrap().len(),1);
    assert_eq!(a.db.lock().unwrap().query_row("SELECT notes FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),"Keep private notes");task.abort();
}
#[tokio::test]async fn conflicts_initial_states_and_remote_deletions_never_auto_overwrite(){
    let(_t,a,_)=fixture();link_fixture(&a,"completed",Some(3));let mock=Mock::new(vec![collection(1,4)]);let(client,task)=server(mock.clone()).await;
    sync_client(&a,&client).await.unwrap();assert_eq!(status(&a),"completed");assert!(mock.writes.lock().unwrap().is_empty());assert_eq!(state(&a).unwrap()["rows"][0]["kind"],"conflict");
    let input=choice(&a,1,"remote");let account=connected(&a).unwrap();apply_choice(&a,&account,&client,&input).await.unwrap();assert_eq!(status(&a),"on_hold");
    mock.items.lock().unwrap().clear();sync_client(&a,&client).await.unwrap();assert_eq!(status(&a),"on_hold");assert!(mock.writes.lock().unwrap().is_empty());assert_eq!(state(&a).unwrap()["rows"][0]["kind"],"missing");
    let input=choice(&a,1,"local");apply_choice(&a,&connected(&a).unwrap(),&client,&input).await.unwrap();assert_eq!(mock.writes.lock().unwrap()[0],json!({"type":4,"private":true}));task.abort();
}
#[tokio::test]async fn stale_previews_are_rejected_and_edits_during_upload_remain_pending(){
    let(_t,a,mut account)=fixture();link_fixture(&a,"completed",Some(3));let mock=Mock::new(vec![collection(1,3)]);let(client,task)=server(mock.clone()).await;fetch(&a,&mut account,&client).await.unwrap();
    let old=choice(&a,1,"local");a.db.lock().unwrap().execute("UPDATE works SET status='on_hold',revision=revision+1 WHERE id='w'",[]).unwrap();assert!(apply_choice(&a,&account,&client,&old).await.is_err());
    let new=choice(&a,1,"local");mock.items.lock().unwrap().get_mut(&1).unwrap().status=5;assert!(apply_choice(&a,&account,&client,&new).await.is_err());assert!(mock.writes.lock().unwrap().is_empty());
    mock.items.lock().unwrap().get_mut(&1).unwrap().status=3;*mock.edit.lock().unwrap()=Some(a.db.clone());apply_choice(&a,&account,&client,&new).await.unwrap();assert_eq!(status(&a),"dropped");assert_eq!(state(&a).unwrap()["rows"][0]["kind"],"push");assert_eq!(state(&a).unwrap()["rows"][0]["base"],4);task.abort();
}
#[tokio::test]async fn hidden_cards_stay_hidden_and_reappear_only_on_restore(){
    let(_t,a,_)=fixture();let id={let c=a.db.lock().unwrap();store::replace_remote(&c,&[collection(1,2)]).unwrap();store::import(&c,1).unwrap()};
    let mut headers=HeaderMap::new();headers.insert("authorization",format!("Bearer {}",a.token).parse().unwrap());
    let _ = hide(State(a.clone()),headers.clone(),Json(Hide{work_id:id.clone(),hidden:true})).await.map_err(|e|e.0).unwrap();
    {let c=a.db.lock().unwrap();assert!(crate::work_catalog::all(&c).unwrap().is_empty());assert_eq!(crate::collection_catalog::page(&c,&BTreeMap::new(),&Default::default(),None).map_err(|e|e.0).unwrap()["total"],0);}
    let mock=Mock::new(vec![collection(1,3)]);let(client,task)=server(mock.clone()).await;sync_client(&a,&client).await.unwrap();assert_eq!(a.db.lock().unwrap().query_row("SELECT count(*) FROM works",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert!(mock.writes.lock().unwrap().is_empty());
    let _ = hide(State(a.clone()),headers,Json(Hide{work_id:id,hidden:false})).await.map_err(|e|e.0).unwrap();sync_client(&a,&client).await.unwrap();assert_eq!(crate::work_catalog::all(&a.db.lock().unwrap()).unwrap()[0]["status"],"playing");task.abort();
}
#[tokio::test]async fn partial_failure_keeps_successful_baselines_and_unsent_edits(){
    let(_t,a,mut account)=fixture();link_fixture(&a,"completed",Some(3));
    {let c=a.db.lock().unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,status) VALUES('second','Second','Second','dropped');INSERT INTO bangumi_links(work_id,subject_id,base_status) VALUES('second',2,3);").unwrap();}
    let mock=Mock::new(vec![collection(1,3),collection(2,3)]);*mock.fail.lock().unwrap()=Some(2);let(client,task)=server(mock.clone()).await;fetch(&a,&mut account,&client).await.unwrap();
    let results=apply_choices(&a,&mut account,&client,&[choice(&a,1,"local"),choice(&a,2,"local")]).await.unwrap();assert_eq!(results[0]["ok"],true);assert_eq!(results[1]["ok"],false);assert_eq!(mock.items.lock().unwrap()[&1].status,2);assert_eq!(mock.items.lock().unwrap()[&2].status,3);assert!(!account.error.is_empty());
    *mock.fail.lock().unwrap()=None;sync_client(&a,&client).await.unwrap();assert_eq!(mock.items.lock().unwrap()[&2].status,5);task.abort();
}
#[tokio::test]async fn pagination_reads_all_game_statuses(){
    let mock=Mock::new((1..=105).map(|id|collection(id,id%5+1)).collect());let(client,task)=server(mock).await;let rows=client.collections("fixture").await.unwrap();assert_eq!(rows.len(),105);task.abort();
}
#[test]fn merging_later_resources_keeps_bangumi_identity_and_play_status(){
    let(_t,a,_)=fixture();let source={let c=a.db.lock().unwrap();store::replace_remote(&c,&[collection(1,2)]).unwrap();let id=store::import(&c,1).unwrap();c.execute_batch("INSERT INTO works(id,title,original_title,vndb_id) VALUES('local','Game','Game','v10')").unwrap();id};
    crate::grouping::merge(&a.db,&source,"local",1,1).unwrap();let c=a.db.lock().unwrap();let work=crate::work_catalog::one(&c,"local").unwrap().unwrap();assert_eq!(work["bangumi_id"],1);assert_eq!(work["status"],"completed");assert_eq!(work["vndb_id"],"v10");assert_eq!(store::rows(&c,"full").unwrap().len(),1);
}
#[test]fn backup_keeps_cards_but_cannot_reactivate_account_and_migration_preserves_library(){
    let(t,a,_)=fixture();{let c=a.db.lock().unwrap();store::replace_remote(&c,&[collection(1,2)]).unwrap();store::import(&c,1).unwrap();}
    let folder=crate::backup::export(&a.db,t.path()).unwrap();let saved=rusqlite::Connection::open(folder.join("collection.sqlite")).unwrap();assert!(store::account(&saved).unwrap().is_none());assert_eq!(crate::work_catalog::all(&saved).unwrap().len(),1);
    let raw=std::fs::read(folder.join("collection.sqlite")).unwrap();assert!(!raw.windows(b"not-a-real-credential".len()).any(|w|w==b"not-a-real-credential"));
    let path=t.path().join("upgrade.sqlite");{let db=crate::db::open(&path).unwrap();db.lock().unwrap().execute_batch("DROP TABLE bangumi_links;DROP TABLE bangumi_remote;DROP TABLE bangumi_ignored;PRAGMA user_version=48;INSERT INTO works(id,title,original_title) VALUES('old','Saved','Saved');").unwrap();}
    let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(crate::work_catalog::all(&c).unwrap().len(),1);assert!(store::rows(&c,"full").unwrap().is_empty());assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);
}
#[tokio::test]async fn status_does_not_expose_credential_and_unauthenticated_routes_are_denied(){
    let(_t,a,_)=fixture();assert!(!state(&a).unwrap().to_string().contains("not-a-real-credential"));
    let response=crate::router(a).oneshot(axum::http::Request::builder().uri("/api/bangumi").header("host","127.0.0.1").body(axum::body::Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),401);
}

#[tokio::test]async fn ignored_subjects_and_sync_baselines_survive_restart(){
    let(t,a,mut account)=fixture();link_fixture(&a,"completed",Some(3));let mock=Mock::new(vec![collection(1,3),collection(2,4)]);let(client,task)=server(mock.clone()).await;
    fetch(&a,&mut account,&client).await.unwrap();apply_choice(&a,&account,&client,&choice(&a,2,"skip")).await.unwrap();sync_client(&a,&client).await.unwrap();
    let path=a.state_dir.clone();drop(a);let a=crate::initialize(path).unwrap();sync_client(&a,&client).await.unwrap();
    assert_eq!(status(&a),"completed");assert_eq!(mock.writes.lock().unwrap().len(),1);assert_eq!(state(&a).unwrap()["rows"][1]["kind"],"ignored");assert_eq!(crate::work_catalog::all(&a.db.lock().unwrap()).unwrap().len(),1);task.abort();drop(t);
}

#[tokio::test]async fn upstream_auth_rate_limit_and_redirect_fail_without_exposing_tokens(){
    for(code,message)in [(401,"authorization failed"),(429,"rate limit"),(302,"failed (302)")]{
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
        let server=tokio::spawn(async move{axum::serve(listener,Router::new().route("/v0/me",get(move||async move{(StatusCode::from_u16(code).unwrap(),"generated upstream secret")}))).await.unwrap();});
        let client=remote::Client::at("generated-token-not-for-logs".into(),base).unwrap();let error=match client.me().await{Ok(_)=>panic!("request unexpectedly succeeded"),Err(error)=>error.0};assert!(error.contains(message),"{error}");assert!(!error.contains("generated"));server.abort();
    }
}

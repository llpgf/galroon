use super::*;
use crate::web_projection::tests::{fixture,request,url};
use crate::{Transport,web_projection::{WEB_ROUTES,WebAccess}};
#[test]fn schema_45_upgrade_is_backed_up_and_starts_at_zero(){
 let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");let db=crate::db::open(&path).unwrap();
 {let c=db.lock().unwrap();let triggers=c.prepare("SELECT name FROM sqlite_schema WHERE type='trigger' AND name LIKE 'catalog_changed_%'").unwrap().query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();for name in triggers{c.execute_batch(&format!("DROP TRIGGER {name};")).unwrap();}c.execute_batch("DROP TABLE catalog_change_state; INSERT INTO works(id,title,original_title) VALUES('old','Saved','Saved'); PRAGMA user_version=45; UPDATE settings SET value='45' WHERE key='schema_version';").unwrap();}drop(db);
 let db=crate::db::open(&path).unwrap();{let c=db.lock().unwrap();assert_eq!(seq(&c).unwrap(),0);assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);assert_eq!(c.query_row("SELECT title FROM works WHERE id='old'",[],|r|r.get::<_,String>(0)).unwrap(),"Saved");c.execute("UPDATE works SET title='Changed' WHERE id='old'",[]).unwrap();assert_eq!(seq(&c).unwrap(),1);}drop(db);
 let db=crate::db::open(&path).unwrap();assert_eq!(seq(&db.lock().unwrap()).unwrap(),1);
 let backups=std::fs::read_dir(t.path().join("migration-backups")).unwrap().collect::<Result<Vec<_>,_>>().unwrap();assert_eq!(backups.len(),1);let saved=Connection::open(backups[0].path()).unwrap();assert_eq!(saved.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),45);assert_eq!(saved.query_row("SELECT count(*) FROM sqlite_schema WHERE name='catalog_change_state'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}
#[test]fn visible_table_writes_bump_and_rollback_does_not_publish(){
 let(_t,a,_,_)=fixture();let c=a.db.lock().unwrap();
 let writes=[
 ("match_suggestions","INSERT INTO match_suggestions VALUES('suggestion','resource',1,'[{\"vndb_id\":\"v1\",\"title\":\"Game\"}]','Game','Review','mcp:test',1,'[]','open',1,1)"),
 ("works","UPDATE works SET notes='Edited',tags='[\"tag\"]'"),("releases","UPDATE releases SET label='Changed'"),("resources","UPDATE resources SET title='Changed'"),("files","UPDATE files SET availability='missing'"),
 ("resource_files","DELETE FROM resource_files"),("resource_bindings","DELETE FROM resource_bindings"),("work_preferences","DELETE FROM work_preferences"),
 ("work_releases","INSERT INTO works(id,title,original_title) VALUES('other','Other','Other'); INSERT INTO work_releases VALUES('other','edition')"),
 ("custom_tags","UPDATE custom_tags SET name='Changed'"),("custom_tag_works","DELETE FROM custom_tag_works"),("custom_tag_entities","DELETE FROM custom_tag_entities"),
 ("curated_lists","UPDATE curated_lists SET name='Changed'"),("list_entries","UPDATE list_entries SET notes='Changed'"),("smart_lists","UPDATE smart_lists SET definition='{}'"),
 ("relation_overrides","UPDATE relation_overrides SET revision=revision+1"),("relation_corrections","UPDATE relation_corrections SET revision=revision+1"),("entity_overrides","UPDATE entity_overrides SET revision=revision+1"),
 ("list_entry_variants","INSERT INTO list_entry_variants VALUES('variant','entry','{}')"),("operations","INSERT INTO operations VALUES('op','plan','source','target','pending','{}','')"),("plan_reversals","INSERT INTO plan_reversals VALUES('plan','plan')"),("job_summary_entries","INSERT INTO job_summary_entries VALUES('job','new_files','file','yes')"),
 ("roots","UPDATE roots SET label='Changed'"),("source_watch","UPDATE source_watch SET enabled=0"),("issues","UPDATE issues SET state='deferred'"),("plans","UPDATE plans SET state='approved'"),("plan_locations","UPDATE plan_locations SET relative_path='Changed'"),("quarantine","UPDATE quarantine SET state='restored'"),("match_items","UPDATE match_items SET state='matched'"),("jobs","UPDATE jobs SET message='Changed'"),("library_identity","UPDATE library_identity SET name='Changed',revision=revision+1"),
 ];
 for table in TABLES{assert!(writes.iter().any(|(covered,_)|covered==table),"Missing write test for {table}");}
 for(table,sql)in writes{let before=seq(&c).unwrap();c.execute_batch("SAVEPOINT change_test").unwrap();c.execute_batch(sql).unwrap_or_else(|e|panic!("{table}: {e}"));assert!(seq(&c).unwrap()>before,"{table}");c.execute_batch("ROLLBACK TO change_test; RELEASE change_test").unwrap();assert_eq!(seq(&c).unwrap(),before,"rollback {table}");}
 // A relationship-only insertion must work without an accompanying entity update.
 let before=seq(&c).unwrap();c.execute("INSERT INTO work_releases VALUES('work',(SELECT id FROM releases LIMIT 1)) ON CONFLICT DO NOTHING",[]).unwrap();assert_eq!(seq(&c).unwrap(),before);
 c.execute("INSERT INTO releases(id,label) VALUES('join-only','Join')",[]).unwrap();let before=seq(&c).unwrap();c.execute("INSERT INTO work_releases VALUES('work','join-only')",[]).unwrap();assert_eq!(seq(&c).unwrap(),before+1);
 let before=seq(&c).unwrap();c.execute("DELETE FROM work_releases WHERE release_id='join-only'",[]).unwrap();assert_eq!(seq(&c).unwrap(),before+1);
 let before=seq(&c).unwrap();c.execute("UPDATE works SET title=title",[]).unwrap();assert_eq!(seq(&c).unwrap(),before);
 for key in ["organization.managed_root","auto_match_enabled","auto_match_requested"]{
  let before=seq(&c).unwrap();c.execute("INSERT INTO settings(key,value) VALUES(?1,'fixture') ON CONFLICT(key) DO UPDATE SET value=excluded.value",[key]).unwrap();assert!(seq(&c).unwrap()>before,"{key}");let changed=seq(&c).unwrap();c.execute("UPDATE settings SET value=value WHERE key=?1",[key]).unwrap();assert_eq!(seq(&c).unwrap(),changed);
 }
 let before=seq(&c).unwrap();c.execute("UPDATE smart_compute_state SET error='Different failure' WHERE list_id='smart'",[]).unwrap();assert_eq!(seq(&c).unwrap(),before+1);let before=seq(&c).unwrap();c.execute("UPDATE smart_compute_state SET last_attempt=last_attempt+1,error=error",[]).unwrap();assert_eq!(seq(&c).unwrap(),before);

}
#[test]fn online_transitions_and_completed_smart_results_bump_once(){
 let(_t,a,_,_)=fixture();let c=a.db.lock().unwrap();let mut online=std::collections::BTreeMap::from([("root".to_string(),true)]);
 crate::smart_updates::observe_roots(&c,&online).unwrap();let before=seq(&c).unwrap();crate::smart_updates::observe_roots(&c,&online).unwrap();assert_eq!(seq(&c).unwrap(),before);
 online.insert("root".into(),false);crate::smart_updates::observe_roots(&c,&online).unwrap();assert_eq!(seq(&c).unwrap(),before+1);
 let before=seq(&c).unwrap();let input=crate::smart_snapshots::Build{revision:1,request_id:"computed".into()};crate::smart_snapshots::build(&c,"smart",&input,&online).map_err(|e|e.0).unwrap();assert_eq!(seq(&c).unwrap(),before+1);crate::smart_snapshots::build(&c,"smart",&input,&online).map_err(|e|e.0).unwrap();assert_eq!(seq(&c).unwrap(),before+1);
 let before=seq(&c).unwrap();let input=crate::smart_snapshots::Build{revision:1,request_id:"cancelled".into()};assert!(crate::smart_snapshots::build_cancellable(&c,"smart",&input,&online,&||true).is_err());assert_eq!(seq(&c).unwrap(),before);
}
#[tokio::test]async fn reads_cache_fills_leases_and_authentication_do_not_bump(){
 let(_t,a,desktop,cookie)=fixture();let before=seq(&a.db.lock().unwrap()).unwrap();
 for(route,access)in WEB_ROUTES{if *access!=WebAccess::Deny{let(status,body)=request(&a,Transport::Loopback,"web",&cookie,"GET",&url(&a,route)).await;assert_eq!(status,200,"{route}: {}",String::from_utf8_lossy(&body));assert_eq!(seq(&a.db.lock().unwrap()).unwrap(),before,"{route}");}}
 // GET smart results created a lease; GET artwork touched its separate on-disk cache.
 assert!(a.db.lock().unwrap().query_row("SELECT count(*) FROM smart_snapshot_cache",[],|r|r.get::<_,i64>(0)).unwrap()>0);
 crate::exploration::save(&a,"exploration.v4.work.v99.1",&json!({"vn":{"id":"v99"},"fetched_at":crate::db::now()})).unwrap();assert_eq!(seq(&a.db.lock().unwrap()).unwrap(),before);
 {let c=a.db.lock().unwrap();c.execute_batch("INSERT INTO access_attempts VALUES('fixture',0,1); UPDATE access_attempts SET count=count+1; UPDATE sessions SET last_used=last_used+1; INSERT INTO settings VALUES('discovery.fixture','{}'); INSERT INTO settings VALUES('diagnostic.fixture','{}');").unwrap();assert_eq!(seq(&c).unwrap(),before);}
 let before_issues=seq(&a.db.lock().unwrap()).unwrap();for path in ["/api/issues","/api/diagnostics","/api/artwork/cache","/api/timeline","/api/backups"]{let(status,_)=request(&a,Transport::Loopback,"owner",&a.token,"GET",path).await;assert_eq!(status,200,"{path}");assert_eq!(seq(&a.db.lock().unwrap()).unwrap(),before_issues,"{path}");}
 crate::issues::refresh(&a.db.lock().unwrap()).unwrap();let reconciled=seq(&a.db.lock().unwrap()).unwrap();crate::issues::refresh(&a.db.lock().unwrap()).unwrap();assert_eq!(seq(&a.db.lock().unwrap()).unwrap(),reconciled);
 let before=reconciled;
 for(role,token)in [("owner",&a.token),("desktop",&desktop),("web",&cookie)]{let(status,body)=request(&a,Transport::Loopback,role,token,"GET","/api/changes").await;assert_eq!(status,200);let value:Value=serde_json::from_slice(&body).unwrap();assert_eq!(value["seq"],before);assert_eq!(value["library_id"],crate::context::library(&a.db.lock().unwrap()).unwrap().id);}
}

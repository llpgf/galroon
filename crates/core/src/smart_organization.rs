//! Catalog observations only. These rules never inspect, move or acquire files.
use crate::smart_rules::KnownSet;
use rusqlite::{Connection, OptionalExtension};
use std::collections::BTreeSet;
type R<T> = Result<T, String>;
pub fn load(c: &Connection, work: &str) -> R<KnownSet> {
    let active: Option<String> = c.prepare_cached("SELECT value FROM settings WHERE key='organization.managed_root'").map_err(|e|e.to_string())?
        .query_row([],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    let mut q = c.prepare_cached("SELECT r.id,
        EXISTS(SELECT 1 FROM plan_locations l JOIN plans p ON p.id=l.plan_id WHERE l.resource_id=r.id AND p.kind IN ('organize','undo_organize') AND p.state='partial'),
        EXISTS(SELECT 1 FROM plan_locations l JOIN plans p ON p.id=l.plan_id WHERE l.resource_id=r.id AND p.kind IN ('organize','undo_organize') AND p.state IN ('approved','executing')),
        count(f.id),coalesce(sum(rt.path=?2),0),coalesce(sum(rt.role='managed'),0)
        FROM resource_bindings b JOIN resources r ON r.id=b.resource_id
        LEFT JOIN resource_files rf ON rf.resource_id=r.id LEFT JOIN files f ON f.id=rf.file_id LEFT JOIN roots rt ON rt.id=f.root_id
        WHERE b.work_id=?1 GROUP BY r.id ORDER BY r.id").map_err(|e|e.to_string())?;
    let rows=q.query_map(rusqlite::params![work,active],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?,r.get::<_,bool>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?)))
        .map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    let mut values=BTreeSet::new();let mut complete=!rows.is_empty();
    for (_,partial,pending,count,inside,legacy_managed) in rows {
        let state=if partial {Some("partial")} else if pending {Some("pending")}
            else if count==0 {None}
            else if active.is_none()&&legacy_managed>0 {None}
            else if inside==count {Some("organized")}
            else if inside==0 {Some("unorganized")}
            else {Some("partial")};
        if let Some(state)=state {values.insert(state.into());} else {complete=false;}
    }
    Ok(KnownSet{values,complete})
}

#[cfg(test)] mod tests {
    use super::*;
    use crate::smart_rules::{Field, Predicate, Rule, SetMode, Truth, evaluate_checked};
    fn fixture()->(tempfile::TempDir,crate::db::Db){
        let t=tempfile::tempdir().unwrap();let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();
        d.lock().unwrap().execute_batch("INSERT INTO roots(id,path,label,role) VALUES('s','/source','Source','source'),('m','/managed','Managed','managed'); INSERT INTO settings(key,value) VALUES('organization.managed_root','/managed');").unwrap();(t,d)
    }
    fn resource(c:&Connection,work:&str,rid:&str,roots:&[&str]){
        c.execute("INSERT OR IGNORE INTO works(id,title,original_title) VALUES(?1,?1,?1)",[work]).unwrap();
        c.execute("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES(?1,'s',?1,?1,'folder')",[rid]).unwrap();
        c.execute("INSERT INTO resource_bindings(resource_id,work_id,role) VALUES(?1,?2,'main')",rusqlite::params![rid,work]).unwrap();
        for(i,root)in roots.iter().enumerate(){let fid=format!("{rid}-{i}");c.execute("INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job) VALUES(?1,?2,?1,?1,1,'1','bin','generated')",rusqlite::params![fid,root]).unwrap();c.execute("INSERT INTO resource_files VALUES(?1,?2)",rusqlite::params![rid,fid]).unwrap();}
    }
    fn plan(c:&Connection,rid:&str,state:&str){
        c.execute("INSERT INTO plans(id,kind,state,items,digest,created) VALUES(?1,'organize',?2,'[]','fixture',1)",rusqlite::params![rid,state]).unwrap();
        c.execute("INSERT INTO plan_locations(plan_id,resource_id,root_path,relative_path) VALUES(?1,?1,'/managed',?1)",[rid]).unwrap();
    }
    fn states(c:&Connection,w:&str,expected:&[&str],complete:bool){let actual=load(c,w).unwrap();assert_eq!(actual.values,expected.iter().map(|s|s.to_string()).collect(),"{w}");assert_eq!(actual.complete,complete,"{w}");}
    #[test]fn linked_resource_states_keep_mixed_and_unknown_evidence_distinct(){
        let(_t,d)=fixture();let c=d.lock().unwrap();
        resource(&c,"source","source-r",&["s"]);resource(&c,"managed","managed-r",&["m"]);resource(&c,"mixed","mixed-r",&["s","m"]);
        resource(&c,"both","both-s",&["s"]);resource(&c,"both","both-m",&["m"]);resource(&c,"empty","empty-r",&[]);
        resource(&c,"pending","pending-r",&["s"]);plan(&c,"pending-r","approved");resource(&c,"partial","partial-r",&["s"]);plan(&c,"partial-r","partial");
        states(&c,"source",&["unorganized"],true);states(&c,"managed",&["organized"],true);states(&c,"mixed",&["partial"],true);states(&c,"both",&["organized","unorganized"],true);states(&c,"empty",&[],false);states(&c,"no-resources",&[],false);states(&c,"pending",&["pending"],true);states(&c,"partial",&["partial"],true);
        c.execute("UPDATE plans SET state='ready' WHERE id='pending-r'",[]).unwrap();states(&c,"pending",&["unorganized"],true);
        c.execute("UPDATE files SET availability='missing' WHERE root_id='m'",[]).unwrap();states(&c,"managed",&["organized"],true); // Location is independent of availability.
        c.execute("DELETE FROM settings WHERE key='organization.managed_root'",[]).unwrap();states(&c,"managed",&[],false);states(&c,"source",&["unorganized"],true);
        c.execute("INSERT INTO resource_bindings(resource_id,work_id,role) VALUES('managed-r','source','extra')",[]).unwrap();states(&c,"source",&["unorganized"],false);
        let mut facts=crate::smart_rules::WorkFacts::default();facts.fields.insert(Field::OrganizingState,load(&c,"source").unwrap());
        let rule=Rule::Field{predicate:Predicate{field:Field::OrganizingState,mode:SetMode::None,values:vec!["organized".into()],exclude:false,include_unknown:false}};
        assert_eq!(evaluate_checked(&rule,&facts).unwrap(),Truth::Unknown);
        let bad=Rule::Field{predicate:Predicate{field:Field::OrganizingState,mode:SetMode::Any,values:vec!["downloaded".into()],exclude:false,include_unknown:false}};assert!(evaluate_checked(&bad,&facts).is_err());
        assert_eq!(c.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]fn schema39_upgrade_installs_organization_invalidation_once(){
        let(t,d)=fixture();let before={let c=d.lock().unwrap();c.execute_batch("DROP TRIGGER smart_dirty_plans_UPDATE; DROP TRIGGER smart_dirty_managed_UPDATE; PRAGMA user_version=39;").unwrap();c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap()};drop(d);
        let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();let revision={let c=d.lock().unwrap();assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),crate::db::SCHEMA_VERSION);assert_eq!(c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name IN ('smart_dirty_plans_UPDATE','smart_dirty_managed_UPDATE')",[],|r|r.get::<_,i64>(0)).unwrap(),2);c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap()};assert!(revision>before);drop(d);
        let d=crate::db::open(&t.path().join("library.sqlite")).unwrap();assert_eq!(d.lock().unwrap().query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap(),revision);
    }
    #[test]fn plan_changes_recompute_snapshots_without_rewriting_old_pages_or_moving_files(){
        let(t,d)=fixture();{
            let mut c=d.lock().unwrap();resource(&c,"w","r",&["s"]);
            let definition=serde_json::json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"organizing_state","mode":"any","values":["pending","partial"]}}});
            c.execute("INSERT INTO smart_lists(id,name,description,revision,definition) VALUES('list','Needs completion','',1,?1)",[definition.to_string()]).unwrap();
            let stop=std::sync::atomic::AtomicBool::new(false);assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());
            assert_eq!(crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap()["entries"].as_array().unwrap().len(),0);
            plan(&c,"r","approved");assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());
            let first=crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap();assert_eq!(first["entries"][0]["work_key"],"local:w");
            c.execute("UPDATE plans SET state='partial' WHERE id='r'",[]).unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());
            c.execute("UPDATE plans SET state='completed' WHERE id='r'",[]).unwrap();assert!(crate::smart_updates::tick(&mut c,&Default::default(),&stop).unwrap());
            assert_eq!(crate::smart_snapshots::page(&c,"list",&Default::default()).ok().unwrap()["entries"].as_array().unwrap().len(),0);
            let old=crate::smart_snapshots::page(&c,"list",&crate::smart_snapshots::Page{snapshot:first["snapshot"].as_str().map(str::to_owned),after:None}).ok().unwrap();assert_eq!(old["entries"][0]["work_key"],"local:w");assert_eq!(old["newer_available"],true);
            let before:i64=c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get(0)).unwrap();c.execute("UPDATE settings SET value='/another-managed-root' WHERE key='organization.managed_root'",[]).unwrap();assert!(c.query_row("SELECT revision FROM smart_catalog_state",[],|r|r.get::<_,i64>(0)).unwrap()>before);
            assert_eq!(c.query_row("SELECT count(*) FROM operations",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM jobs",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        }
        let backup=crate::backup::export(&d,t.path()).unwrap();let restored=crate::backup::restore_new(&backup,&t.path().join("restored"),None).unwrap();let restored=crate::db::open(&restored.join("library.sqlite")).unwrap();let c=restored.lock().unwrap();assert_eq!(crate::smart_lists::read_one(&c,"list").ok().unwrap()["definition"]["rule"]["predicate"]["field"],"organizing_state");states(&c,"w",&["unorganized"],true);
    }
}

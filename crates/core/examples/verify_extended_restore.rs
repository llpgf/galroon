//! Combined generated catalog, actual separate installed Core, isolated restore.
use galroon_core::{backup,db,local_core::{self,Session},plans};
use serde_json::{json,Value};
use std::{collections::BTreeMap,fs,path::{Path,PathBuf},time::Duration};

async fn api(s:&Session,path:&str,body:Option<Value>)->Value {
    let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();
    let url=format!("{}/api{path}",s.url);
    let r=if let Some(b)=body {c.post(url).json(&b)} else {c.get(url)};
    let r=r.bearer_auth(&s.token).send().await.unwrap();let status=r.status();let body=r.text().await.unwrap();
    assert!(status.is_success(),"{path}: {status}: {body}");serde_json::from_str(&body).unwrap()
}
fn catalog(path:&Path)->BTreeMap<String,Vec<String>> {
    let c=rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    let mut q=c.prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name").unwrap();
    let names=q.query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    let mut out=BTreeMap::new();
    for name in names {
        if !["works","releases","work_releases","work_preferences","preference_history","curated_lists","list_entries","list_requests","list_entry_variants","custom_tags","custom_tag_works","custom_tag_entities","tag_requests","tag_undo","smart_lists","smart_requests","smart_snapshots","smart_snapshot_entries","smart_snapshot_requests"].contains(&name.as_str()) && !name.starts_with("relation_") && !name.starts_with("entity_override") {continue;}
        let mut q=c.prepare(&format!("SELECT * FROM \"{name}\"")).unwrap();let count=q.column_count();
        let mut rows=q.query_map([],|r| {let mut cells=Vec::new();for i in 0..count {cells.push(format!("{:?}",r.get_ref(i)?));}Ok(serde_json::to_string(&cells).unwrap())}).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();rows.sort();out.insert(name,rows);
    }out
}
async fn list_edit(s:&Session,rev:i64,action:Value)->Value {api(s,"/lists/manual",Some(json!({"revision":rev,"request_id":format!("manual-{rev}"),"edit":action}))).await}
#[tokio::main] async fn main() {
    let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();
    assert!(!output.exists());fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");let restored=output.join("restored");let backups=output.join("backups");fs::create_dir(&backups).unwrap();
    {let d=db::open(&state.join("library.sqlite")).unwrap();let c=d.lock().unwrap();
        for n in 1..=4 {c.execute("INSERT INTO works(id,title,original_title,vndb_id,notes,overrides) VALUES(?1,?2,?2,?3,?4,?5)",rusqlite::params![format!("w{n}"),format!("Generated {n}"),format!("v{n}"),format!("Private 日本語 note {n}"),json!({"title":format!("Generated {n}")}).to_string()]).unwrap();}
        c.execute_batch("INSERT INTO releases(id,label,languages,notes) VALUES('edition','Generated Japanese edition','[\"ja\"]','Private edition note'); INSERT INTO work_releases VALUES('w1','edition');").unwrap();
        let raw=json!({"fetched_at":db::now(),"vn":{"id":"v1","title":"Generated 1","developers":[],"staff":[],"va":[],"relations":[]},"characters":[],"more":false});
        c.execute("INSERT INTO settings VALUES('exploration.v4.work.v1.1',?1)",[raw.to_string()]).unwrap();
        c.execute("INSERT INTO settings VALUES('relationship.candidate.s2',?1)",[json!({"id":"s2","kind":"person","name":"Generated person","aliases":[{"aid":9,"name":"Generated alias"}],"fetched_at":db::now()}).to_string()]).unwrap();
    }
    let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();assert_ne!(first.pid,std::process::id());
    let rs=state.clone();let rr=restored.clone();let re=exe.clone();let dest=output.clone();
    let outcome=tokio::spawn(async move {
        let tag=api(&first,"/custom-tags",Some(json!({"name":"Private 日本語","work_ids":["w1"]}))).await;let tid=tag["id"].as_str().unwrap().to_owned();let tp=format!("/custom-tags/{tid}");
        let assigned=json!({"revision":1,"request_id":"batch-add","action":"add","work_ids":["w2"]});let batch=api(&first,&tp,Some(assigned.clone())).await;
        let etag=api(&first,"/custom-tags",Some(json!({"name":"Private people"}))).await;let eid=etag["id"].as_str().unwrap();
        api(&first,"/entity-tags/s2",Some(json!({"tag_id":eid,"revision":1,"assigned":true,"request_id":"entity-add"}))).await;
        let fields=json!({"revision":0,"request_id":"person-fields","fields":{"name":"Private person","description":"Private introduction 日本語"}});
        let field_receipt=api(&first,"/entities/s2/field-decisions",Some(fields.clone())).await;
        let edit=json!({"revision":0,"request_id":"correction-add","corrections":[{"id":"private-staff","replaces":null,"link":{"key":{"kind":"staff","id":"s2","aid":9,"role":"scenario","note":"Private credit"},"name":"Generated alias","character_name":null,"spoiler":0}}]});
        let preview=api(&first,"/vns/v1/relationship-corrections/preview",Some(json!({"edit":edit,"spoilers":true}))).await;
        let correction_body=json!({"edit":edit,"spoilers":true,"preview_digest":preview["preview_digest"]});let correction_receipt=api(&first,"/vns/v1/relationship-corrections",Some(correction_body.clone())).await;
        api(&first,"/vns/v1/relationship-decisions",Some(json!({"revision":0,"request_id":"broad-hide","hidden":{"people":["s2"]}}))).await;
        api(&first,"/works/w1/preferred-edition",Some(json!({"revision":1,"release_id":"edition"}))).await;
        list_edit(&first,0,json!({"action":"create","name":"Private combined list"})).await;
        list_edit(&first,1,json!({"action":"add","members":[{"work_key":"local:w1","title":"unused"},{"work_key":"local:w2","title":"unused"},{"work_key":"vndb:v99","title":"External generated reference"}]})).await;
        let members=api(&first,"/lists/manual",None).await;let entry=members["entries"][0]["id"].clone();
        list_edit(&first,2,json!({"action":"annotate","entry_id":entry,"notes":"Private list 日本語","preferred_release_id":"edition"})).await;
        list_edit(&first,3,json!({"action":"move","entry_id":entry,"position":3})).await;
        let definition=json!({"schema_version":1,"scope":"collection","sort":"title","descending":false,"rule":{"kind":"field","predicate":{"field":"personal_tags","mode":"any","values":[tid]}}});
        api(&first,"/smart-lists/private",Some(json!({"revision":0,"request_id":"smart-create","edit":{"action":"save","name":"Private tagged","description":"Combined restore","pinned":true,"definition":definition}}))).await;
        let computed=api(&first,"/smart-lists/private/compute",Some(json!({"revision":1,"request_id":"smart-compute"}))).await;let snapshot=computed["snapshot"].as_str().unwrap().to_owned();
        let result_path=format!("/smart-lists/private/results?snapshot={snapshot}");let pinned=api(&first,&result_path,None).await;assert_eq!(pinned["total"],2);
        assert_eq!(pinned["entries"].as_array().unwrap().len(),2);
        let dependent=json!({"schema_version":1,"scope":"collection","sort":"title","rule":{"kind":"field","predicate":{"field":"related_personal_tags","mode":"any","values":[eid]}}});
        api(&first,"/smart-lists/dependency",Some(json!({"revision":0,"request_id":"dependent-create","edit":{"action":"save","name":"Related private people","description":"","pinned":false,"definition":dependent}}))).await;
        let tags=api(&first,"/custom-tags",None).await;let impact=tags.as_array().unwrap().iter().find(|t|t["id"]==eid).unwrap();assert_eq!(impact["smart_list_count"],1);
        api(&first,&format!("/custom-tags/{eid}"),Some(json!({"revision":2,"request_id":"delete-entity-tag","action":"delete","impact_revision":impact["impact_revision"]}))).await;
        assert_eq!(api(&first,"/smart-lists/dependency",None).await["needs_repair"],true);
        list_edit(&first,4,json!({"action":"delete"})).await;
        let before= catalog(&rs.join("library.sqlite"));
        for table in ["custom_tag_entities","tag_undo","list_entries","preference_history","relation_correction_history","relation_override_history","entity_override_history","smart_snapshot_entries"] {assert!(!before[table].is_empty(),"Empty coverage: {table}");}
        let saved=api(&first,"/backups",Some(json!({"destination":backups}))).await;let bp=PathBuf::from(saved["path"].as_str().unwrap());
        let manifest=backup::inspect(&bp).unwrap();assert_eq!(catalog(&bp.join("collection.sqlite")),before);
        {let c=rusqlite::Connection::open_with_flags(bp.join("collection.sqlite"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key LIKE 'exploration.%' OR key LIKE 'relationship.candidate.%'",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
        // Mutate the original after export; restore must recover the reviewed snapshot.
        list_edit(&first,5,json!({"action":"restore"})).await;
        api(&first,&tp,Some(json!({"revision":2,"request_id":"original-remove","action":"remove","work_ids":["w1","w2"]}))).await;
        local_core::request(&rs,&re,"stop").await.unwrap();
        backup::restore_new(&bp,&rr,Some(&manifest.sha256)).unwrap();assert_eq!(catalog(&rr.join("library.sqlite")),before);
        let second=local_core::connect_or_start(rr.clone(),re.clone()).await.unwrap();assert_eq!(second.library_id,first.library_id);
        assert_eq!(api(&second,&result_path,None).await["entries"],pinned["entries"]);assert_eq!(api(&second,&result_path,None).await["total"],2);
        assert_eq!(api(&second,&tp,Some(assigned)).await,batch);
        assert_eq!(api(&second,"/entities/s2/field-decisions",Some(fields)).await,field_receipt);
        assert_eq!(api(&second,"/vns/v1/relationship-corrections",Some(correction_body)).await,correction_receipt);
        assert_eq!(api(&second,"/vns/v1/relationship-corrections?spoilers=true",None).await["source_unavailable"],true);
        assert_eq!(api(&second,"/smart-lists/dependency",None).await["needs_repair"],true);
        api(&second,&format!("/custom-tags/{eid}"),Some(json!({"revision":3,"request_id":"restore-entity-tag","action":"restore"}))).await;
        assert_eq!(api(&second,"/smart-lists/dependency",None).await["needs_repair"],false);
        assert_eq!(api(&second,"/entity-tags/s2",None).await[0]["id"],eid);
        assert_eq!(api(&second,"/lists/manual",None).await["deleted"],true);
        list_edit(&second,5,json!({"action":"restore"})).await;
        let restored_list=api(&second,"/lists/manual",None).await;assert_eq!(restored_list["total"],3);assert_eq!(restored_list["entries"][2]["notes"],"Private list 日本語");assert_eq!(restored_list["entries"][2]["preferred_release_id"],"edition");
        api(&second,&tp,Some(json!({"revision":2,"request_id":"restored-undo","action":"undo"}))).await;
        let tags=api(&second,"/custom-tags",None).await;assert_eq!(tags.as_array().unwrap().iter().find(|t|t["id"]==tid).unwrap()["work_ids"],json!(["w1"]));
        assert_eq!(api(&second,&result_path,None).await["total"],2);
        local_core::request(&rr,&re,"stop").await.unwrap();
        let original=local_core::connect_or_start(rs.clone(),re.clone()).await.unwrap();assert_eq!(api(&original,"/lists/manual",None).await["deleted"],false);
        let tags=api(&original,"/custom-tags",None).await;assert_eq!(tags.as_array().unwrap().iter().find(|t|t["id"]==tid).unwrap()["work_ids"],json!([]));local_core::request(&rs,&re,"stop").await.unwrap();
        let counts:BTreeMap<_,_>=before.iter().map(|(k,v)|(k.clone(),v.len())).collect();
        fs::write(dest.join("snapshot-rows.json"),serde_json::to_vec_pretty(&before).unwrap()).unwrap();
        json!({"schema":db::SCHEMA_VERSION,"separate_installed_core":true,"source_files":0,"generated_works":4,"external_reference_members":1,"exact_snapshot_table_counts":counts,"backup_sha256":manifest.sha256,"http_export":true,"isolated_local_restore":true,"cache_and_credentials_excluded":true,"receipts_replayed_after_restore":true,"deleted_list_restored_with_order_notes_preference":true,"deleted_tag_dependency_repaired_only_after_explicit_restore":true,"broad_hidden_and_exact_correction_history_preserved":true,"batch_tag_undo_after_restore":true,"pinned_snapshot_survives_later_changes":true,"original_post_backup_changes_preserved":true,"native_desktop_switch_back":false,"full_extended_acceptance":false})
    }).await;
    let _=local_core::request(&state,&exe,"stop").await;let _=local_core::request(&restored,&exe,"stop").await;
    let mut report=outcome.expect("Combined restore failed; cleanup attempted");report["core_sha256"]=json!(plans::hash(&exe).unwrap().1);
    fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}


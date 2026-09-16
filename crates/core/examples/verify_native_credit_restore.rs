//! Restore an already native-saved generated credit without repeating editor writes.
use galroon_core::{backup, local_core::{self, Session}, plans};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, time::Duration};

fn snapshot(path: &Path) -> BTreeMap<String, Vec<String>> {
    let c = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)).unwrap(), "ok");
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    let mut result = BTreeMap::new();
    for table in ["works", "releases", "work_releases", "work_preferences", "relation_corrections", "relation_correction_history", "relation_correction_requests", "relation_correction_targets", "relation_overrides", "relation_override_history", "relation_override_requests"] {
        let mut q = c.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let columns = q.column_count();
        let mut rows = q.query_map([], |r| Ok((0..columns).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join("\n"))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        rows.sort(); result.insert(table.into(), rows);
    }
    result
}
async fn api(s: &Session, path: &str, body: Option<Value>) -> Value {
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();
    let url = format!("{}/api{path}", s.url);
    let request = if let Some(body) = body { client.post(url).json(&body) } else { client.get(url) };
    request.bearer_auth(&s.token).send().await.unwrap().error_for_status().unwrap().json().await.unwrap()
}
async fn stop(state: &Path, exe: &Path) {
    local_core::request(state, exe, "stop").await.unwrap();
    let lock = fs::OpenOptions::new().read(true).write(true).open(state.join("core.lock")).unwrap();
    let until = tokio::time::Instant::now() + Duration::from_secs(15);
    while fs2::FileExt::try_lock_exclusive(&lock).is_err() {
        assert!(tokio::time::Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let state = PathBuf::from(&args[1]).canonicalize().unwrap();
    let exe = PathBuf::from(&args[2]).canonicalize().unwrap();
    let output = PathBuf::from(&args[3]);
    let allowed = PathBuf::from("test-output/mvp-goal").canonicalize().unwrap();
    assert!(state.starts_with(&allowed) && output.parent().unwrap().canonicalize().unwrap().starts_with(&allowed));
    assert!(!output.exists()); fs::create_dir(&output).unwrap();
    let output = output.canonicalize().unwrap(); let restored = output.join("restored");
    let backups = output.join("backups"); fs::create_dir(&backups).unwrap();
    let first = local_core::connect_or_start(state.clone(), exe.clone()).await.unwrap();
    let (s, e, r, o) = (state.clone(), exe.clone(), restored.clone(), output.clone());
    let outcome = tokio::spawn(async move {
        let before = snapshot(&s.join("library.sqlite"));
        let corrections = api(&first, "/vns/v1/relationship-corrections?spoilers=true", None).await;
        assert_eq!(corrections["revision"], 4);
        let history = api(&first, "/vns/v1/relationship-corrections/history?spoilers=true", None).await;
        let exported = api(&first, "/backups", Some(json!({"destination": backups}))).await;
        let folder = PathBuf::from(exported["path"].as_str().unwrap());
        let manifest = backup::inspect(&folder).unwrap();
        assert!(!manifest.includes_credentials && !manifest.includes_game_files);
        assert_eq!(snapshot(&folder.join("collection.sqlite")), before);
        stop(&s, &e).await;
        backup::restore_new(&folder, &r, Some(&manifest.sha256)).unwrap();
        assert_eq!(snapshot(&r.join("library.sqlite")), before);
        let second = local_core::connect_or_start(r.clone(), e.clone()).await.unwrap();
        assert_eq!(second.library_id, first.library_id);
        let recovered = api(&second, "/vns/v1/relationship-corrections?spoilers=true", None).await;
        assert_eq!(recovered["revision"], 4);
        assert_eq!(recovered["corrections"], corrections["corrections"]);
        assert_eq!(recovered["source_unavailable"], true);
        assert_eq!(api(&second, "/vns/v1/relationship-corrections/history?spoilers=true", None).await, history);
        stop(&r, &e).await;
        assert_eq!(snapshot(&r.join("library.sqlite")), before);
        let c = rusqlite::Connection::open_with_flags(r.join("library.sqlite"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        for query in ["SELECT count(*) FROM sessions", "SELECT count(*) FROM source_watch WHERE enabled=1"] {
            assert_eq!(c.query_row(query, [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        }
        let report = json!({"passed":true,"scope":"Native-saved generated staff credit, installed HTTP export and restored installed HTTP reads; restore_new helper is not native restore UI","exact_tables":before.keys().collect::<Vec<_>>(),"revision":4,"history_exact":true,"corrections_exact":true,"source_cache_absence_explicit":true,"watchers_disabled":true,"credentials_absent":true,"backup_sha256":manifest.sha256,"core_sha256":plans::hash(&e).unwrap().1,"original_pid":first.pid,"restored_pid":second.pid,"both_core_identity_stopped":true});
        fs::write(o.join("report.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap(); report
    }).await;
    if outcome.is_err() {
        let _ = local_core::request(&state, &exe, "stop").await;
        let _ = local_core::request(&restored, &exe, "stop").await;
    }
    println!("{}", outcome.expect("Restore verification failed; cleanup attempted"));
}

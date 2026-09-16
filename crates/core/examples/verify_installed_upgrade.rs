use galroon_core::local_core;
use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
async fn stop(state: &Path, exe: &Path) {
    local_core::request(state, exe, "stop").await.unwrap();
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(state.join("core.lock"))
        .unwrap();
    let until = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if fs2::FileExt::try_lock_exclusive(&f).is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
fn read(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    let output = PathBuf::from(&args[1]);
    assert!(!output.exists());
    fs::create_dir_all(&output).unwrap();
    let output = output.canonicalize().unwrap();
    let state = output.join("state");
    let old = PathBuf::from(&args[2]).canonicalize().unwrap();
    let new = PathBuf::from(&args[3]).canonicalize().unwrap();
    let old_schema: i64 = args.get(4).map(|v| v.parse().unwrap()).unwrap_or(15);
    let new_schema: i64 = args.get(5).map(|v| v.parse().unwrap()).unwrap_or(35);
    let initial = local_core::connect_or_start(state.clone(), old.clone())
        .await
        .unwrap();
    stop(&state, &old).await;
    let path = state.join("library.sqlite");
    let c = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        old_schema
    );
    c.execute("INSERT INTO works(id,title,original_title,vndb_id,description,developer,released,tags,status,favorite,notes,source_json,overrides,revision) VALUES('upgrade-work','Generated multilingual title','生成作品 テスト','v1','Generated biography','Generated studio','2020-02-03','[\"Generated tag\"]','playing',1,'Private generated notes','{\"fixture\":true}','{\"title\":true}',7)",[]).unwrap();
    c.execute(
        "INSERT INTO settings(key,value) VALUES('upgrade.fixture','retained setting')",
        [],
    )
    .unwrap();
    drop(c);
    let prior = read(&path);
    let saved:String=prior.query_row("SELECT json_array(id,title,original_title,vndb_id,description,developer,released,tags,status,favorite,notes,source_json,overrides,revision) FROM works WHERE id='upgrade-work'",[],|r|r.get(0)).unwrap();
    drop(prior);
    if let Some(installer) = args.get(6) {
        assert_eq!(
            old, new,
            "In-place installer mode requires the same executable path"
        );
        let destination = old.parent().unwrap().canonicalize().unwrap();
        let workspace = std::env::current_dir()
            .unwrap()
            .join("test-output")
            .canonicalize()
            .unwrap();
        assert!(
            destination.starts_with(workspace),
            "Installer destination must remain in generated test-output"
        );
        let destination = destination
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned();
        let exit = std::process::Command::new(PathBuf::from(installer).canonicalize().unwrap())
            .raw_arg(format!("/S /UPDATE /D={destination}"))
            .status()
            .unwrap();
        assert!(exit.success(), "Installer failed: {exit}");
    }
    let upgraded = local_core::connect_or_start(state.clone(), new.clone())
        .await
        .unwrap();
    let rs = state.clone();
    let re = new.clone();
    let installer_update = args.get(6).is_some();
    let outcome=tokio::spawn(async move{
  assert_eq!(initial.library_id,upgraded.library_id);assert_eq!(initial.device_id,upgraded.device_id);
  let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(15)).build().unwrap();
  let response=client.get(format!("{}/api/works",upgraded.url)).bearer_auth(&upgraded.token).send().await.unwrap().error_for_status().unwrap();assert!(response.headers()["content-security-policy"].to_str().unwrap().contains("img-src 'self' data: blob:"));let works:Value=response.json().await.unwrap();assert!(works.as_array().unwrap().iter().any(|v|v["id"]=="upgrade-work"&&v["notes"]=="Private generated notes"));
  let c=read(&rs.join("library.sqlite"));assert_eq!(c.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),new_schema);assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");let current:String=c.query_row("SELECT json_array(id,title,original_title,vndb_id,description,developer,released,tags,status,favorite,notes,source_json,overrides,revision) FROM works WHERE id='upgrade-work'",[],|r|r.get(0)).unwrap();assert_eq!(saved,current);assert_eq!(c.query_row("SELECT value FROM settings WHERE key='upgrade.fixture'",[],|r|r.get::<_,String>(0)).unwrap(),"retained setting");drop(c);
  let snapshots:Vec<_>=fs::read_dir(rs.join("migration-backups")).unwrap().map(|e|e.unwrap().path()).collect();assert_eq!(snapshots.len(),1);let backup=read(&snapshots[0]);assert_eq!(backup.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),old_schema);assert_eq!(backup.query_row("SELECT count(*) FROM sessions",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(backup.query_row("SELECT notes FROM works WHERE id='upgrade-work'",[],|r|r.get::<_,String>(0)).unwrap(),"Private generated notes");drop(backup);
  stop(&rs,&re).await;let restarted=local_core::connect_or_start(rs.clone(),re).await.unwrap();assert_eq!(restarted.library_id,upgraded.library_id);assert_eq!(restarted.device_id,upgraded.device_id);assert_ne!(restarted.instance_id,upgraded.instance_id);
  let c=read(&rs.join("library.sqlite"));assert_eq!(c.query_row("SELECT revision FROM works WHERE id='upgrade-work'",[],|r|r.get::<_,i64>(0)).unwrap(),7);assert_eq!(fs::read_dir(rs.join("migration-backups")).unwrap().filter(|e|e.as_ref().unwrap().path().extension().is_some_and(|v|v=="sqlite")).count(),1);
  json!({"old_schema":old_schema,"new_schema":new_schema,"installed_executables":true,"in_place_installer_update":installer_update,"same_library_and_device":true,"fields_exact":14,"migration_snapshot_schema":old_schema,"snapshot_sessions":0,"restart_preserved":true,"http_csp_blob":true,"source_files":0})
 }).await;
    let stopped = local_core::request(&state, &new, "stop").await;
    let report = outcome.expect("Upgrade verification failed; stop attempted");
    stopped.unwrap();
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}

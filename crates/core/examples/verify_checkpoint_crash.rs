//! Kill only this helper's generated child after a committed large edition save.
use std::{fs,path::PathBuf,process::Command,time::{Duration,Instant}};
use galroon_core::{db,editions};
use rusqlite::{Connection,OpenFlags};
use serde_json::json;
fn main() {
    let args=std::env::args().collect::<Vec<_>>();
    let out=PathBuf::from(&args[1]);
    if args.get(2).is_some_and(|s|s=="--child") {
        let db=db::open(&out.join("library.sqlite")).unwrap();
        let input=serde_json::from_value(json!({"label":"Committed before termination","revision":1,"work_ids":(0..1000).map(|i|format!("w{i}")).collect::<Vec<_>>() })).unwrap();
        editions::save(&db,Some("e"),input).unwrap();
        fs::write(out.join("committed"),b"ok").unwrap();
        loop { std::thread::park(); }
    }
    assert!(!out.exists(),"Use a fresh generated output directory");
    fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
    {
        let db=db::open(&out.join("library.sqlite")).unwrap();
        db.lock().unwrap().execute_batch("WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<999) INSERT INTO works(id,title,original_title) SELECT 'w'||i,'Generated '||i,'Generated '||i FROM n; INSERT INTO releases(id,label) VALUES('e','Before'); INSERT INTO work_releases SELECT id,'e' FROM works;").unwrap();
    }
    // Pin the pre-save snapshot so the worker cannot checkpoint the committed change.
    let reader=Connection::open_with_flags(out.join("library.sqlite"),OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    reader.execute_batch("BEGIN; SELECT * FROM releases;").unwrap();
    let mut command=Command::new(std::env::current_exe().unwrap());command.arg(&out).arg("--child");
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    let mut child=command.spawn().unwrap();let deadline=Instant::now()+Duration::from_secs(45);
    while !out.join("committed").exists() {
        if child.try_wait().unwrap().is_some(){panic!("Generated child exited before commit");}
        if Instant::now()>deadline {let _=child.kill();let _=child.wait();panic!("Generated child commit timed out");}
        std::thread::sleep(Duration::from_millis(20));
    }
    let wal_bytes=fs::metadata(out.join("library.sqlite-wal")).unwrap().len();assert!(wal_bytes>0);
    child.kill().unwrap();let terminated=child.wait().unwrap();assert!(!terminated.success());
    assert_eq!(reader.query_row("SELECT label FROM releases WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"Before");
    reader.execute_batch("COMMIT").unwrap();drop(reader);
    let recovered=db::open(&out.join("library.sqlite")).unwrap();let c=recovered.lock().unwrap();
    assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(c.query_row("SELECT label FROM releases WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"Committed before termination");
    assert_eq!(c.query_row("SELECT count(*) FROM works WHERE revision=2",[],|r|r.get::<_,i64>(0)).unwrap(),1000);
    assert_eq!(c.query_row("SELECT count(*) FROM work_releases",[],|r|r.get::<_,i64>(0)).unwrap(),1000);
    c.backup(rusqlite::MAIN_DB,out.join("backup.sqlite"),None).unwrap();
    let backup=Connection::open_with_flags(out.join("backup.sqlite"),OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(backup.query_row("SELECT count(*) FROM works WHERE revision=2",[],|r|r.get::<_,i64>(0)).unwrap(),1000);
    assert_eq!(backup.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
    let report=json!({"passed":true,"sqlite_version":rusqlite::version(),"generated_child_pid":child.id(),"wal_bytes_before_termination":wal_bytes,"works_preserved":1000,"work_revisions":2,"integrity":"ok","foreign_key_errors":0,"backup_integrity":"ok","scope":"actual child-process termination after production edition save; not full HTTP Core or native crash acceptance"});
    fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

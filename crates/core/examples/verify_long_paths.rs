//! Real filesystem lifecycle on generated Unicode paths beyond MAX_PATH.
use galroon_core::{acquire, backup, db, plans, scan, undo};
use rusqlite::params;
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}};
fn length(p: &Path) -> usize { p.to_string_lossy().encode_utf16().count() }
fn nested(base: &Path, label: &str) -> PathBuf {
    (0..3).fold(base.join(label), |p,i| p.join(format!("階層{i}-{}", "長い名前".repeat(16))))
}
fn execute(d: &db::Db, p: &Value) {
    let pid=p["id"].as_str().unwrap();
    plans::approve(d,pid,p["digest"].as_str().unwrap()).unwrap();
    assert_eq!(plans::execute(d,pid).unwrap()["state"],"completed");
}
fn main() {
    assert!(cfg!(windows), "This verifier records Windows filesystem evidence");
    let out=PathBuf::from(std::env::args().nth(1).expect("Fresh fixture directory required"));
    assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
    let source=out.join("source");let managed=nested(&out,"managed");let destination=nested(&out,"downloads");
    let folder=nested(&source,"Game");fs::create_dir_all(&folder).unwrap();fs::create_dir_all(&managed).unwrap();fs::create_dir_all(&destination).unwrap();
    let mut files=vec![folder.join("説明と同梱ファイル.txt"),folder.join("音声と背景データ.bin")];
    for (i,p) in files.iter().enumerate(){fs::write(p,format!("Generated long-path fixture {i}: 日本語、中文、é\n").repeat(100)).unwrap();assert!(length(p)>260);}
    let archive=folder.join("同梱アーカイブ.zip");
    let mut command=std::process::Command::new(acquire::helper().expect("Bundled 7-Zip required"));
    #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    assert!(command.args(["a","-tzip"]).arg(&archive).args(&files).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap().success());files.push(archive);
    let originals:Vec<_>=files.iter().map(|p|plans::hash(p).unwrap()).collect();
    let database=out.join("state/library.sqlite");let d=db::open(&database).unwrap();
    d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('source',?1,'Generated long paths')",[source.to_str().unwrap()]).unwrap();
    let job=scan::create(&d,scan::ScanSpec{root_id:"source".into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),job.clone());
    assert_eq!(d.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get::<_,String>(0)).unwrap(),"completed");
    let rid:String={let c=d.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM files",[],|r|r.get::<_,i64>(0)).unwrap(),3);c.execute("INSERT INTO works(id,title,original_title) VALUES('generated-work','Generated work','日本語の生成作品')",[]).unwrap();let rid:String=c.query_row("SELECT id FROM resources",[],|r|r.get(0)).unwrap();c.execute("UPDATE resources SET work_id='generated-work' WHERE id=?1",[&rid]).unwrap();rid};
    let plan=plans::organize(&d,&rid,managed.to_str().unwrap()).unwrap();assert_eq!(plan["items"].as_array().unwrap().len(),3);
    for (p,hash) in files.iter().zip(&originals){assert_eq!(&plans::hash(p).unwrap(),hash);}
    execute(&d,&plan);
    let mut max_target=0;
    for item in plan["items"].as_array().unwrap(){let p=Path::new(item["target"].as_str().unwrap());max_target=max_target.max(length(p));assert!(length(p)>260);assert_eq!(plans::hash(p).unwrap().1,item["sha256"].as_str().unwrap());assert!(!Path::new(item["source"].as_str().unwrap()).exists());}
    drop(d);let d=db::open(&database).unwrap();let reverse=undo::prepare(&d,plan["id"].as_str().unwrap()).unwrap();execute(&d,&reverse);
    for (p,hash) in files.iter().zip(&originals){assert_eq!(&plans::hash(p).unwrap(),hash);}
    let exported=backup::export(&d,&out).unwrap();let restored=backup::restore_new(&exported,&out.join("restored"),None).unwrap();let recovered=db::open(&restored.join("library.sqlite")).unwrap();
    assert_eq!(recovered.lock().unwrap().query_row("SELECT value FROM settings WHERE key='organization.managed_root'",[],|r|r.get::<_,String>(0)).unwrap(),managed.to_string_lossy());
    let job=acquire::create(&recovered,&rid,destination.to_str().unwrap(),true).unwrap();acquire::run(recovered.clone(),job.clone(),None);
    let(state,spec,message):(String,String,String)=recovered.lock().unwrap().query_row("SELECT state,spec,message FROM jobs WHERE id=?1",[&job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(state,"completed","{message}");let spec:acquire::Spec=serde_json::from_str(&spec).unwrap();
    let mut max_copy=0;for m in &spec.files {let p=Path::new(&spec.final_folder).join(&m.relative);max_copy=max_copy.max(length(&p));assert_eq!(plans::hash(&p).unwrap(),plans::hash(Path::new(&m.source)).unwrap());}
    let archive=spec.files.iter().find(|m|m.relative.ends_with(".zip")).unwrap();
    let extraction=Path::new(&spec.final_folder).join(format!("{}-extracted",archive.relative));
    let mut max_extracted=0;for source in files.iter().take(2){let extracted=extraction.join(source.file_name().unwrap());max_extracted=max_extracted.max(length(&extracted));assert_eq!(plans::hash(&extracted).unwrap(),plans::hash(source).unwrap());}
    // Simulate only the durable job replay, with an already published real folder.
    recovered.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",params![job]).unwrap();acquire::run(recovered.clone(),job.clone(),None);
    assert_eq!(recovered.lock().unwrap().query_row("SELECT state FROM jobs WHERE id=?1",[job],|r|r.get::<_,String>(0)).unwrap(),"completed");
    for (p,hash) in files.iter().zip(&originals){assert_eq!(&plans::hash(p).unwrap(),hash);}
    let report=json!({"platform":"Windows","generated_files":3,"zip_extraction":true,"extracted_max_utf16_units":max_extracted,"source_max_utf16_units":files.iter().map(|p|length(p)).max(),"organized_max_utf16_units":max_target,"copied_max_utf16_units":max_copy,"scan":true,"manual_fixture_binding_not_matching_evidence":true,"preview_did_not_move":true,"organization":true,"reopen_then_undo":true,"backup_restore":true,"copy_from_restored_catalog":true,"publication_replay":true,"original_hashes":originals,"sources_restored_and_unchanged":true,"scope":"Core library real filesystem; not installed UI or universal path/format support"});
    fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

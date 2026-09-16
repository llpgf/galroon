//! Windows long-path lifecycle driven through an actual separate Core executable.
//! All files are generated under a new output directory; no provider calls.
use galroon_core::{backup, db, local_core::{self, Session}, plans};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, time::Duration};

fn length(p: &Path) -> usize { p.to_string_lossy().encode_utf16().count() }
fn nested(base: &Path, label: &str) -> PathBuf {
    (0..3).fold(base.join(label), |p,i| p.join(format!("階層{i}-{}", "長い名前".repeat(16))))
}
async fn api(s: &Session, path: &str, body: Option<Value>) -> Value {
    let c=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(30)).build().unwrap();
    let url=format!("{}/api{path}",s.url);
    let req=if let Some(body)=body {c.post(url).json(&body)} else {c.get(url)};
    let r=req.bearer_auth(&s.token).send().await.unwrap();
    let status=r.status();let v:Value=r.json().await.unwrap();
    assert!(status.is_success(),"{path}: {status}: {v}");v
}
async fn wait_job(s:&Session,id:&str)->Value {
    let deadline=tokio::time::Instant::now()+Duration::from_secs(60);
    loop {
        let job=api(s,&format!("/jobs/{id}"),None).await;
        if job["state"]=="completed" {assert_eq!(job["errors"],0,"{job}");return job;}
        assert_ne!(job["state"],"failed","{job}");
        assert!(tokio::time::Instant::now()<deadline,"Job did not complete: {job}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn execute(s:&Session,p:&Value) {
    let id=p["id"].as_str().unwrap();
    api(s,&format!("/plans/{id}/approve"),Some(json!({"digest":p["digest"]}))).await;
    assert_eq!(api(s,&format!("/plans/{id}/execute"),Some(json!({}))).await["state"],"completed");
}
fn files_under(dir:&Path,files:&mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let p=entry.unwrap().path();let m=fs::symlink_metadata(&p).unwrap();
        assert!(!m.file_type().is_symlink());
        if m.is_dir() {files_under(&p,files)} else {assert!(m.is_file());files.push(p);}
    }
}
#[tokio::main] async fn main() {
    assert!(cfg!(windows));
    let args:Vec<_>=std::env::args().collect();
    let out=PathBuf::from(&args[1]);let exe=PathBuf::from(&args[2]).canonicalize().unwrap();
    assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
    let state=out.join("state");drop(db::open(&state.join("library.sqlite")).unwrap());
    let source=out.join("source");let folder=nested(&source,"Game");
    let managed=nested(&out,"managed");let downloads=nested(&out,"downloads");let backups=out.join("backups");
    for p in [&folder,&managed,&downloads,&backups] {fs::create_dir_all(p).unwrap();}
    let mut originals=vec![folder.join("説明と同梱ファイル.txt"),folder.join("音声と背景データ.bin")];
    for (i,p) in originals.iter().enumerate() {
        fs::write(p,format!("Generated HTTP long-path fixture {i}: 日本語、中文、é\n").repeat(100)).unwrap();
        assert!(length(p)>260);
    }
    let helper=exe.parent().unwrap().join("runtime/7z.exe");assert!(helper.is_file(),"Installed archive runtime required");
    let archive=folder.join("同梱アーカイブ.zip");
    let mut command=std::process::Command::new(&helper);
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    assert!(command.args(["a","-tzip"]).arg(&archive).args(&originals).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap().success());
    originals.push(archive);let hashes:Vec<_>=originals.iter().map(|p|plans::hash(p).unwrap()).collect();
    let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();
    assert_ne!(first.pid,std::process::id());let cleanup_state=state.clone();let cleanup_exe=exe.clone();let output=out.clone();
    let result=tokio::spawn(async move {
        let root=api(&first,"/roots",Some(json!({"path":source,"label":"Generated long paths"}))).await;
        let scan=api(&first,"/scans",Some(json!({"root_id":root["id"],"scope":"","exclude":[]}))).await;
        let scan=wait_job(&first,scan["id"].as_str().unwrap()).await;assert_eq!(scan["processed"],3);
        let resources=api(&first,"/resources",None).await;assert_eq!(resources.as_array().unwrap().len(),1);
        let resource=&resources[0];assert_eq!(resource["files"],3);let rid=resource["id"].as_str().unwrap();
        let work=api(&first,"/works",Some(json!({"title":"Generated long-path work","original_title":"日本語の生成作品"}))).await;
        api(&first,&format!("/resources/{rid}/bind"),Some(json!({"revision":resource["revision"],"work_id":work["id"],"release":"Generated edition"}))).await;
        let plan=api(&first,"/plans/organize",Some(json!({"resource_id":rid,"destination":managed}))).await;
        assert_eq!(plan["items"].as_array().unwrap().len(),3);
        for (p,h) in originals.iter().zip(&hashes) {assert_eq!(&plans::hash(p).unwrap(),h);}
        execute(&first,&plan).await;
        let mut max_move=0;
        for item in plan["items"].as_array().unwrap() {
            let target=Path::new(item["target"].as_str().unwrap());max_move=max_move.max(length(target));
            assert!(length(target)>260);assert_eq!(plans::hash(target).unwrap().1,item["sha256"].as_str().unwrap());
            assert!(!Path::new(item["source"].as_str().unwrap()).exists());
        }
        local_core::request(&state,&exe,"stop").await.unwrap();
        let second=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();
        assert_eq!(second.library_id,first.library_id);assert_ne!(second.instance_id,first.instance_id);
        let reverse=api(&second,&format!("/plans/{}/undo",plan["id"].as_str().unwrap()),Some(json!({}))).await;
        execute(&second,&reverse).await;
        for (p,h) in originals.iter().zip(&hashes) {assert_eq!(&plans::hash(p).unwrap(),h);}
        let preview=api(&second,&format!("/resources/{rid}/acquire-preview"),None).await;
        let copy=api(&second,"/acquire",Some(json!({"resource_id":rid,"destination":downloads,"extract":true,"manifest_digest":preview["manifest_digest"],"file_ids":preview["files"].as_array().unwrap().iter().map(|f|f["id"].clone()).collect::<Vec<_>>()}))).await;
        let job=wait_job(&second,copy["id"].as_str().unwrap()).await;
        let publications:Vec<_>=fs::read_dir(&downloads).unwrap().map(|e|e.unwrap().path()).filter(|p|!p.file_name().unwrap().to_string_lossy().starts_with('.')).collect();
        assert_eq!(publications.len(),1);let mut published=vec![];files_under(&publications[0],&mut published);assert_eq!(published.len(),5);
        let mut receipts=vec![];
        for (original,hash) in originals.iter().zip(&hashes) {
            let found:Vec<_>=published.iter().filter(|p|p.file_name()==original.file_name()).collect();
            assert_eq!(found.len(),if original.extension().unwrap()=="zip" {1}else{2});
            for p in found {assert!(length(p)>260);assert_eq!(&plans::hash(p).unwrap(),hash);receipts.push(json!({"path":p,"utf16_units":length(p),"sha256":hash.1}));}
            assert_eq!(&plans::hash(original).unwrap(),hash);
        }
        let timeline=api(&second,"/timeline",None).await;assert!(timeline["next"].is_null());
        let saved=api(&second,"/backups",Some(json!({"destination":backups}))).await;
        local_core::request(&state,&exe,"stop").await.unwrap();
        let restored=backup::restore_new(Path::new(saved["path"].as_str().unwrap()),&output.join("restored"),None).unwrap();
        let third=local_core::connect_or_start(restored.clone(),exe.clone()).await.unwrap();
        assert_eq!(api(&third,"/timeline",None).await,timeline);
        assert_eq!(api(&third,&format!("/jobs/{}",copy["id"].as_str().unwrap()),None).await,job);
        assert_eq!(api(&third,&format!("/plans/{}",reverse["id"].as_str().unwrap()),None).await["state"],"completed");
        local_core::request(&restored,&exe,"stop").await.unwrap();
        fs::write(output.join("timeline.json"),serde_json::to_vec_pretty(&timeline).unwrap()).unwrap();
        fs::write(output.join("publication-receipts.json"),serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
        json!({"schema":db::SCHEMA_VERSION,"core_sha256":plans::hash(&exe).unwrap().1,"archive_helper_sha256":plans::hash(&helper).unwrap().1,"generated_source_files":3,"published_files_including_extraction":5,"source_max_utf16":originals.iter().map(|p|length(p)).max(),"organized_max_utf16":max_move,"published_max_utf16":published.iter().map(|p|length(p)).max(),"http_scan_manual_bind_review_move_restart_undo_copy_extract":true,"source_hashes_unchanged":true,"audit_and_task_equal_after_http_backup_local_restore_and_actual_restored_core":true,"audit_events":timeline["items"].as_array().unwrap().len(),"binding":"Manual generated fixture, not matching accuracy","native_ui":false,"universal_long_path_support":false})
    }).await;
    let _=local_core::request(&cleanup_state,&cleanup_exe,"stop").await;
    let _=local_core::request(&out.join("restored"),&cleanup_exe,"stop").await;
    let report=result.expect("Acceptance failed; fixture Core shutdown attempted");
    fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

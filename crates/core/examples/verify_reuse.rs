//! Read-only reuse of generated encrypted volumes, including password retry and a missing last volume.
use galroon_core::{acquire,db,scan,plans};
use std::{path::{Path,PathBuf},fs,process::{Command,Stdio}};
fn index(d:&db::Db,path:&Path,root:&str)->String{
 d.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES(?1,?2,'Reuse fixture')",rusqlite::params![root,path.to_str().unwrap()]).unwrap();
 let j=scan::create(d,scan::ScanSpec{root_id:root.into(),scope:"".into(),exclude:vec![]}).unwrap();scan::run(d.clone(),j);
 d.lock().unwrap().query_row("SELECT id FROM resources WHERE root_id=?1",[root],|r|r.get(0)).unwrap()
}
fn state(d:&db::Db,j:&str)->(String,String,i64){d.lock().unwrap().query_row("SELECT state,message,bytes FROM jobs WHERE id=?1",[j],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap()}
fn main(){
 let out=PathBuf::from(std::env::args().nth(1).expect("Fresh generated fixture directory required"));assert!(!out.exists());fs::create_dir_all(&out).unwrap();let out=out.canonicalize().unwrap();
 for name in ["full","missing-last"]{fs::create_dir(out.join(name)).unwrap();}
 let input=out.join("fixture.bin");fs::write(&input,(0..512*1024).map(|i|(i%251)as u8).collect::<Vec<_>>()).unwrap();
 let mut cmd=Command::new(acquire::helper().unwrap());#[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
 // Fixed non-secret fixture password is only used to generate this synthetic archive.
 let status=cmd.args(["a","-t7z","-mx=0","-v128k","-pReuseFixtureOnly","-mhe=on"]).arg(out.join("full/story.7z")).arg(&input).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap();assert!(status.success());
 let mut parts=fs::read_dir(out.join("full")).unwrap().map(|e|e.unwrap().path()).collect::<Vec<_>>();parts.sort();assert!(parts.len()>=4);let original:Vec<_>=parts.iter().map(|p|plans::hash(p).unwrap()).collect();
 for p in parts.iter().take(parts.len()-1){fs::copy(p,out.join("missing-last").join(p.file_name().unwrap())).unwrap();}
 let d=db::open(&out.join("state/library.sqlite")).unwrap();let rid=index(&d,&out.join("full"),"full");let p=acquire::preview(&d,&rid).unwrap();let selection=acquire::Selection{file_ids:p["files"].as_array().unwrap().iter().map(|f|f["id"].as_str().unwrap().into()).collect(),manifest_digest:p["manifest_digest"].as_str().unwrap().into()};
 let job=acquire::create_selected(&d,&rid,out.join("full").to_str().unwrap(),false,Some(&selection)).unwrap();acquire::run(d.clone(),job.clone(),None);assert_eq!(state(&d,&job).0,"failed");
 d.lock().unwrap().execute("UPDATE jobs SET state='queued' WHERE id=?1",[&job]).unwrap();acquire::run(d.clone(),job.clone(),Some("ReuseFixtureOnly".into()));let(st,msg,bytes)=state(&d,&job);assert_eq!(st,"completed","{msg}");assert!(msg.contains("Using existing files"));assert_eq!(bytes,0);
 let spec:String=d.lock().unwrap().query_row("SELECT spec FROM jobs WHERE id=?1",[&job],|r|r.get(0)).unwrap();assert!(!spec.contains("ReuseFixtureOnly"));assert_eq!(fs::read_dir(out.join("full")).unwrap().count(),parts.len());assert!(!out.join("full").join(format!(".galroon-{job}")).exists());
 let missing=index(&d,&out.join("missing-last"),"missing");let bad=acquire::create(&d,&missing,out.join("missing-last").to_str().unwrap(),false).unwrap();acquire::run(d.clone(),bad.clone(),Some("ReuseFixtureOnly".into()));assert_eq!(state(&d,&bad).0,"failed");assert_eq!(fs::read_dir(out.join("missing-last")).unwrap().count(),parts.len()-1);
 for(p,hash)in parts.iter().zip(original){assert_eq!(plans::hash(p).unwrap(),hash);}
 let report=serde_json::json!({"volumes":parts.len(),"password_retry":true,"reuse_bytes_copied":bytes,"missing_last_volume_rejected":true,"no_staging_or_duplicate_files":true,"source_hashes_unchanged":true,"password_absent_from_job_spec":true});fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

//! Opt-in read-only inventory of a supplied source. Generated state stays in test-output.
use galroon_core::{db,scan};
use serde_json::json;
fn main(){
    let source=std::env::args().nth(1).expect("source path required");
    let out=std::env::args().nth(2).expect("output directory required");
    let root=std::path::Path::new(&source).canonicalize().unwrap();let out=std::path::Path::new(&out);
    std::fs::create_dir_all(out).unwrap();assert!(!out.canonicalize().unwrap().starts_with(&root),"State must not be inside input");
    let d=db::open(&out.join("scan.sqlite")).unwrap();d.lock().unwrap().execute("INSERT OR IGNORE INTO roots(id,path,label) VALUES('sample',?1,'User supplied samples')",[root.to_str().unwrap()]).unwrap();
    let j=scan::create(&d,scan::ScanSpec{root_id:"sample".into(),scope:"".into(),exclude:vec![]}).unwrap();let start=std::time::Instant::now();scan::run(d.clone(),j.clone());
    let c=d.lock().unwrap();let report=c.query_row("SELECT state,processed,bytes,errors FROM jobs WHERE id=?1",[j],|r|Ok(json!({"state":r.get::<_,String>(0)?,"files":r.get::<_,i64>(1)?,"bytes":r.get::<_,i64>(2)?,"errors":r.get::<_,i64>(3)?,"elapsed_ms":start.elapsed().as_millis(),"mode":"read-only metadata scan"}))).unwrap();
    std::fs::write(out.join("scan-report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{}",report);
}

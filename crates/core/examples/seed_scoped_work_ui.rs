//! Generated-only collection/detail fixture. No provider requests or original files.
use galroon_core::{access,db,initialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
fn main(){
 let state=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());assert!(!state.exists());let app=initialize(state.clone()).unwrap();access::setup(&app.db,"Generated scoped work passphrase",false).unwrap();
 let root=state.join("generated-source");std::fs::create_dir(&root).unwrap();for(name,bytes)in [("present.dat",b"abc".as_slice()),("unknown.dat",b"unknown".as_slice()),("missing.dat",b"no".as_slice())]{std::fs::write(root.join(name),bytes).unwrap();}
 let mut c=app.db.lock().unwrap();let tx=c.transaction().unwrap();
 for i in 0..125{tx.execute("INSERT INTO works(id,title,original_title,vndb_id,description,developer,released,tags,aliases,notes) VALUES(?1,?2,?3,?4,?5,'Generated Studio','2004-04-28','[\"Drama\"]','[]','Original generated notes')",rusqlite::params![format!("collection-{i:06}"),format!("Generated volume {i}"),format!("原題 {i}"),format!("v{}",i+1),"Generated detail description. ".repeat(100)]).unwrap();}
 tx.execute("INSERT INTO roots(id,path,label) VALUES('root',?1,'Generated shared source')",[root.canonicalize().unwrap().to_str().unwrap()]).unwrap();
 tx.execute_batch("INSERT INTO releases(id,label,languages,platforms,origin,notes) VALUES('shared-edition','Shared Japanese edition','[\"ja\"]','[\"Windows\"]','official','Generated shared edition note'),('alt-edition','Alternative edition','[\"en\"]','[]','',''),('unrelated-edition','Unrelated edition','[]','[]','','');
 INSERT INTO work_releases(work_id,release_id) VALUES('collection-000124','shared-edition'),('collection-000000','shared-edition'),('collection-000124','alt-edition'),('collection-000060','unrelated-edition');
 INSERT INTO work_preferences(work_id,preferred_release_id) VALUES('collection-000124','shared-edition');
 INSERT INTO resources(id,root_id,relative_path,title,kind,work_id) VALUES('shared','root','shared','Generated shared files','folder','collection-000000'),('unrelated','root','unrelated','Generated unrelated files','folder','collection-000060'),('empty','root','empty','Generated empty resource','folder','collection-000124');
 INSERT INTO files(id,root_id,path,relative_path,size,mtime,ext,seen_job,availability) VALUES('f1','root','present.dat','present.dat',3,'','dat','','present'),('f2','root','unknown.dat','unknown.dat',7,'','dat','','legacy-unknown'),('f3','root','missing.dat','missing.dat',2,'','dat','','missing');
 INSERT INTO resource_files(resource_id,file_id) VALUES('shared','f1'),('shared','f2'),('unrelated','f3');
 INSERT INTO resource_bindings(resource_id,work_id,release_id,role) VALUES('shared','collection-000124','shared-edition','patch'),('shared','collection-000000','shared-edition','main'),('unrelated','collection-000060','unrelated-edition','main'),('empty','collection-000124',NULL,'extra');").unwrap();
 // This UI fixture records synthetic availability and relative file rows; it is not a scan fixture.
 tx.execute("INSERT INTO source_watch(root_id,path,enabled,status,needs_scan) SELECT id,path,0,'disabled',0 FROM roots",[]).unwrap();
 let staff=json!([{"id":"s1","aid":7,"name":"Generated writer","role":"scenario"}]);
 let vn=|id:&str,title:&str|json!({"id":id,"title":title,"staff":staff,"va":[],"developers":[],"relations":[],"released":"2004-04-28"});
 let save=|key:String,value:Value|{tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![key,value.to_string()]).unwrap();};
 for (id,title)in [("v1","Provider first"),("v61","Provider middle"),("v125","Provider current"),("v900","Generated reference 900")]{let mut value=vn(id,title);if id=="v125"{value["relations"]=json!([{"id":"v1","title":"Provider first","relation":"pre"},{"id":"v900","title":"Generated reference 900","relation":"side"}]);}save(format!("exploration.v4.work.{id}.1"),json!({"vn":value,"characters":[],"more":false,"fetched_at":db::now()}));}
 for(page,works,more)in [(1,json!([vn("v1","Provider first"),vn("v61","Provider middle")]),true),(2,json!([vn("v125","Provider current"),vn("v900","Generated reference 900")]),false)]{save(format!("exploration.v3.person.s1.{page}"),json!({"person":{"id":"s1","name":"Generated writer","description":"Generated cached biography"},"works":works,"more":more,"page":page,"fetched_at":db::now()}));}
 for title in ["Generated volume 0","Generated volume 124"]{let body=json!({"filters":["search","=",title],"fields":"title,alttitle,released,image.url,image.sexual,image.violence,developers.id,developers.name","sort":"searchrank","reverse":false,"results":24,"page":1});let key=format!("discovery.v1.{}",hex::encode(Sha256::digest(format!("vn:{body}").as_bytes())));save(key,json!({"results":[vn("v1","Provider first"),vn("v125","Provider current"),vn("v900","Generated reference 900")],"more":false,"page":1,"fetched_at":db::now()}));}
 tx.commit().unwrap();println!("Generated 125 works, shared edition/resources and cached exploration pages; no Core started");
}

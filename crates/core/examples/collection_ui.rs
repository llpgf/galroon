//! Generated collection fixture for real Core/React pagination verification.
use galroon_core::{access,initialize};
use serde_json::json;
#[tokio::main] async fn main(){
    let dir=std::path::PathBuf::from(std::env::args_os().nth(1).expect("Fresh generated state directory"));
    let port:u16=std::env::args().nth(2).expect("Loopback port").parse().unwrap();
    let count:usize=std::env::args().nth(3).unwrap_or_else(||"125".into()).parse().unwrap();
    assert!(!dir.exists(),"Fixture state must be new");assert!(count<=100_000);
    let app=initialize(dir.clone()).unwrap();
    access::setup(&app.db,"Generated collection UI passphrase",false).unwrap();
    {let mut c=app.db.lock().unwrap();let tx=c.transaction().unwrap();
        for i in 0..count {tx.execute("INSERT INTO works(id,title,original_title,description,developer,released,tags,aliases,status,favorite) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",rusqlite::params![format!("collection-{i:06}"),format!("Generated volume {i}"),format!("原題 {i}"),"Generated description. ".repeat(100),if i%2==0 {"Generated Studio A"}else{"Generated Studio B"},format!("{}-04-28",2000+i%20),if i%2==0 {"[\"Drama\"]"}else{"[\"Comedy\"]"},json!([format!("ExactAlias{i:06}")]).to_string(),if i%2==0 {"backlog"}else{"completed"},i%3==0]).unwrap();}
        tx.execute_batch("INSERT INTO custom_tags(id,name,name_key) VALUES('collection-personal','Drama','drama'); INSERT INTO custom_tag_works SELECT 'collection-personal',id FROM works WHERE rowid%5=0; INSERT INTO releases(id,label,languages) VALUES('collection-ja','Generated Japanese','[\"ja\"]'); INSERT INTO work_releases SELECT id,'collection-ja' FROM works WHERE rowid%3=0;").unwrap();tx.commit().unwrap();
    }
    let listener=tokio::net::TcpListener::bind(("127.0.0.1",port)).await.unwrap();
    std::fs::write(dir.join("ui-session.json"),json!({"url":format!("http://127.0.0.1:{port}"),"token":app.token}).to_string()).unwrap();
    println!("Generated {count} works; loopback fixture ready on {port}");
    galroon_core::serve_listener(app,listener).await.unwrap();
}

//! Explicit, bounded read-only VNDB comparison; input contains names, never file contents.
use serde_json::{json,Value};
#[tokio::main] async fn main(){
    let args:Vec<String>=std::env::args().collect();
    let samples:Vec<Value>=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    assert!(samples.len()<=12);
    let client=reqwest::Client::new();let mut report=Vec::new();
    for sample in samples{
        let raw=sample["raw"].as_str().unwrap();
        let query=galroon_core::scan::clean_title(raw);
        let hints:Vec<String>=serde_json::from_value(sample["hints"].clone()).unwrap();
        let start=std::time::Instant::now();
        let old=client.post("https://api.vndb.org/kana/vn").timeout(std::time::Duration::from_secs(8)).json(&json!({"filters":["search","=",query],"fields":"title,alttitle","results":10})).send().await;
        let old=match old{Ok(r) if r.status().is_success()=>r.json::<Value>().await.unwrap(),Ok(r)=>json!({"error":r.status().as_u16()}),Err(e)=>json!({"error":e.to_string()})};
        tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
        let input=galroon_core::matching::prepare(&query,&hints).unwrap();
        let new=galroon_core::matching::search(input).await;
        let new=match new{Ok(v)=>v,Err(e)=>json!({"error":e})};
        println!("{}",json!({"sample":sample["name"],"old_count":old["results"].as_array().map(Vec::len),"new_count":new["results"].as_array().map(Vec::len),"first":new["results"][0]["title"],"reason":new["results"][0]["match"],"ms":start.elapsed().as_millis()}));
        report.push(json!({"sample":sample,"old_query":query,"old":old,"new":new}));
        std::fs::write(&args[2],serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
    }
}

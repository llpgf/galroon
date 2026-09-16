use galroon_core::{local_core,artwork_cache::Cache};use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::{Arc,atomic::{AtomicUsize,Ordering}},time::Duration};
#[tokio::main]async fn main(){
 use tokio::io::{AsyncReadExt,AsyncWriteExt};
 let args:Vec<_>=std::env::args().collect();let output=PathBuf::from(args.get(1).expect("Fresh output"));let exe=PathBuf::from(args.get(2).expect("Core executable")).canonicalize().unwrap();let prior=PathBuf::from(args.get(3).expect("Prior artwork-native evidence"));assert!(!output.exists());
 let source:Value=serde_json::from_str(&fs::read_to_string(prior.join("public-source.json")).unwrap()).unwrap();let image=source["results"][0]["image"]["url"].as_str().unwrap().to_owned();let bytes=Cache::open(&prior.join("state")).unwrap().get(&image).unwrap().unwrap().bytes;
 fs::create_dir_all(&output).unwrap();let output=output.canonicalize().unwrap();let state=output.join("state");fs::create_dir(&state).unwrap();let missing="https://t.vndb.org/cv/00/999999999.jpg";
 {let db=galroon_core::db::open(&state.join("library.sqlite")).unwrap();let c=db.lock().unwrap();for(key,url)in [("cached",image.as_str()),("missing",missing)]{c.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![format!("exploration.{key}"),json!({"image":{"url":url}}).to_string()]).unwrap();}}Cache::open(&state).unwrap().put(&image,&bytes).unwrap();
 let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let proxy_url=format!("http://{}",listener.local_addr().unwrap());let failures=Arc::new(AtomicUsize::new(0));let observed=failures.clone();let proxy=tokio::spawn(async move{loop{let(mut socket,_)=listener.accept().await.unwrap();let mut request=[0u8;4096];let size=socket.read(&mut request).await.unwrap();assert!(String::from_utf8_lossy(&request[..size]).starts_with("CONNECT t.vndb.org:443 "));observed.fetch_add(1,Ordering::SeqCst);let _=socket.write_all(b"HTTP/1.1 502 Offline fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;}});
 // Process-local only; the isolated child inherits a deliberately failing HTTPS proxy.
 std::env::set_var("HTTPS_PROXY",&proxy_url);std::env::set_var("https_proxy",&proxy_url);std::env::set_var("NO_PROXY","127.0.0.1,localhost");std::env::set_var("no_proxy","127.0.0.1,localhost");
 let first=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();let rs=state.clone();let re=exe.clone();
 let outcome=tokio::spawn(async move{
  let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(25)).build().unwrap();let endpoint=format!("{}/api/artwork",first.url);
  let cached=client.get(&endpoint).query(&[("url",&image)]).bearer_auth(&first.token).send().await.unwrap().error_for_status().unwrap().bytes().await.unwrap();assert_eq!(cached,bytes);assert_eq!(failures.load(Ordering::SeqCst),0);
  let failed=client.get(&endpoint).query(&[("url",missing)]).bearer_auth(&first.token).send().await.unwrap();assert_eq!(failed.status(),400);assert_eq!(failures.load(Ordering::SeqCst),1);
  local_core::request(&rs,&re,"stop").await.unwrap();let second=local_core::connect_or_start(rs,re).await.unwrap();let cached=client.get(format!("{}/api/artwork",second.url)).query(&[("url",&image)]).bearer_auth(&second.token).send().await.unwrap().error_for_status().unwrap().bytes().await.unwrap();assert_eq!(cached,bytes);assert_eq!(failures.load(Ordering::SeqCst),1);
  json!({"separate_core":true,"outbound_https_failure_proxy":true,"cached_exact_bytes_before_after_restart":true,"cached_requests_to_proxy":0,"missing_request_failed_at_proxy":1,"cached_bytes":bytes.len(),"system_network_changed":false,"source_files":0,"schema":galroon_core::db::SCHEMA_VERSION})
 }).await;let stopped=local_core::request(&state,&exe,"stop").await;proxy.abort();let report=outcome.expect("Verification failed; stop attempted");stopped.unwrap();fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

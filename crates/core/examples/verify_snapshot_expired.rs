//! Read-only acceptance against an already running fixture Core; no token output.
// Needs the Windows-only local Core; elsewhere only a stub is built so `cargo build --examples` works.
#[cfg(windows)] mod windows_only {
use galroon_core::local_core::{self,Session};
use serde_json::{json,Value};
use std::{path::Path,fs,time::Duration};
#[tokio::main] pub async fn main(){
 let a:Vec<_>=std::env::args().collect();
 let s:Session=serde_json::from_value(local_core::request(Path::new(&a[1]),Path::new(&a[2]),"session").await.unwrap()).unwrap();
 let client=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
 let r=client.get(format!("{}/api/smart-lists/{}/results",s.url,a[3])).query(&[("snapshot",&a[4])]).bearer_auth(&s.token).send().await.unwrap();
 let status=r.status().as_u16();let body:Value=r.json().await.unwrap();
 assert_eq!(status,400);assert!(body.to_string().contains("Snapshot expired"),"{body}");
 let report=json!({"status":status,"response":body,"list":a[3],"snapshot":a[4],"empty_success_not_returned":true});
 fs::write(&a[5],serde_json::to_vec_pretty(&report).unwrap()).unwrap();println!("{report}");
}

}
#[cfg(windows)] use windows_only::main;
#[cfg(not(windows))] fn main(){eprintln!("This example needs the Windows local Core.");}

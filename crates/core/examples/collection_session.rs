//! Start/stop a generated UI fixture through the production Core identity channel.
// Needs the Windows-only local Core; elsewhere only a stub is built so `cargo build --examples` works.
#[cfg(windows)] mod windows_only {
use galroon_core::{local_core,plans};
use serde_json::json;
use std::{fs,path::PathBuf,time::{Duration,Instant}};
#[tokio::main]pub async fn main(){
    let args=std::env::args().collect::<Vec<_>>();let state=PathBuf::from(&args[2]).canonicalize().unwrap();let exe=PathBuf::from(&args[3]).canonicalize().unwrap();
    assert!(state.join("library.sqlite").is_file());
    match args[1].as_str(){
        "start"=>{let session=local_core::connect_or_start(state.clone(),exe.clone()).await.unwrap();fs::write(state.join("ui-session.json"),serde_json::to_vec(&session).unwrap()).unwrap();println!("{}",json!({"url":session.url,"pid":session.pid,"instance_id":session.instance_id,"core_sha256":plans::hash(&exe).unwrap().1}));},
        "stop"=>{local_core::request(&state,&exe,"stop").await.unwrap();let lock=fs::OpenOptions::new().read(true).write(true).open(state.join("core.lock")).unwrap();let until=Instant::now()+Duration::from_secs(15);loop{if fs2::FileExt::try_lock_exclusive(&lock).is_ok(){break;}assert!(Instant::now()<until);tokio::time::sleep(Duration::from_millis(30)).await;}let private=state.join("ui-session.json");if private.is_file(){fs::remove_file(private).unwrap();}println!("Core identity stop acknowledged and lock released");},
        _=>panic!("Use start or stop"),
    }
}

}
#[cfg(windows)] use windows_only::main;
#[cfg(not(windows))] fn main(){eprintln!("This example needs the Windows local Core.");}

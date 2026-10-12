#![cfg_attr(windows,windows_subsystem="windows")]
#[cfg(windows)]
#[tokio::main]async fn main(){
 let mut args=std::env::args_os().skip(1);let dir=if args.next().as_deref()==Some(std::ffi::OsStr::new("--data-dir")){args.next().map(std::path::PathBuf::from)}else{None};
 let Some(dir)=dir else{eprintln!("Usage: galroon-core --data-dir <local folder>");std::process::exit(2)};
 let device_dir=if args.next().as_deref()==Some(std::ffi::OsStr::new("--device-dir")){args.next().map(std::path::PathBuf::from)}else{None}.unwrap_or_else(||dir.clone());
 let _=galroon_core::diagnostics::init(&dir);
 #[cfg(windows)]if let Err(error)=galroon_core::local_core::run_with_device(dir,device_dir).await{galroon_core::diagnostics::record("error","core.fatal",&error,None);eprintln!("{error}");std::process::exit(1);}
}


#[cfg(unix)]
#[tokio::main]async fn main(){
 let command=match galroon_core::owner_cli::parse(std::env::args_os().skip(1)){Ok(command)=>command,Err(error)=>{eprintln!("{error}");std::process::exit(2);}};
 let serving=matches!(command,galroon_core::owner_cli::Command::Serve{..});
 // Like Windows, record why a serving Core stopped so the reason survives the container log.
 if let Err(error)=galroon_core::owner_cli::run(command).await{if serving{galroon_core::diagnostics::record("error","core.fatal",&error,None);}eprintln!("{error}");std::process::exit(1);}
}
#[cfg(not(any(windows,unix)))]fn main(){eprintln!("This executable supports Windows and Unix");std::process::exit(2);}

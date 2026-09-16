#![cfg_attr(windows,windows_subsystem="windows")]
#[tokio::main]async fn main(){
 let mut args=std::env::args_os().skip(1);let dir=if args.next().as_deref()==Some(std::ffi::OsStr::new("--data-dir")){args.next().map(std::path::PathBuf::from)}else{None};
 let Some(dir)=dir else{eprintln!("Usage: galroon-core --data-dir <local folder>");std::process::exit(2)};
 let device_dir=if args.next().as_deref()==Some(std::ffi::OsStr::new("--device-dir")){args.next().map(std::path::PathBuf::from)}else{None}.unwrap_or_else(||dir.clone());
 let _=galroon_core::diagnostics::init(&dir);
 #[cfg(windows)]if let Err(error)=galroon_core::local_core::run_with_device(dir,device_dir).await{galroon_core::diagnostics::record("error","core.fatal",&error,None);eprintln!("{error}");std::process::exit(1);}
 #[cfg(not(windows))]{let _=(dir,device_dir);eprintln!("This executable currently supports Windows");std::process::exit(2);}
}


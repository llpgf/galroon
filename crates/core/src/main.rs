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
 let raw_args:Vec<std::ffi::OsString>=std::env::args_os().skip(1).collect();
 if raw_args.len()==1&&raw_args[0]=="--version"{println!("{}",env!("CARGO_PKG_VERSION"));return;}
 let command=match galroon_core::owner_cli::parse(raw_args){Ok(command)=>command,Err(error)=>{eprintln!("{error}");std::process::exit(2);}};
 let serving=matches!(command,galroon_core::owner_cli::Command::Serve{..});
 // Like Windows, record why a serving Core stopped so the reason survives the container log.
 match galroon_core::owner_cli::run(command).await{
  Ok(0)=>{},
  Ok(code)=>std::process::exit(code),
  Err(error)=>{if serving{galroon_core::diagnostics::record("error","core.fatal",&error,None);}eprintln!("{error}");std::process::exit(1);}
 }
}
#[cfg(not(any(windows,unix)))]fn main(){eprintln!("This executable supports Windows and Unix");std::process::exit(2);}

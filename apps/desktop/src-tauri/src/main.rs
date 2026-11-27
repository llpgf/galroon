#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::Manager;
mod controller;
mod paired;
use controller::Controller;
#[tauri::command] async fn list_connections(controller:tauri::State<'_,std::sync::Arc<Controller>>)->Result<paired::Connections,String>{controller.connections().await}
#[tauri::command] async fn restore_history(controller:tauri::State<'_,std::sync::Arc<Controller>>)->Result<serde_json::Value,String>{controller.restore_history().await}
#[tauri::command] async fn pair_connection(controller:tauri::State<'_,std::sync::Arc<Controller>>,url:String,code:String)->Result<paired::Connections,String>{controller.pair(url,code).await}
#[tauri::command] async fn pair_lan_connection(controller:tauri::State<'_,std::sync::Arc<Controller>>,pairing:String)->Result<paired::Connections,String>{controller.pair_lan(pairing).await}
#[tauri::command] async fn core_request(controller:tauri::State<'_,std::sync::Arc<Controller>>,method:String,path:String,body:Option<Vec<u8>>,content_type:Option<String>,library_id:String,device_id:String)->Result<tauri::ipc::Response,String>{
 let bytes=controller.core_request(galroon_core::lan_client::ProxyRequest{method,path,body,content_type,library_id,device_id}).await?;Ok(tauri::ipc::Response::new(bytes))
}
#[tauri::command] async fn update_connection_endpoint(controller:tauri::State<'_,std::sync::Arc<Controller>>,id:String,url:String)->Result<paired::Connections,String>{controller.update_endpoint(id,url).await}
#[tauri::command] async fn select_connection(controller:tauri::State<'_,std::sync::Arc<Controller>>,id:Option<String>)->Result<(),String>{controller.select(id).await}
#[tauri::command] async fn remove_connection(controller:tauri::State<'_,std::sync::Arc<Controller>>,id:String,forget:bool)->Result<paired::Connections,String>{controller.remove_connection(id,forget).await}
#[tauri::command] async fn core_session(controller:tauri::State<'_,std::sync::Arc<Controller>>)->Result<serde_json::Value,String>{controller.client_session().await}
#[tauri::command] async fn stop_core(app:tauri::AppHandle,controller:tauri::State<'_,std::sync::Arc<Controller>>)->Result<(),String>{controller.stop().await?;app.exit(0);Ok(())}
#[tauri::command] async fn restore_core(controller:tauri::State<'_,std::sync::Arc<Controller>>,path:String,sha256:String)->Result<galroon_core::local_core::Session,String>{controller.restore(path.into(),sha256).await?;controller.session().await}
#[tauri::command] fn record_client_error(message:String){galroon_core::diagnostics::record("error","desktop.client",&message,None);}
#[tauri::command] fn diagnostic_logs()->Result<serde_json::Value,String>{galroon_core::diagnostics::snapshot()}
#[tauri::command] fn reveal_in_folder(path:String)->Result<(),String>{
    let trimmed=path.trim();
    if trimmed.is_empty()||trimmed.contains('\0'){return Err("Invalid path".into());}
    let target=std::path::PathBuf::from(trimmed);
    if !target.is_absolute(){return Err("Path must be absolute".into());}
    let existing=if target.exists(){target}else if let Some(parent)=target.parent().filter(|p|p.exists()){parent.to_path_buf()}else{return Err("Folder or file was not found on this device".into());};
    #[cfg(target_os="windows")]
    {
        let mut cmd=std::process::Command::new("explorer.exe");
        if existing.is_file(){cmd.arg(format!("/select,{}",existing.display()));}else{cmd.arg(&existing);}
        cmd.spawn().map_err(|e|format!("Failed to open Explorer: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os="macos")]
    {
        let mut cmd=std::process::Command::new("open");
        if existing.is_file(){cmd.arg("-R");}
        cmd.arg(&existing).spawn().map_err(|e|format!("Failed to open Finder: {e}"))?;
        return Ok(());
    }
    #[cfg(all(unix,not(target_os="macos")))]
    {
        let dir=if existing.is_dir(){existing.as_path()}else{existing.parent().unwrap_or(existing.as_path())};
        std::process::Command::new("xdg-open").arg(dir).spawn().map_err(|e|format!("Failed to open file manager: {e}"))?;
        return Ok(());
    }
}
fn main() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![record_client_error,diagnostic_logs,reveal_in_folder,core_session,stop_core,restore_core,restore_history,list_connections,pair_connection,pair_lan_connection,core_request,select_connection,remove_connection,update_connection_endpoint]).setup(|app|{
        if let Some(window)=app.get_webview_window("main") {
            if let Some(monitor)=window.current_monitor()? {
                let size=monitor.size().to_logical::<f64>(monitor.scale_factor());
                window.set_size(tauri::LogicalSize::new(1440.0_f64.min(size.width*0.92),940.0_f64.min(size.height*0.90)))?;
                window.center()?;
            }
        }
        let dir=std::env::var_os("GALROON_DATA").map(std::path::PathBuf::from).unwrap_or(app.path().app_local_data_dir()?);
        let _=galroon_core::diagnostics::init(&dir.join("desktop-diagnostics"));
        let exe=std::env::current_exe()?.parent().ok_or_else(||std::io::Error::other("No application directory"))?.join("galroon-core.exe");
        app.manage(Controller::new(dir,exe));
        Ok(())
    }).run(tauri::generate_context!()).expect("Unable to start Galroon");
}


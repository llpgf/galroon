#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::Manager;
mod controller;
mod paired;
use controller::Controller;
#[tauri::command] async fn list_connections(controller:tauri::State<'_,Controller>)->Result<paired::Connections,String>{controller.connections().await}
#[tauri::command] async fn restore_history(controller:tauri::State<'_,Controller>)->Result<serde_json::Value,String>{controller.restore_history().await}
#[tauri::command] async fn pair_connection(controller:tauri::State<'_,Controller>,url:String,code:String)->Result<paired::Connections,String>{controller.pair(url,code).await}
#[tauri::command] async fn update_connection_endpoint(controller:tauri::State<'_,Controller>,id:String,url:String)->Result<paired::Connections,String>{controller.update_endpoint(id,url).await}
#[tauri::command] async fn select_connection(controller:tauri::State<'_,Controller>,id:Option<String>)->Result<(),String>{controller.select(id).await}
#[tauri::command] async fn remove_connection(controller:tauri::State<'_,Controller>,id:String,forget:bool)->Result<paired::Connections,String>{controller.remove_connection(id,forget).await}
#[tauri::command] async fn core_session(controller:tauri::State<'_,Controller>)->Result<galroon_core::local_core::Session,String>{controller.session().await}
#[tauri::command] async fn stop_core(app:tauri::AppHandle,controller:tauri::State<'_,Controller>)->Result<(),String>{controller.stop().await?;app.exit(0);Ok(())}
#[tauri::command] async fn restore_core(controller:tauri::State<'_,Controller>,path:String,sha256:String)->Result<galroon_core::local_core::Session,String>{controller.restore(path.into(),sha256).await?;controller.session().await}
#[tauri::command] fn record_client_error(message:String){galroon_core::diagnostics::record("error","desktop.client",&message,None);}
#[tauri::command] fn diagnostic_logs()->Result<serde_json::Value,String>{galroon_core::diagnostics::snapshot()}
fn main() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![record_client_error,diagnostic_logs,core_session,stop_core,restore_core,restore_history,list_connections,pair_connection,select_connection,remove_connection,update_connection_endpoint]).setup(|app|{
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
        app.manage(Controller{base:dir,exe,gate:tokio::sync::Mutex::new(())});
        Ok(())
    }).run(tauri::generate_context!()).expect("Unable to start Galroon");
}


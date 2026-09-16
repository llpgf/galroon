//! Tool-managed matching UI fixture. Credentials stay in its local fixture directory.
#[tokio::main]async fn main(){
    let dir=std::env::args().nth(1).expect("Isolated test state directory required");
    let session_path=std::path::Path::new(&dir).join("ui-session.json");
    let port:u16=std::env::args().nth(2).map(|v|v.parse().expect("Invalid port")).unwrap_or(14800);
    let device=std::env::args().nth(3).unwrap_or_else(||dir.clone());
    let app=galroon_core::initialize_with_device(dir.into(),std::path::Path::new(&device)).unwrap();
    let library=galroon_core::context::library(&app.db.lock().unwrap()).unwrap();
    let listener=tokio::net::TcpListener::bind(("127.0.0.1",port)).await.unwrap();
    let session=serde_json::json!({"url":format!("http://127.0.0.1:{port}"),"token":app.token,"library_id":library.id,"device_id":app.device.id,"instance_id":app.instance_id,"api_min":1,"api_max":1});
    std::fs::write(session_path,serde_json::to_vec(&session).unwrap()).unwrap();
    println!("Matching fixture listening on loopback port {port}; session retained locally.");
    galroon_core::serve_listener(app,listener).await.unwrap();
}

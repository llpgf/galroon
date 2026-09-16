//! Local UI verification harness. Session is emitted only to its launching parent's pipe.
#[tokio::main]async fn main(){
    let dir=std::env::args().nth(1).expect("Isolated test state directory required");
    let port:u16=std::env::args().nth(2).map(|v|v.parse().expect("Invalid port")).unwrap_or(14800);
    let device=std::env::args().nth(3).unwrap_or_else(||dir.clone());
    let app=galroon_core::initialize_with_device(dir.into(),std::path::Path::new(&device)).unwrap();
    let library=galroon_core::context::library(&app.db.lock().unwrap()).unwrap();
    let listener=tokio::net::TcpListener::bind(("127.0.0.1",port)).await.unwrap();
    println!("{}",serde_json::json!({"url":format!("http://127.0.0.1:{port}"),"token":app.token,"library_id":library.id,"device_id":app.device.id,"instance_id":app.instance_id,"api_min":1,"api_max":1}));
    galroon_core::serve_listener(app,listener).await.unwrap();
}

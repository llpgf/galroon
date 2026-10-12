use super::*;
use axum::{body::Body,extract::ConnectInfo};
use crate::{network::NetworkSettings,lan_tls::{LanTls,PeerAddr}};
use tower::ServiceExt;
use std::{net::SocketAddr,time::Duration};
const PASSWORD:&str="Generated LAN fixture password";
fn fixture()->(tempfile::TempDir,App,String,String){
 let t=tempfile::tempdir().unwrap();let mut app=crate::initialize(t.path().join("state")).unwrap();app.network=Some(NetworkSettings::for_test("192.168.50.154".parse().unwrap(),14830));setup(&app.db,PASSWORD,false).unwrap();
 let code=create_pair(&app.db,"LAN desktop").unwrap();let session=redeem(&app.db,code["code"].as_str().unwrap()).unwrap();let desktop=session["token"].as_str().unwrap().to_owned();
 let (token,_)=login(&app.db,PASSWORD,"Browser").unwrap();let cookie=format!("{}={token}",cookie_name(&app));(t,app,desktop,cookie)
}
#[tokio::test]async fn role_transport_endpoint_matrix(){
 let (_t,app,desktop,cookie)=fixture();
 for transport in [Transport::Loopback,Transport::Lan]{for role in ["owner","desktop","web"]{
  let router=crate::router_for(app.clone(),transport);let host=if transport==Transport::Lan{"192.168.50.154:14830"}else{"localhost:14800"};let origin=format!("{}://{host}",if transport==Transport::Lan{"https"}else{"http"});
  for (method,path,expected) in [("GET","/api/health",200),("HEAD","/api/health",200),("GET","/api/context",200),("GET","/api/works",200),("POST","/api/works",if role=="web"{403}else{200}),("GET","/api/access/devices",if role=="owner"{200}else{403}),("POST","/api/access/login",if transport==Transport::Lan{403}else{400}),("GET","/__lan_missing_static__.js",404),("GET","/api/__lan_unknown__",404),("OPTIONS","/api/works",200),("POST","/api/access/me",if role=="web"{403}else{405})]{
   let expected=if transport==Transport::Lan&&role=="owner"{403}else{expected};
   let mut request=Request::builder().method(method).uri(path).header(header::HOST,host).header(header::ORIGIN,&origin).header(header::CONTENT_TYPE,"application/json").header(header::ACCESS_CONTROL_REQUEST_METHOD,"POST").extension(ConnectInfo(PeerAddr("192.168.1.40:42000".parse().unwrap()))).extension(Transport::Loopback);
   request=if role=="web"{request.header(header::COOKIE,&cookie)}else{request.header(header::AUTHORIZATION,format!("Bearer {}",if role=="owner"{&app.token}else{&desktop}))};
   let response=router.clone().oneshot(request.body(Body::from(r#"{"title":"LAN matrix edit","password":"incorrect"}"#)).unwrap()).await.unwrap();assert_eq!(response.status().as_u16(),expected,"{role} {transport:?} {method} {path}");
   if path=="/api/context"&&expected==200{let bytes=axum::body::to_bytes(response.into_body(),4096).await.unwrap();let value:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(value["core"]["transport"],if transport==Transport::Lan{"lan"}else{"loopback"});assert_eq!(value["core"]["location"],"local");assert_eq!(value["capabilities"]["lan_access"],true);}
  }
 }}
}
#[tokio::test]async fn lan_host_origin_and_secure_logout(){
 let (_t,app,desktop,cookie)=fixture();let router=crate::router_for(app.clone(),Transport::Lan);
 for host in ["evil.local","localhost","127.0.0.1:14830","192.168.50.154:80","NAS.local.evil"]{for (method,path) in [("OPTIONS","/api/works"),("GET","/"),("GET","/api/health"),("GET","/api/unknown")]{let request=Request::builder().method(method).uri(path).header(header::HOST,host).header(header::AUTHORIZATION,format!("Bearer {desktop}")).body(Body::empty()).unwrap();assert_eq!(router.clone().oneshot(request).await.unwrap().status(),403);}}
 for origin in ["http://nas.local","https://evil.local","http://localhost:1420","https://nas.local:443"]{for path in ["/api/access/pair","/api/access/logout"]{let request=Request::builder().method("POST").uri(path).header(header::HOST,"nas.local").header(header::ORIGIN,origin).header(header::COOKIE,&cookie).header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"code":"invalid"}"#)).unwrap();assert_eq!(router.clone().oneshot(request).await.unwrap().status(),403,"{path} {origin}");}}
 for transport in [Transport::Loopback,Transport::Lan]{for lifetime in [0,WEB_LIFETIME]{let cookie=session_cookie("test","token",lifetime,transport);assert_eq!(cookie.contains("; Secure"),transport==Transport::Lan);}}
 let request=Request::builder().method("POST").uri("/api/access/logout").header(header::HOST,"nas.local").header(header::ORIGIN,"https://nas.local").header(header::COOKIE,&cookie).body(Body::empty()).unwrap();let response=router.oneshot(request).await.unwrap();assert_eq!(response.status(),200);assert!(response.headers()[header::SET_COOKIE].to_str().unwrap().contains("Max-Age=0; Secure"));
}
#[test]fn source_budgets_normalize_addresses_and_enforce_all_three_tiers(){
 let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let c=app.db.lock().unwrap();
 for _ in 0..10{budget_from(&c,"pair","192.168.1.1".parse().unwrap()).unwrap();}
 assert!(budget_from(&c,"pair","192.168.1.1".parse().unwrap()).is_err());assert!(budget_from(&c,"pair","::ffff:192.168.1.1".parse().unwrap()).is_err());budget_from(&c,"pair","192.168.1.2".parse().unwrap()).unwrap();
 c.execute("DELETE FROM access_attempts",[]).unwrap();for i in 1..=30{budget_from(&c,"pair",format!("fd00::{}",i).parse().unwrap()).unwrap();}assert!(budget_from(&c,"pair","fd00::ffff".parse().unwrap()).is_err());budget_from(&c,"pair","fd00:0:0:1::1".parse().unwrap()).unwrap();
 c.execute("DELETE FROM access_attempts",[]).unwrap();for i in 1..=100{budget_from(&c,"pair",format!("192.168.1.{i}").parse().unwrap()).unwrap();}assert!(budget_from(&c,"pair","192.168.2.1".parse().unwrap()).is_err());
 c.execute("UPDATE access_attempts SET started=?1",[db::now()-60]).unwrap();budget_from(&c,"pair","192.168.2.1".parse().unwrap()).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM access_attempts",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}
#[test]fn source_record_cap_does_not_evict_live_limits(){
 let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();let c=app.db.lock().unwrap();
 for i in 0..SOURCE_RECORD_CAP{c.execute("INSERT INTO access_attempts VALUES(?1,?2,10)",params![format!("pair:ip:10.0.{}.{}",i/256,i%256),db::now()]).unwrap();}
 assert!(budget_from(&c,"pair","192.168.1.1".parse().unwrap()).unwrap_err().contains("sources"));assert_eq!(c.query_row("SELECT count(*) FROM access_attempts",[],|r|r.get::<_,i64>(0)).unwrap(),SOURCE_RECORD_CAP);assert!(budget_from(&c,"pair","10.0.0.0".parse().unwrap()).is_err());
 c.execute("UPDATE access_attempts SET started=0",[]).unwrap();budget_from(&c,"pair","192.168.1.1".parse().unwrap()).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM access_attempts",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}
#[tokio::test]async fn only_socket_peer_controls_rate_limit_and_loopback_is_one_source(){
 let t=tempfile::tempdir().unwrap();let mut app=crate::initialize(t.path().join("state")).unwrap();app.network=Some(NetworkSettings::for_test("192.168.50.154".parse().unwrap(),14830));
 for transport in [Transport::Lan,Transport::Loopback]{app.db.lock().unwrap().execute("DELETE FROM access_attempts",[]).unwrap();let router=crate::router_for(app.clone(),transport);let host=if transport==Transport::Lan{"nas.local"}else{"localhost"};
  for i in 0..12{let peer:SocketAddr=if i==11{"192.168.1.2:1000"}else{"192.168.1.1:1000"}.parse().unwrap();let request=Request::builder().method("POST").uri("/api/access/pair").header(header::HOST,host).header(header::ORIGIN,format!("{}://{host}",if transport==Transport::Lan{"https"}else{"http"})).header(header::CONTENT_TYPE,"application/json").header("x-forwarded-for",format!("10.0.0.{i}")).header("x-forwarded-proto","http").extension(ConnectInfo(PeerAddr(peer))).body(Body::from(r#"{"code":"invalid"}"#)).unwrap();let response=router.clone().oneshot(request).await.unwrap();let bytes=axum::body::to_bytes(response.into_body(),4096).await.unwrap();let error:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(error["error"].as_str().unwrap().contains("Too many"),i==10||(i==11&&transport==Transport::Loopback));}
 }
}
#[tokio::test]async fn real_tls_permissions_and_both_listeners_drain(){
 use tokio::io::{AsyncReadExt,AsyncWriteExt};
 let (_t,mut app,desktop,_cookie)=fixture();let plain=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let lan=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let plain_addr=plain.local_addr().unwrap();let lan_addr=lan.local_addr().unwrap();app.network=Some(NetworkSettings::for_test(lan_addr.ip(),lan_addr.port()));
 let tls=LanTls::load_or_generate(&app.device_dir,app.network.as_ref().unwrap()).unwrap();let mut roots=rustls::RootCertStore::empty();roots.add(tls.cert_der.clone()).unwrap();
 let config=rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider())).with_safe_default_protocol_versions().unwrap().with_root_certificates(roots).with_no_client_auth();let connector=tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
 // Trust only this fixture certificate; hostname and TLS handshake signatures remain verified.
 let client=reqwest::Client::builder().no_proxy().tls_built_in_root_certs(false).add_root_certificate(reqwest::Certificate::from_der(&tls.cert_der).unwrap()).timeout(Duration::from_secs(3)).build().unwrap();
 let (stop,stopped)=tokio::sync::oneshot::channel();let server=tokio::spawn(crate::serve_tls_listeners(app.clone(),vec![crate::Listener::Loopback(plain),crate::Listener::Lan(lan,tls.config)],async{let _=stopped.await;}));let base=format!("https://{lan_addr}");
 let stalled=tokio::net::TcpStream::connect(lan_addr).await.unwrap();
 assert_eq!(client.get(format!("{base}/api/health")).send().await.unwrap().status(),200);
 assert!(client.get(format!("http://{lan_addr}/api/health")).send().await.is_err());
 assert_eq!(client.get(format!("{base}/api/health")).bearer_auth(&app.token).send().await.unwrap().status(),403);
 assert_eq!(client.post(format!("{base}/api/access/login")).header("Origin",&base).json(&json!({"password":PASSWORD})).send().await.unwrap().status(),403);
 assert_eq!(client.get(format!("{base}/api/health")).header("Host","evil.local").send().await.unwrap().status(),403);
 let context=client.get(format!("{base}/api/context")).bearer_auth(&desktop).send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();assert_eq!(context["core"]["transport"],"lan");
 let code=create_pair(&app.db,"HTTPS redeemed").unwrap();let paired=client.post(format!("{base}/api/access/pair")).header("Origin",&base).json(&json!({"code":code["code"]})).send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();assert_eq!(paired["role"],"desktop");
 assert_eq!(client.get(format!("http://{plain_addr}/api/context")).bearer_auth(&app.token).send().await.unwrap().json::<Value>().await.unwrap()["core"]["transport"],"loopback");
 let tcp=tokio::net::TcpStream::connect(lan_addr).await.unwrap();let name=rustls::pki_types::ServerName::IpAddress(lan_addr.ip().into());let tls_stream=connector.connect(name,tcp).await.unwrap();let plain_stream=tokio::net::TcpStream::connect(plain_addr).await.unwrap();
 trait Stream:tokio::io::AsyncRead+tokio::io::AsyncWrite+Unpin+Send{}impl<T:tokio::io::AsyncRead+tokio::io::AsyncWrite+Unpin+Send> Stream for T{}
 let mut pending:Vec<(Box<dyn Stream>,SocketAddr)>=vec![(Box::new(plain_stream),plain_addr),(Box::new(tls_stream),lan_addr)];let body=r#"{"title":"LAN drain fixture"}"#;
 for (stream,addr) in &mut pending{let headers=format!("POST /api/works HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {desktop}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",body.len());stream.write_all(headers.as_bytes()).await.unwrap();let mut ready=[0;25];tokio::time::timeout(Duration::from_secs(3),stream.read_exact(&mut ready)).await.unwrap().unwrap();assert_eq!(&ready,b"HTTP/1.1 100 Continue\r\n\r\n");}
 stop.send(()).unwrap();tokio::time::timeout(Duration::from_secs(20),async{for addr in [plain_addr,lan_addr]{while tokio::net::TcpStream::connect(addr).await.is_ok(){tokio::time::sleep(Duration::from_millis(10)).await;}}}).await.unwrap();
 for (mut stream,_) in pending{assert!(!server.is_finished());stream.write_all(body.as_bytes()).await.unwrap();let mut response=String::new();tokio::time::timeout(Duration::from_secs(3),stream.read_to_string(&mut response)).await.unwrap().unwrap();assert!(response.starts_with("HTTP/1.1 200 OK"),"{response}");}
 tokio::time::timeout(Duration::from_secs(3),server).await.unwrap().unwrap().unwrap();drop(stalled);assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM works WHERE title='LAN drain fixture'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}
#[tokio::test]async fn disabled_opens_no_lan_listener_and_bind_conflicts_fail(){
 let t=tempfile::tempdir().unwrap();let mut app=crate::initialize(t.path().join("state")).unwrap();
 for configured in [false,true]{if configured{std::fs::write(app.device_dir.join("network.json"),r#"{"version":1,"enabled":false,"address":"192.168.50.154","port":14830,"hostnames":[]}"#).unwrap();app.network=NetworkSettings::load(&app.device_dir).unwrap();}
  let tcp=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let listeners=crate::configured_listeners(&app,tcp).await.unwrap();assert_eq!(listeners.len(),1);assert!(!app.device_dir.join("lan-key.pem").exists());
 }
 let occupied=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let addr=occupied.local_addr().unwrap();app.network=Some(NetworkSettings::for_test(addr.ip(),addr.port()));let tcp=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let error=crate::configured_listeners(&app,tcp).await.err().unwrap();assert!(error.contains("Cannot bind LAN HTTPS listener"));assert!(error.contains(&addr.to_string()));
}
#[tokio::test]async fn lan_pair_requires_peer_and_preflight_only_allows_https_same_origin(){
 let (_t,app,_,_)=fixture();let router=crate::router_for(app,Transport::Lan);
 let request=Request::builder().method("POST").uri("/api/access/pair").header(header::HOST,"nas.local").header(header::ORIGIN,"https://nas.local").header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"code":"invalid"}"#)).unwrap();assert_eq!(router.clone().oneshot(request).await.unwrap().status(),403);
 for origin in ["https://nas.local","http://nas.local","http://localhost:1420","https://other.local"]{let request=Request::builder().method("OPTIONS").uri("/api/works").header(header::HOST,"nas.local").header(header::ORIGIN,origin).header(header::ACCESS_CONTROL_REQUEST_METHOD,"POST").body(Body::empty()).unwrap();let response=router.clone().oneshot(request).await.unwrap();assert_eq!(response.status(),200);assert_eq!(response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),origin=="https://nas.local");}
}

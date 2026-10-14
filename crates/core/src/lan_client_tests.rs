use super::*;
use crate::{lan_tls::{LanTls,TlsListener,PeerAddr,spki_fingerprint,format_spki_fingerprint},network::NetworkSettings};
use axum::{body::Body,http::{Request,StatusCode},response::IntoResponse,routing::{get,post},Router};
use std::sync::atomic::{AtomicUsize,Ordering};
use sha2::{Digest,Sha256};
fn tls()->(tempfile::TempDir,LanTls){let dir=tempfile::tempdir().unwrap();let tls=LanTls::load_or_generate(dir.path(),&NetworkSettings::for_test("192.168.1.2".parse().unwrap(),14830)).unwrap();(dir,tls)}
fn request(method:&str,path:&str)->ProxyRequest{ProxyRequest{method:method.into(),path:path.into(),body:None,content_type:None,library_id:"library".into(),device_id:"device".into()}}
#[test]fn validation_and_binary_envelope(){
 let c=LanConnection::new("https://nas.local:14830".into(),format_spki_fingerprint(&[1;32]),"secret".into(),"library".into(),"device".into()).unwrap();
 for path in ["/api/../health","https://nas.local/api/health","//nas.local/api/health","/health","/api/%2e%2e/health","/api/x\\..\\health","/api/x#fragment","/api/%252e/x"]{assert!(validate_request(&c,&request("GET",path)).is_err(),"{path}");}
 assert!(validate_request(&c,&request("GET",&format!("/api/{}","x".repeat(2048)))).is_err());
 for method in ["DELETE","PUT","OPTIONS","get"]{assert!(validate_request(&c,&request(method,"/api/health")).is_err());}
 for method in ["GET","POST","HEAD"]{assert!(validate_request(&c,&request(method,"/api/health")).is_ok());}
 for field in ["library","device"]{let mut r=request("GET","/api/context");if field=="library"{r.library_id="other".into();}else{r.device_id="other".into();}assert!(validate_request(&c,&r).unwrap_err().starts_with("409"));}
 for raw in ["http://nas.local:123","https://8.8.8.8:123","https://127.0.0.1:123","https://[fe80::1]:123","https://bad_host:123","https://nas.local:0","https://user@nas.local:123","https://nas.local:123/path"]{assert!(endpoint(raw).is_err(),"{raw}");}
 for raw in ["https://192.168.1.2:14830","https://[fd00::1]:14830","https://nas-1.local:14830"]{assert_eq!(endpoint(raw).unwrap(),raw);}
 for fp in [[0;32],[255;32],[127;32]]{assert_eq!(decode_fingerprint(&format_spki_fingerprint(&fp)).unwrap(),fp);}
 assert!(decode_fingerprint(&"B".repeat(52)).is_err());
 let bytes=ProxyResponse{status:503,headers:BTreeMap::from([("retry-after".into(),"3".into())]),body:vec![0,255,128,1]}.into_ipc_bytes().unwrap();let n=u32::from_be_bytes(bytes[..4].try_into().unwrap())as usize;assert_eq!(&bytes[4+n..],&[0,255,128,1]);let meta:Value=serde_json::from_slice(&bytes[4..4+n]).unwrap();assert_eq!(meta["status"],503);assert_eq!(meta["headers"]["retry-after"],"3");
 let verifier=PinnedVerifier{fingerprint:[0;32]};assert!(verifier.verify_server_cert(&CertificateDer::from(vec![0,1,2]),&[],&"nas.local".try_into().unwrap(),&[],UnixTime::now()).is_err());
}
// Real TLS over in-memory streams also runs under the socket-binding sandbox.
#[derive(Debug)]struct BadKey(Arc<dyn rustls::sign::SigningKey>);
#[derive(Debug)]struct BadSignature(Box<dyn rustls::sign::Signer>);
impl rustls::sign::SigningKey for BadKey{
 fn choose_scheme(&self,schemes:&[SignatureScheme])->Option<Box<dyn rustls::sign::Signer>>{self.0.choose_scheme(schemes).map(|s|Box::new(BadSignature(s))as Box<dyn rustls::sign::Signer>)}
 fn algorithm(&self)->rustls::SignatureAlgorithm{self.0.algorithm()}
}
impl rustls::sign::Signer for BadSignature{
 fn sign(&self,message:&[u8])->Result<Vec<u8>,rustls::Error>{let mut signature=self.0.sign(message)?;*signature.last_mut().unwrap()^=1;Ok(signature)}
 fn scheme(&self)->SignatureScheme{self.0.scheme()}
}
#[tokio::test]async fn handshake_checks_pin_and_signatures_in_both_tls_versions(){
 let (_dir,tls)=tls();let (_other,other)=self::tls();let fp=spki_fingerprint(&tls.cert_der);
 for version in [&rustls::version::TLS12,&rustls::version::TLS13]{for kind in ["valid","wrong pin","changed key","bad signature"]{
  let mut server=if kind=="changed key"{(*other.config).clone()}else{(*tls.config).clone()};
  if kind=="bad signature"{
   use rustls::pki_types::{PrivateKeyDer,pem::PemObject};
   let pem=std::fs::read(_dir.path().join("lan-key.pem")).unwrap();let key=PrivateKeyDer::from_pem_slice(&pem).unwrap();let key=rustls::crypto::ring::sign::any_supported_type(&key).unwrap();
   let key=rustls::sign::CertifiedKey::new(vec![tls.cert_der.clone()],Arc::new(BadKey(key)));server.cert_resolver=Arc::new(rustls::sign::SingleCertAndKey::from(key));
  }
  let config=rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_protocol_versions(&[version]).unwrap().dangerous().with_custom_certificate_verifier(Arc::new(PinnedVerifier{fingerprint:if kind=="wrong pin"{[0;32]}else{fp}})).with_no_client_auth();
  let (client,stream)=tokio::io::duplex(16384);let acceptor=tokio_rustls::TlsAcceptor::from(Arc::new(server));let server=tokio::spawn(async move{acceptor.accept(stream).await});
  // The SAN deliberately differs: trust must come from SPKI alone.
  let result=tokio_rustls::TlsConnector::from(Arc::new(config)).connect("different-host.local".try_into().unwrap(),client).await;
  assert_eq!(result.is_ok(),kind=="valid","{kind} {version:?}");drop(result);let _=server.await;
 }}
}
struct Fixture{_dir:tempfile::TempDir,app:crate::App,url:String,fp:[u8;32],server:tokio::task::JoinHandle<()>}
impl Drop for Fixture{fn drop(&mut self){self.server.abort();}}
async fn fixture(extra:Router)->Fixture{
 let dir=tempfile::tempdir().unwrap();let mut app=crate::initialize(dir.path().join("catalog")).unwrap();
 let tcp=tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("LAN client TLS fixture requires socket binding (sandbox may block)");let addr=tcp.local_addr().unwrap();
 let settings=NetworkSettings::for_test(addr.ip(),addr.port());app.network=Some(settings.clone());let tls=LanTls::load_or_generate(&app.device_dir,&settings).unwrap();let fp=spki_fingerprint(&tls.cert_der);
 let router=crate::router_for(app.clone(),crate::Transport::Lan).merge(extra);let listener=TlsListener::new(tcp,tls.config);let server=tokio::spawn(async move{axum::serve(listener,router.into_make_service_with_connect_info::<PeerAddr>()).await.unwrap();});
 Fixture{_dir:dir,app,url:format!("https://{addr}"),fp,server}
}
fn code(f:&Fixture,fp:[u8;32])->String{
 let code="123456ABCDEF";let c=f.app.db.lock().unwrap();c.execute("INSERT OR REPLACE INTO pairing_codes(hash,name,expires,role,id) VALUES(?1,'Test desktop',?2,'desktop','fixture')",rusqlite::params![hex::encode(Sha256::digest(code.as_bytes())),crate::db::now()+300]).unwrap();
 format!("galroon-pair:v1?addr={}&code={code}&fp={}",f.url.trim_start_matches("https://"),format_spki_fingerprint(&fp))
}
fn conn(f:&Fixture,token:String,library_id:String,device_id:String)->LanConnection{LanConnection::with_client(f.url.clone(),pinned_client(f.fp).unwrap(),token,library_id,device_id).unwrap()}
#[tokio::test]async fn pairing_checks_pin_before_sending_code_and_context_is_pinned(){
 let f=fixture(Router::new()).await;let pairing=code(&f,[0;32]);assert_eq!(pair_with_string(&pairing,"Desktop").await.err().unwrap(),PIN_MISMATCH);
 {let c=f.app.db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM pairing_codes",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT count(*) FROM access_attempts",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
 let paired=pair_with_string(&code(&f,f.fp),"Desktop").await.unwrap();let token=paired.token.clone();let c=conn(&f,paired.token,paired.library_id.clone(),paired.device_id.clone());let session=serde_json::to_value(c.session().await.unwrap()).unwrap();assert_eq!(session["url"],"ipc://core");assert_eq!(session["token"],"");assert_eq!(session["remote"],true);let mut r=request("GET","/api/context");r.library_id=paired.library_id;r.device_id=paired.device_id;let response=proxy(&c,r).await.unwrap();assert_eq!(response.status,200);let v:Value=serde_json::from_slice(&response.body).unwrap();assert_eq!(v["core"]["transport"],"lan");
 // A well-formed token whose session was revoked; a malformed token is plain "Authentication required".
 f.app.db.lock().unwrap().execute("UPDATE sessions SET revoked=1",[]).unwrap();let c=conn(&f,token,c.library_id,c.device_id);let mut r=request("GET","/api/context");r.library_id=c.library_id.clone();r.device_id=c.device_id.clone();assert!(proxy(&c,r).await.unwrap().credential_invalid());
}
#[tokio::test]async fn parse_errors_pass_through(){for value in ["bad","galroon-pair:v1?bad=x"]{assert_eq!(pair_with_string(value,"Desktop").await.err().unwrap(),crate::pairing::parse_pairing_string(value).unwrap_err());}}
#[tokio::test]async fn proxy_preserves_errors_headers_binary_and_never_retries_post(){
 let count=Arc::new(AtomicUsize::new(0));let calls=count.clone();
 let extra=Router::new().route("/api/test/{status}",post(move|axum::extract::Path(status):axum::extract::Path<u16>,r:Request<Body>|{let calls=calls.clone();async move{calls.fetch_add(1,Ordering::SeqCst);assert_eq!(r.headers()["x-galroon-library"],"library");assert_eq!(r.headers()["x-galroon-device"],"device");(StatusCode::from_u16(status).unwrap(),[("content-type","application/json"),("retry-after","7"),("x-galroon-library","library"),("x-galroon-device","device"),("set-cookie","do-not-forward")],r#"{ "code":"credential_invalid", "error":"untouched" }"#)}})).route("/api/binary",get(||async{([( "content-type","image/png")],vec![0u8,255,128,1])}));
 let f=fixture(extra).await;let c=conn(&f,"token".into(),"library".into(),"device".into());
 for status in [401,403,409,503]{let response=proxy(&c,request("POST",&format!("/api/test/{status}"))).await.unwrap();assert_eq!(response.status,status);assert_eq!(response.body,br#"{ "code":"credential_invalid", "error":"untouched" }"#);assert_eq!(response.headers.len(),4);assert_eq!(response.headers["retry-after"],"7");}
 assert_eq!(count.load(Ordering::SeqCst),4);let response=proxy(&c,request("GET","/api/binary")).await.unwrap();assert_eq!(response.body,[0,255,128,1]);
 assert!(proxy(&c,request("HEAD","/api/binary")).await.unwrap().body.is_empty());
}
#[tokio::test]async fn response_limits_include_chunked_bodies(){
 async fn large(axum::extract::Path(size):axum::extract::Path<usize>)->axum::response::Response{
  // No size hint: force chunked transfer so the incremental bound is exercised.
  let mut response=Body::from_stream(Body::from(vec![b'x';size]).into_data_stream()).into_response();response.headers_mut().insert("content-type","text/plain".parse().unwrap());response
 }
 let extra=Router::new().route("/api/large/{size}",get(large)).route("/api/artwork/{size}",get(large));let f=fixture(extra).await;let c=conn(&f,"token".into(),"library".into(),"device".into());
 for (path,limit) in [("/api/large",JSON_LIMIT),("/api/artwork",IMAGE_LIMIT)]{assert_eq!(proxy(&c,request("GET",&format!("{path}/{limit}"))).await.unwrap().body.len(),limit);assert!(proxy(&c,request("GET",&format!("{path}/{}",limit+1))).await.unwrap_err().contains("size limit"));}
}

#[tokio::test]async fn cancellation_stops_queued_requests_and_future_calls(){
 let c=LanConnection::new("https://nas.local:14830".into(),format_spki_fingerprint(&[1;32]),"secret".into(),"library".into(),"device".into()).unwrap();
 let held=c.slots.acquire_many(8).await.unwrap();assert!(c.slots.try_acquire().is_err());
 let call=proxy(&c,request("POST","/api/works"));tokio::pin!(call);
 tokio::select!{result=&mut call=>panic!("Request escaped the concurrency limit: {result:?}"),_=tokio::task::yield_now()=>{}}
 c.cancel();assert!(c.is_cancelled());assert_eq!(call.await.unwrap_err(),"Connection changed");drop(held);
 assert_eq!(c.slots.available_permits(),8);assert_eq!(proxy(&c,request("GET","/api/context")).await.unwrap_err(),"Connection changed");
}
#[tokio::test]async fn response_reader_preserves_errors_and_enforces_limits_without_sockets(){
 for status in [401,403,409,503]{let raw=br#"{ "code": "credential_invalid", "error": "original" }"#.to_vec();
  let response=reqwest::Response::from(axum::http::Response::builder().status(status).header("content-type","application/json").header("retry-after","7").header("set-cookie","secret").body(raw.clone()).unwrap());
  let response=read_response(response,1024).await.unwrap();assert_eq!(response.status,status);assert_eq!(response.body,raw);assert_eq!(response.headers.len(),2);assert_eq!(response.credential_invalid(),status==401);
 }
 for limit in [IMAGE_LIMIT,JSON_LIMIT]{for size in [limit,limit+1]{let response=reqwest::Response::from(axum::http::Response::new(vec![0u8;size]));let result=read_response(response,limit).await;if size==limit{assert_eq!(result.unwrap().body.len(),size);}else{assert!(result.unwrap_err().contains("size limit"));}}}
}

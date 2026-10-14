//! Pinned desktop LAN transport. Secrets stay in the native process.
use std::{collections::BTreeMap,sync::Arc,time::Duration};
use rustls::{client::danger::{HandshakeSignatureValid,ServerCertVerified,ServerCertVerifier},pki_types::{CertificateDer,ServerName,UnixTime},DigitallySignedStruct,SignatureScheme};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
type R<T>=Result<T,String>;
const PIN_MISMATCH:&str="The Core's certificate fingerprint doesn't match the pairing string; refusing to connect";
const JSON_LIMIT:usize=32*1024*1024;
const IMAGE_LIMIT:usize=16*1024*1024;
#[derive(Debug)]
struct PinnedVerifier{fingerprint:[u8;32]}
impl ServerCertVerifier for PinnedVerifier{
 fn verify_server_cert(&self,leaf:&CertificateDer<'_>,_:&[CertificateDer<'_>],_:&ServerName<'_>,_:&[u8],_:UnixTime)->Result<ServerCertVerified,rustls::Error>{
  rustls::server::ParsedCertificate::try_from(leaf)?;
  if crate::lan_tls::spki_fingerprint(leaf)!=self.fingerprint{return Err(rustls::Error::General(PIN_MISMATCH.into()));}
  Ok(ServerCertVerified::assertion())
 }
 fn verify_tls12_signature(&self,message:&[u8],cert:&CertificateDer<'_>,signature:&DigitallySignedStruct)->Result<HandshakeSignatureValid,rustls::Error>{rustls::crypto::verify_tls12_signature(message,cert,signature,&rustls::crypto::ring::default_provider().signature_verification_algorithms)}
 fn verify_tls13_signature(&self,message:&[u8],cert:&CertificateDer<'_>,signature:&DigitallySignedStruct)->Result<HandshakeSignatureValid,rustls::Error>{rustls::crypto::verify_tls13_signature(message,cert,signature,&rustls::crypto::ring::default_provider().signature_verification_algorithms)}
 fn supported_verify_schemes(&self)->Vec<SignatureScheme>{rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()}
}
fn tls_config(fingerprint:[u8;32])->R<rustls::ClientConfig>{Ok(rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_protocol_versions(&[&rustls::version::TLS13,&rustls::version::TLS12]).map_err(|e|e.to_string())?.dangerous().with_custom_certificate_verifier(Arc::new(PinnedVerifier{fingerprint})).with_no_client_auth())}
pub fn pinned_client(fingerprint:[u8;32])->R<reqwest::Client>{reqwest::Client::builder().use_preconfigured_tls(tls_config(fingerprint)?).https_only(true).no_proxy().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(60)).build().map_err(|e|e.to_string())}
pub fn decode_fingerprint(value:&str)->R<[u8;32]>{
 // Reuse the canonical grammar (including the final padding bits).
 crate::pairing::format_pairing_string("nas.local",1,"000000000000",value)?;
 let mut result=[0;32];let(mut bits,mut buffer,mut index)=(0,0u32,0);
 for b in value.bytes(){let n=if b.is_ascii_uppercase(){b-b'A'}else{b-b'2'+26};buffer=(buffer<<5)|u32::from(n);bits+=5;if bits>=8{bits-=8;result[index]=(buffer>>bits)as u8;index+=1;}}
 Ok(result)
}
/// Validate saved LAN endpoints without allowing URL credentials, paths or public IP literals.
pub fn endpoint(raw:&str)->R<String>{
 let url=reqwest::Url::parse(raw.trim()).map_err(|_|"Invalid LAN Core URL")?;
 if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.path()!="/"||url.query().is_some()||url.fragment().is_some(){return Err("LAN Core requires an HTTPS origin with a port".into());}
 let host=url.host_str().ok_or("Missing LAN host")?;let port=url.port_or_known_default().filter(|p|*p!=0).ok_or("Missing LAN port")?;
 crate::pairing::format_pairing_string(host,port,"000000000000",&crate::lan_tls::format_spki_fingerprint(&[0;32]))?;
 if let Ok(ip)=host.trim_matches(['[',']']).parse(){crate::network::validate_address(ip)?;}
 Ok(format!("https://{host}:{port}"))
}
fn transport_error(e:reqwest::Error)->String{
 use std::error::Error;let mut source:Option<&(dyn Error+'static)>=Some(&e);
 while let Some(error)=source{if error.to_string().contains(PIN_MISMATCH){return PIN_MISMATCH.into();}source=error.source();}
 // Do not include URLs (which may contain user data) or authorization values.
 if e.is_timeout(){"Core request timed out".into()}else{"Core connection failed".into()}
}
#[derive(Serialize)]
pub struct PairedLan{pub url:String,pub fingerprint:String,pub token:String,pub library_id:String,pub device_id:String,pub name:String}
pub async fn pair_with_string(pairing:&str,device_name:&str)->R<PairedLan>{
 let parsed=crate::pairing::parse_pairing_string(pairing)?;let url=format!("https://{}:{}",parsed.host,parsed.port);
 // Test fixtures bind loopback; production callers validate the saved endpoint as well.
 if let Ok(ip)=parsed.host.trim_matches(['[',']']).parse(){
  #[cfg(not(test))]crate::network::validate_address(ip)?;
  #[cfg(test)]if !std::net::IpAddr::is_loopback(&ip){crate::network::validate_address(ip)?;}
 }
 let client=pinned_client(decode_fingerprint(&parsed.fingerprint)?)?;
 let health=read_response(client.get(format!("{url}/api/health")).send().await.map_err(transport_error)?,65536).await?;success(&health)?;
 let reply=read_response(client.post(format!("{url}/api/access/pair")).header("Origin",&url).json(&json!({"code":parsed.code,"name":device_name})).send().await.map_err(transport_error)?,65536).await?;
 let reply=success(&reply)?;let token=field(&reply,"token")?;
 let context=read_response(client.get(format!("{url}/api/context")).bearer_auth(&token).send().await.map_err(transport_error)?,65536).await?;
 let context=success(&context)?;
 Ok(PairedLan{url,fingerprint:parsed.fingerprint,token,library_id:field(&context["library"],"id")?,device_id:field(&context["core"]["device"],"id")?,name:field(&context["library"],"name")?})
}
fn field(value:&Value,key:&str)->R<String>{value[key].as_str().filter(|s|!s.is_empty()&&s.len()<1024).map(str::to_owned).ok_or_else(||format!("Core response is missing {key}"))}
fn success(response:&ProxyResponse)->R<Value>{if !(200..300).contains(&response.status){return Err(if response.credential_invalid(){"credential_invalid".into()}else{format!("Core request failed ({})",response.status)});}serde_json::from_slice(&response.body).map_err(|_|"Core returned invalid JSON".into())}
/// Immutable credentials and identity; one HTTP pool and eight in-flight requests per connection.
/// Dropping a proxy future cancels the upstream request and releases its slot.
pub struct LanConnection{url:String,token:String,library_id:String,device_id:String,client:reqwest::Client,slots:tokio::sync::Semaphore,cancel:tokio::sync::watch::Sender<bool>}
impl LanConnection{
 pub fn new(url:String,fingerprint:String,token:String,library_id:String,device_id:String)->R<Self>{
  let url=endpoint(&url)?;Self::with_client(url,pinned_client(decode_fingerprint(&fingerprint)?)?,token,library_id,device_id)
 }
 fn with_client(url:String,client:reqwest::Client,token:String,library_id:String,device_id:String)->R<Self>{let(cancel,_)=tokio::sync::watch::channel(false);Ok(Self{url,token,library_id,device_id,client,slots:tokio::sync::Semaphore::new(8),cancel})}
 pub fn cancel(&self){self.cancel.send_replace(true);}
 pub fn is_cancelled(&self)->bool{*self.cancel.borrow()}
 pub async fn session(&self)->R<RemoteSession>{
  let response=proxy(self,ProxyRequest{method:"GET".into(),path:"/api/context".into(),body:None,content_type:None,library_id:self.library_id.clone(),device_id:self.device_id.clone()}).await?;
  if response.status==409&&serde_json::from_slice::<Value>(&response.body).is_ok_and(|v|v["code"]=="collection_changed"){return Err("collection_changed".into());}
  let value=success(&response)?;
  if field(&value["library"],"id")?!=self.library_id||field(&value["core"]["device"],"id")?!=self.device_id{return Err("collection_changed".into());}
  let min=value["api"]["min"].as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Missing API compatibility")?;
  let max=value["api"]["max"].as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Missing API compatibility")?;
  Ok(RemoteSession{url:"ipc://core",token:"",remote:true,library_id:self.library_id.clone(),device_id:self.device_id.clone(),api_min:min,api_max:max})
 }
}
#[derive(Serialize)]
pub struct RemoteSession{pub url:&'static str,pub token:&'static str,pub remote:bool,pub library_id:String,pub device_id:String,pub api_min:u32,pub api_max:u32}
#[derive(Deserialize)]
pub struct ProxyRequest{pub method:String,pub path:String,pub body:Option<Vec<u8>>,pub content_type:Option<String>,pub library_id:String,pub device_id:String}
#[derive(Debug)]
pub struct ProxyResponse{pub status:u16,pub headers:BTreeMap<String,String>,pub body:Vec<u8>}
impl ProxyResponse{
 pub fn credential_invalid(&self)->bool{self.status==401&&serde_json::from_slice::<Value>(&self.body).is_ok_and(|v|v["code"]=="credential_invalid")}
 /// Wire format: big-endian u32 JSON metadata length, metadata, then untouched body bytes.
 pub fn into_ipc_bytes(self)->R<Vec<u8>>{let metadata=serde_json::to_vec(&json!({"status":self.status,"headers":self.headers})).map_err(|e|e.to_string())?;let mut result=Vec::with_capacity(4+metadata.len()+self.body.len());result.extend_from_slice(&(metadata.len()as u32).to_be_bytes());result.extend_from_slice(&metadata);result.extend_from_slice(&self.body);Ok(result)}
}
fn validate_request(conn:&LanConnection,request:&ProxyRequest)->R<reqwest::Method>{
 let path=&request.path;
 // Check the raw path before URL normalization; encoded traversal and backslashes cannot escape.
 if !path.starts_with("/api/")||path.len()>2048||path.contains("..")||path.contains('#')||path.contains('\\')||path.contains("://")||path.bytes().any(|b|b.is_ascii_control()||b==b' '){return Err("Invalid Core API path".into());}
 let route=path.split('?').next().unwrap().to_ascii_lowercase();if route.contains("%2e")||route.contains("%2f")||route.contains("%5c")||route.contains("%25"){return Err("Invalid encoded Core API path".into());}
 if request.library_id!=conn.library_id||request.device_id!=conn.device_id{return Err("409 collection_changed: saved Core identity does not match request".into());}
 match request.method.as_str(){"GET"=>Ok(reqwest::Method::GET),"POST"=>Ok(reqwest::Method::POST),"HEAD"=>Ok(reqwest::Method::HEAD),_=>Err("Only GET, POST and HEAD are allowed".into())}
}
pub async fn proxy(conn:&LanConnection,request:ProxyRequest)->R<ProxyResponse>{
 let method=validate_request(conn,&request)?;let mut cancelled=conn.cancel.subscribe();if *cancelled.borrow(){return Err("Connection changed".into());}
 tokio::select!{biased;_=cancelled.changed()=>Err("Connection changed".into()),result=tokio::time::timeout(Duration::from_secs(60),async{
  let _slot=conn.slots.acquire().await.map_err(|_|"Connection closed")?;
  let route=request.path.split('?').next().unwrap();let limit=if route=="/api/artwork"||route.starts_with("/api/artwork/"){IMAGE_LIMIT}else{JSON_LIMIT};
  let mut builder=conn.client.request(method,format!("{}{}",conn.url,request.path)).bearer_auth(&conn.token).header("Origin",&conn.url).header("x-galroon-library",&conn.library_id).header("x-galroon-device",&conn.device_id);
  if let Some(content_type)=request.content_type{builder=builder.header("content-type",content_type);}
  if let Some(body)=request.body{builder=builder.body(body);}
  read_response(builder.send().await.map_err(transport_error)?,limit).await
 })=>result.map_err(|_|"Core request timed out")?}
}
async fn read_response(mut response:reqwest::Response,limit:usize)->R<ProxyResponse>{
 let status=response.status().as_u16();let mut headers=BTreeMap::new();for name in ["content-type","x-galroon-library","x-galroon-device","retry-after"]{if let Some(value)=response.headers().get(name).and_then(|v|v.to_str().ok()){headers.insert(name.into(),value.into());}}
 if response.content_length().is_some_and(|n|n>limit as u64){return Err("Core response exceeds size limit".into());}
 let mut body=Vec::new();while let Some(chunk)=response.chunk().await.map_err(transport_error)?{if chunk.len()>limit-body.len(){return Err("Core response exceeds size limit".into());}body.extend_from_slice(&chunk);}
 Ok(ProxyResponse{status,headers,body})
}
#[cfg(test)]#[path="lan_client_tests.rs"]mod tests;

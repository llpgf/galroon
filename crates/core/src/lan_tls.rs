//! Persistent device certificate and concurrent TLS-only acceptance.
use std::{fs,io::Write,net::SocketAddr,path::Path,sync::Arc,time::Duration};
use rustls::{pki_types::{CertificateDer,PrivateKeyDer,pem::PemObject},server::ParsedCertificate};
use sha2::{Digest,Sha256};
use tokio::{net::{TcpListener,TcpStream},task::JoinSet};
use tokio_rustls::{TlsAcceptor,server::TlsStream};
type R<T>=Result<T,String>;
pub struct LanTls{pub cert_der:CertificateDer<'static>,pub config:Arc<rustls::ServerConfig>}
impl LanTls{
 pub fn load_or_generate(device_dir:&Path,settings:&crate::network::NetworkSettings)->R<Self>{
  let key_path=device_dir.join("lan-key.pem");let cert_path=device_dir.join("lan-cert.pem");
  crate::plans::validate_chain(&key_path)?;crate::plans::validate_chain(&cert_path)?;
  // Serialize first creation across catalogs sharing a device directory.
  let lock_path=device_dir.join("lan-tls.lock");crate::plans::validate_chain(&lock_path)?;
  let lock=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(lock_path).map_err(|e|e.to_string())?;
  fs2::FileExt::lock_exclusive(&lock).map_err(|e|e.to_string())?;
  let key=read_existing(&key_path)?;let cert=read_existing(&cert_path)?;
  let (key,cert)=match (key,cert){
   (Some(key),Some(cert))=>(key,cert),
   (None,None)=>{
    let key=rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e|e.to_string())?;
    let cert=rcgen::CertificateParams::new(settings.certificate_names()).map_err(|e|e.to_string())?.self_signed(&key).map_err(|e|e.to_string())?;
    let key=key.serialize_pem().into_bytes();let cert=cert.pem().into_bytes();
    write_new(&key_path,&key,true)?;write_new(&cert_path,&cert,false)?;(key,cert)
   },
   _=>return Err("Incomplete LAN TLS identity: restore both lan-key.pem and lan-cert.pem; existing files will not be overwritten".into())
  };
  let cert_der=CertificateDer::from_pem_slice(&cert).map_err(|e|format!("Invalid lan-cert.pem: {e}"))?;
  ParsedCertificate::try_from(&cert_der).map_err(|e|format!("Invalid lan-cert.pem: {e}"))?;
  let key=PrivateKeyDer::from_pem_slice(&key).map_err(|e|format!("Invalid lan-key.pem: {e}"))?;
  let config=rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_protocol_versions(&[&rustls::version::TLS13,&rustls::version::TLS12]).map_err(|e|e.to_string())?.with_no_client_auth().with_single_cert(vec![cert_der.clone()],key).map_err(|e|format!("Invalid LAN TLS identity: {e}"))?;
  Ok(Self{cert_der,config:Arc::new(config)})
 }
}
fn read_existing(path:&Path)->R<Option<Vec<u8>>>{if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){return Err(format!("TLS identity must not be a symbolic link: {}",path.display()));}match fs::read(path){Ok(v)=>Ok(Some(v)),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(format!("Cannot read {}: {e}",path.display()))}}
fn write_new(path:&Path,bytes:&[u8],private:bool)->R<()>{
 let mut options=fs::OpenOptions::new();options.create_new(true).write(true);
 #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;if private{options.mode(0o600);}}
 #[cfg(not(unix))]let _=private;
 let mut file=options.open(path).map_err(|e|format!("Cannot create {}: {e}",path.display()))?;file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|format!("Cannot save {}: {e}",path.display()))
}
/// SHA-256 of the complete SPKI DER. Panics if cert_der is not a valid X.509 certificate.
/// Load untrusted certificates with ParsedCertificate::try_from before calling this helper.
pub fn spki_fingerprint(cert_der:&[u8])->[u8;32]{let cert=CertificateDer::from(cert_der);let parsed=ParsedCertificate::try_from(&cert).expect("valid X.509 certificate");Sha256::digest(parsed.subject_public_key_info().as_ref()).into()}
pub fn format_spki_fingerprint(fingerprint:&[u8;32])->String{
 const ALPHABET:&[u8]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";let mut out=String::with_capacity(52);let mut bits=0;let mut buffer=0u32;
 for byte in fingerprint{buffer=(buffer<<8)|u32::from(*byte);bits+=8;while bits>=5{bits-=5;out.push(ALPHABET[((buffer>>bits)&31)as usize]as char);}}
 if bits>0{out.push(ALPHABET[((buffer<<(5-bits))&31)as usize]as char);}out
}
pub fn certificate_fingerprint(cert_der:&[u8])->String{Sha256::digest(cert_der).iter().map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(":")}

pub(crate) struct TlsListener{listener:TcpListener,acceptor:TlsAcceptor,pending:JoinSet<Option<(TlsStream<TcpStream>,SocketAddr)>>}
impl TlsListener{pub(crate) fn new(listener:TcpListener,config:Arc<rustls::ServerConfig>)->Self{Self{listener,acceptor:TlsAcceptor::from(config),pending:JoinSet::new()}}}
impl axum::serve::Listener for TlsListener{
 type Io=TlsStream<TcpStream>;type Addr=SocketAddr;
 async fn accept(&mut self)->(Self::Io,Self::Addr){
  loop{tokio::select!{
   result=self.pending.join_next(),if !self.pending.is_empty()=>{if let Some(Ok(Some(connection)))=result{return connection;}},
   result=self.listener.accept(),if self.pending.len()<256=>{match result{
    Ok((stream,peer))=>{let acceptor=self.acceptor.clone();self.pending.spawn(async move{match tokio::time::timeout(Duration::from_secs(10),acceptor.accept(stream)).await{Ok(Ok(tls))=>Some((tls,peer)),_=>None}});},
    Err(_)=>tokio::time::sleep(Duration::from_millis(100)).await
   }}
  }}
 }
 fn local_addr(&self)->std::io::Result<SocketAddr>{self.listener.local_addr()}
}
#[derive(Clone,Copy,Debug)]pub(crate) struct PeerAddr(pub SocketAddr);
impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_,TcpListener>> for PeerAddr{fn connect_info(stream:axum::serve::IncomingStream<'_,TcpListener>)->Self{Self(*stream.remote_addr())}}
impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_,TlsListener>> for PeerAddr{fn connect_info(stream:axum::serve::IncomingStream<'_,TlsListener>)->Self{Self(*stream.remote_addr())}}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn identity_is_persistent_private_and_never_repaired_silently(){
  let t=tempfile::tempdir().unwrap();let settings=crate::network::NetworkSettings::for_test("192.168.50.154".parse().unwrap(),14830);
  let first=LanTls::load_or_generate(t.path(),&settings).unwrap();let key=fs::read(t.path().join("lan-key.pem")).unwrap();let cert=fs::read(t.path().join("lan-cert.pem")).unwrap();let fingerprint=spki_fingerprint(&first.cert_der);
  let parsed_key=rcgen::KeyPair::from_pem(std::str::from_utf8(&key).unwrap()).unwrap();assert_eq!(fingerprint,<[u8;32]>::from(Sha256::digest(parsed_key.public_key_der())));
  assert_eq!(format_spki_fingerprint(&fingerprint).len(),52);assert!(format_spki_fingerprint(&fingerprint).bytes().all(|b|b.is_ascii_uppercase()||(b'2'..=b'7').contains(&b)));
  assert_eq!(format_spki_fingerprint(&[0;32]),"A".repeat(52));assert_eq!(format_spki_fingerprint(&[255;32]),format!("{}Q","7".repeat(51)));
  assert_eq!(certificate_fingerprint(&first.cert_der).len(),95);assert_eq!(certificate_fingerprint(b"abc"),"BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD");
  let second=LanTls::load_or_generate(t.path(),&settings).unwrap();assert_eq!(spki_fingerprint(&second.cert_der),fingerprint);assert_eq!(fs::read(t.path().join("lan-key.pem")).unwrap(),key);assert_eq!(fs::read(t.path().join("lan-cert.pem")).unwrap(),cert);
  #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(fs::metadata(t.path().join("lan-key.pem")).unwrap().permissions().mode()&0o777,0o600);}
  for file in ["lan-key.pem","lan-cert.pem"]{fs::write(t.path().join(file),b"damaged").unwrap();assert!(LanTls::load_or_generate(t.path(),&settings).is_err());assert_eq!(fs::read(t.path().join(file)).unwrap(),b"damaged");fs::write(t.path().join("lan-key.pem"),&key).unwrap();fs::write(t.path().join("lan-cert.pem"),&cert).unwrap();}
  fs::remove_file(t.path().join("lan-key.pem")).unwrap();assert!(LanTls::load_or_generate(t.path(),&settings).is_err());assert!(!t.path().join("lan-key.pem").exists());assert_eq!(fs::read(t.path().join("lan-cert.pem")).unwrap(),cert);
 }
 #[test]fn mismatched_or_unreadable_identity_is_not_overwritten(){
  let a=tempfile::tempdir().unwrap();let b=tempfile::tempdir().unwrap();let settings=crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830);
  LanTls::load_or_generate(a.path(),&settings).unwrap();LanTls::load_or_generate(b.path(),&settings).unwrap();let wrong_key=fs::read(b.path().join("lan-key.pem")).unwrap();fs::write(a.path().join("lan-key.pem"),&wrong_key).unwrap();assert!(LanTls::load_or_generate(a.path(),&settings).is_err());assert_eq!(fs::read(a.path().join("lan-key.pem")).unwrap(),wrong_key);
  fs::remove_file(a.path().join("lan-key.pem")).unwrap();fs::create_dir(a.path().join("lan-key.pem")).unwrap();assert!(LanTls::load_or_generate(a.path(),&settings).is_err());assert!(a.path().join("lan-key.pem").is_dir());
 }
 #[tokio::test]async fn tls12_and_tls13_verify_the_generated_certificate_without_sockets(){
  use tokio::io::{AsyncReadExt,AsyncWriteExt};
  let t=tempfile::tempdir().unwrap();let settings=crate::network::NetworkSettings::for_test("192.168.1.1".parse().unwrap(),14830);let tls=LanTls::load_or_generate(t.path(),&settings).unwrap();
  for version in [&rustls::version::TLS12,&rustls::version::TLS13]{
   let mut roots=rustls::RootCertStore::empty();roots.add(tls.cert_der.clone()).unwrap();
   let config=rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_protocol_versions(&[version]).unwrap().with_root_certificates(roots).with_no_client_auth();
   let (client,server)=tokio::io::duplex(8192);let acceptor=TlsAcceptor::from(tls.config.clone());
   let server=tokio::spawn(async move{let mut stream=acceptor.accept(server).await.unwrap();let mut request=[0;4];stream.read_exact(&mut request).await.unwrap();assert_eq!(&request,b"ping");stream.write_all(b"pong").await.unwrap();stream.shutdown().await.unwrap();});
   let connector=tokio_rustls::TlsConnector::from(Arc::new(config));let mut stream=connector.connect("nas.local".try_into().unwrap(),client).await.unwrap();assert_eq!(stream.get_ref().1.protocol_version(),Some(version.version));stream.write_all(b"ping").await.unwrap();let mut response=Vec::new();stream.read_to_end(&mut response).await.unwrap();assert_eq!(response,b"pong");server.await.unwrap();
  }
  let (mut client,server)=tokio::io::duplex(8192);client.write_all(b"GET /api/health HTTP/1.1\r\nHost: nas.local\r\n\r\n").await.unwrap();assert!(tokio::time::timeout(Duration::from_secs(1),TlsAcceptor::from(tls.config).accept(server)).await.unwrap().is_err());
 }

}

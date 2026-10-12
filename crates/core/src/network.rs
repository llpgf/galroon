//! Device-local LAN configuration. No catalog state or address-validation bypass.
use std::{fs,net::{IpAddr,SocketAddr},path::Path};
use serde::{Deserialize,Serialize};
type R<T>=Result<T,String>;
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSettings{version:u32,enabled:bool,address:IpAddr,port:u16,hostnames:Vec<String>}
impl NetworkSettings{
 pub fn load(device_dir:&Path)->R<Option<Self>>{
  let path=device_dir.join("network.json");crate::plans::validate_chain(&path)?;
  if fs::symlink_metadata(&path).is_ok_and(|m|m.file_type().is_symlink()){return Err("network.json must not be a symbolic link".into());}
  let bytes=match fs::read(&path){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(e)=>return Err(format!("Cannot read network.json: {e}"))};
  let settings:Self=serde_json::from_slice(&bytes).map_err(|e|format!("Invalid network.json: {e}"))?;settings.validate()?;Ok(Some(settings))
 }
 fn validate(&self)->R<()>{
  if self.version!=1{return Err("network.json: unsupported version (expected 1)".into());}
  validate_address(self.address)?;
  if self.port==0{return Err("network.json: port must be between 1 and 65535".into());}
  for host in &self.hostnames{if host.len()>253||host.parse::<IpAddr>().is_ok()||!host.split('.').all(|label|!label.is_empty()&&label.len()<=63&&!label.starts_with('-')&&!label.ends_with('-')&&label.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')){return Err(format!("network.json: invalid hostname {host:?}; use DNS labels without a scheme or port"));}}
  Ok(())
 }
 pub fn enabled(&self)->bool{self.enabled}
 pub fn socket_addr(&self)->SocketAddr{SocketAddr::new(self.address,self.port)}
 pub(crate) fn certificate_names(&self)->Vec<String>{let mut names=self.hostnames.clone();names.push(self.address.to_string());names}
 pub(crate) fn allows_host(&self,value:&str)->bool{
  let Ok(host)=value.parse::<axum::http::uri::Authority>()else{return false};
  if host.as_str().contains('@'){return false;}
  let name=host.host();let suffix=&value[name.len()..];if !suffix.is_empty()&&!suffix.strip_prefix(':').is_some_and(|p|p.parse::<u16>().is_ok_and(|p|p!=0)){return false;}
  let ip=name.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>();
  if ip.ok()==Some(self.address){return host.port().is_none()||host.port_u16()==Some(self.port);}
  self.hostnames.iter().any(|allowed|allowed.eq_ignore_ascii_case(name))
 }
 #[cfg(test)]pub(crate) fn for_test(address:IpAddr,port:u16)->Self{Self{version:1,enabled:true,address,port,hostnames:vec!["nas.local".into()]}}
}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkEdit{pub address:IpAddr,#[serde(default="default_port")]pub port:u16,#[serde(default)]pub hostnames:Vec<String>,pub enabled:Option<bool>}
fn default_port()->u16{14830}
pub fn show(device_dir:&Path)->R<serde_json::Value>{
 let settings=NetworkSettings::load(device_dir)?;let mut value=serde_json::json!({"settings":settings,"status":if settings.is_some(){"configured"}else{"not configured"}});
 if let Some((spki,certificate))=crate::lan_tls::existing_fingerprints(device_dir)?{value["spki_fingerprint"]=serde_json::json!(spki);value["certificate_sha256"]=serde_json::json!(certificate);}Ok(value)
}
pub fn save(device_dir:&Path,edit:NetworkEdit)->R<NetworkSettings>{
 let previous=NetworkSettings::load(device_dir)?;
 let settings=NetworkSettings{version:1,enabled:edit.enabled.unwrap_or_else(||previous.as_ref().is_some_and(|n|n.enabled)),address:edit.address,port:edit.port,hostnames:edit.hostnames};settings.validate()?;
 let path=device_dir.join("network.json");crate::plans::validate_chain(&path)?;
 let temp=device_dir.join(format!(".network-{}.tmp",crate::db::id()));
 let result=(||{use std::io::Write;let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
  file.write_all(&serde_json::to_vec_pretty(&settings).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
  fs::rename(&temp,&path).map_err(|e|e.to_string())
 })();if result.is_err(){let _=fs::remove_file(&temp);}result?;Ok(settings)
}
pub fn validate_address(address:IpAddr)->R<()>{
 let reason=if address.is_unspecified(){"wildcard addresses are not allowed"}else if address.is_loopback(){"loopback addresses are not allowed"}else if address.is_multicast(){"multicast addresses are not allowed"}else{match address{
  IpAddr::V4(ip)=>{let b=ip.octets();if ip.is_private()||(b[0]==100&&(64..128).contains(&b[1])){return Ok(());}
   if ip.is_link_local(){"link-local addresses are not allowed"}else{"only RFC 1918, 100.64.0.0/10 and IPv6 ULA addresses are allowed; public and other ranges are rejected"}},
  IpAddr::V6(ip)=>{let b=ip.octets();if b[0]&0xfe==0xfc{return Ok(());}
   if b[0]==0xfe&&b[1]&0xc0==0x80{"IPv6 link-local addresses are not supported (scope IDs are client-local)"}else{"only RFC 1918, 100.64.0.0/10 and IPv6 ULA addresses are allowed; public and other ranges are rejected"}}
 }};Err(format!("network.json: LAN address {address}: {reason}"))
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn strict_device_settings_and_private_addresses(){
  let t=tempfile::tempdir().unwrap();assert!(NetworkSettings::load(t.path()).unwrap().is_none());
  let valid=serde_json::json!({"version":1,"enabled":false,"address":"192.168.50.154","port":14830,"hostnames":["nas.local"]});
  let path=t.path().join("network.json");fs::write(&path,valid.to_string()).unwrap();assert!(!NetworkSettings::load(t.path()).unwrap().unwrap().enabled());
  for field in ["version","enabled","address","port","hostnames"]{let mut value=valid.clone();value.as_object_mut().unwrap().remove(field);fs::write(&path,value.to_string()).unwrap();assert!(NetworkSettings::load(t.path()).is_err(),"missing {field}");}
  for (field,value) in [("extra",serde_json::json!(true)),("version",serde_json::json!(2)),("address",serde_json::json!("not-ip")),("port",serde_json::json!(0)),("hostnames",serde_json::json!(["https://nas.local"]))]{let mut bad=valid.clone();bad[field]=value;fs::write(&path,bad.to_string()).unwrap();assert!(NetworkSettings::load(t.path()).is_err());}
  fs::write(&path,b"broken").unwrap();assert!(NetworkSettings::load(t.path()).is_err());
  for address in ["192.168.50.154","100.64.1.2","10.0.0.1","172.16.0.1","172.31.255.255","100.127.255.255","fd00::1","fc00::1"]{validate_address(address.parse().unwrap()).unwrap();}
  for (address,reason) in [("8.8.8.8","public"),("172.32.0.1","public"),("100.128.0.1","public"),("100.63.255.255","public"),("169.254.1.2","link-local"),("fe80::1","link-local"),("127.0.0.1","loopback"),("::1","loopback"),("0.0.0.0","wildcard"),("::","wildcard"),("ff02::1","multicast"),("224.0.0.1","multicast"),("2001:4860::1","public"),("::ffff:192.168.1.1","rejected")]{assert!(validate_address(address.parse().unwrap()).unwrap_err().contains(reason),"{address}");}
 }
 #[test]fn hosts_are_exact_and_ipv6_is_bracketed(){
  for address in ["192.168.50.154","fd00::1"]{let settings=NetworkSettings::for_test(address.parse().unwrap(),14830);let ip=if address.contains(':'){format!("[{address}]")}else{address.into()};
   for host in [ip.clone(),format!("{ip}:14830"),"NAS.Local".into(),"nas.local:443".into()]{assert!(settings.allows_host(&host),"{host}");}
   for host in [format!("{ip}:443"),"evil.local".into(),"nas.local.evil".into(),"user@nas.local".into(),"nas.local:garbage".into(),"nas.local:65536".into(),"nas.local:".into(),"nas.local:0".into()]{assert!(!settings.allows_host(&host),"{host}");}
  }
 }
 #[test]fn settings_belong_to_device_and_invalid_disabled_settings_abort_initialize(){
  let t=tempfile::tempdir().unwrap();let device=t.path().join("device");fs::create_dir(&device).unwrap();
  fs::write(device.join("network.json"),r#"{"version":1,"enabled":false,"address":"192.168.1.1","port":14830,"hostnames":[]}"#).unwrap();
  let app=crate::initialize_with_device(t.path().join("catalog"),&device).unwrap();assert_eq!(app.device_dir,device);assert!(!app.network.as_ref().unwrap().enabled());assert!(!app.state_dir.join("network.json").exists());assert!(!device.join("lan-key.pem").exists());drop(app);
  fs::write(device.join("network.json"),r#"{"version":1,"enabled":false,"address":"127.0.0.1","port":14830,"hostnames":[]}"#).unwrap();assert!(crate::initialize_with_device(t.path().join("catalog"),&device).is_err());
 }
}

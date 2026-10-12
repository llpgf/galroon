//! Shared pairing-string grammar; no application or transport state.
use std::net::{Ipv4Addr,Ipv6Addr};
type R<T>=Result<T,String>;
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PairingString{pub host:String,pub port:u16,pub code:String,pub fingerprint:String}
/// Host is an IPv4 address, bracketed IPv6 address, or lowercase ASCII DNS name.
pub fn format_pairing_string(host:&str,port:u16,code:&str,fingerprint:&str)->R<String>{
 let value=format!("galroon-pair:v1?addr={host}:{port}&code={code}&fp={fingerprint}");parse_pairing_string(&value)?;Ok(value)
}
pub fn parse_pairing_string(value:&str)->R<PairingString>{
 if value.len()>256{return Err("Pairing string exceeds 256 bytes".into());}
 let query=value.strip_prefix("galroon-pair:v1?").ok_or("Unsupported pairing string")?;
 let(mut addr,mut code,mut fingerprint)=(None,None,None);
 for part in query.split('&'){let(key,value)=part.split_once('=').ok_or("Malformed pairing field")?;let slot=match key{"addr"=>&mut addr,"code"=>&mut code,"fp"=>&mut fingerprint,_=>return Err("Unknown pairing field".into())};if slot.replace(value).is_some(){return Err("Duplicate pairing field".into());}}
 let addr=addr.ok_or("Missing pairing address")?;let code=code.ok_or("Missing pairing code")?;let fingerprint=fingerprint.ok_or("Missing pairing fingerprint")?;
 let(host,port)=addr.rsplit_once(':').ok_or("Pairing address requires a port")?;
 if host.contains('%'){return Err("IPv6 zone IDs are not supported".into());}
 if !host.is_ascii(){return Err("Unicode hostnames are not supported; IDNA isn't supported yet".into());}
 if host.starts_with('['){host.strip_prefix('[').and_then(|s|s.strip_suffix(']')).and_then(|s|s.parse::<Ipv6Addr>().ok()).ok_or("Invalid bracketed IPv6 address")?;}
 else if host.parse::<Ipv4Addr>().is_err()&&(host.is_empty()||host.len()>253||host.bytes().all(|b|b.is_ascii_digit()||b==b'.')||!host.split('.').all(|s|!s.is_empty()&&s.len()<=63&&!s.starts_with('-')&&!s.ends_with('-')&&s.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-'))){return Err("Invalid host: use lowercase LDH hostname, IPv4, or bracketed IPv6".into());}
 if port.is_empty()||!port.bytes().all(|b|b.is_ascii_digit()){return Err("Invalid pairing port".into());}let port=port.parse::<u16>().ok().filter(|p|*p!=0).ok_or("Invalid pairing port")?;
 if code.len()!=12||!code.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Pairing code must be 12 hex characters".into());}
 // SHA-256 has one significant bit in its final unpadded base32 character.
 if fingerprint.len()!=52||!fingerprint.bytes().all(|b|b.is_ascii_uppercase()||(b'2'..=b'7').contains(&b))||!matches!(fingerprint.as_bytes()[51],b'A'|b'Q'){return Err("Fingerprint must be canonical 52-character base32 SHA-256".into());}
 Ok(PairingString{host:host.into(),port,code:code.into(),fingerprint:fingerprint.into()})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn round_trip_and_strict_grammar(){
  let fp=crate::lan_tls::format_spki_fingerprint(&[255;32]);
  for host in ["192.168.1.2","[fd00::1234]","nas-1.local"]{let text=format_pairing_string(host,14830,"AB12CD34EF56",&fp).unwrap();let parsed=parse_pairing_string(&text).unwrap();assert_eq!(parsed,PairingString{host:host.into(),port:14830,code:"AB12CD34EF56".into(),fingerprint:fp.clone()});assert_eq!(format_pairing_string(&parsed.host,parsed.port,&parsed.code,&parsed.fingerprint).unwrap(),text);}
  let text=format_pairing_string("nas.local",14830,"AB12CD34EF56",&fp).unwrap();
  for bad in [format!("{text}&extra=x"),format!("{text}&code=AB12CD34EF56"),text.replace("&code=AB12CD34EF56",""),text.replace("addr=nas.local:14830&",""),text.replace(&format!("&fp={fp}"),""),text.replace(&fp,&fp[..51]),text.replace(&fp,&"0".repeat(52)),text.replace(&fp,&"B".repeat(52)),text.replace("nas.local","[fe80::1%en0]"),text.replace("nas.local","[fe80::1%25en0]"),text.replace("nas.local","fd00::1"),text.replace("nas.local","NAS.local"),text.replace("nas.local","bad_host"),text.replace("nas.local","999.1.1.1"),text.replace(":14830",":0"),text.replace(":14830",":65536"),text.replace("AB12CD34EF56","AB12-CD34-EF56"),"a".repeat(257)]{assert!(parse_pairing_string(&bad).is_err(),"{bad}");}
  assert!(parse_pairing_string(&text.replace("nas.local","主機.local")).unwrap_err().contains("IDNA"));
 }
}

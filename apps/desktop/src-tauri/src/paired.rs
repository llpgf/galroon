//! Saved connection metadata contains no secrets. Tokens live in Windows Credential Manager.
use galroon_core::{db, local_core::Session, plans};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, time::Duration};
type R<T> = Result<T, String>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String, pub name: String, pub url: String,
    pub library_id: String, pub device_id: String,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Connections { pub selected: Option<String>, pub entries: Vec<Connection> }
pub fn read(base: &Path) -> R<Connections> {
    let path = base.join("connections.json"); plans::validate_chain(&path)?;
    if !path.exists() { return Ok(Connections::default()); }
    if fs::metadata(&path).map_err(|e|e.to_string())?.len() > 65536 { return Err("Connection settings are too large".into()); }
    let value: Connections = serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|_|"Connection settings are damaged; no fallback collection was opened")?;
    if value.entries.len()>32 || value.entries.iter().any(|e| !valid_id(&e.id)) || value.selected.as_ref().is_some_and(|id|!value.entries.iter().any(|e|&e.id==id)) {return Err("Invalid saved connection settings".into());}
    Ok(value)
}
pub fn save(base: &Path, value: &Connections) -> R<()> {
    plans::validate_chain(base)?; fs::create_dir_all(base).map_err(|e|e.to_string())?;
    let path=base.join("connections.json"); plans::validate_chain(&path)?;
    let temp=base.join(format!("connections-{}.tmp",db::id()));
    let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(|e|e.to_string())?;
    file.write_all(&serde_json::to_vec(value).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);
    fs::rename(temp,path).map_err(|e|e.to_string())
}
fn valid_id(id: &str)->bool {id.len()==36 && id.bytes().all(|b|b.is_ascii_hexdigit()||b==b'-')}
pub fn endpoint(raw:&str)->R<String> {
    let u=reqwest::Url::parse(raw.trim()).map_err(|_|"Enter a Core URL such as http://127.0.0.1:14800")?;
    // LAN/TLS deployment is a separate product boundary. Do not widen the listener or CSP here.
    if u.scheme()!="http" || !matches!(u.host_str(),Some("127.0.0.1"|"localhost")) || !u.username().is_empty() || u.password().is_some() || u.path()!="/" || u.query().is_some() || u.fragment().is_some() {
        return Err("This Windows build supports loopback Core connections only (http://127.0.0.1:port). NAS/LAN deployment is not enabled.".into());
    }
    let port=u.port_or_known_default().ok_or("Core port is missing")?;if port==0{return Err("Core port must not be zero".into());}
    Ok(format!("http://127.0.0.1:{port}"))
}
fn client()->R<reqwest::Client>{reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(10)).build().map_err(|e|e.to_string())}
async fn response(r:reqwest::Response)->R<Value>{
    if !r.status().is_success(){return Err(format!("Core request failed ({}). Check the connection or pair again.",r.status().as_u16()));}
    if r.content_length().is_some_and(|n|n>65536){return Err("Core response is too large".into());}
    let mut r=r;let mut body=Vec::new();while let Some(chunk)=r.chunk().await.map_err(|_|"Core response interrupted")?{if body.len()+chunk.len()>65536{return Err("Core response is too large".into());}body.extend_from_slice(&chunk);}
    serde_json::from_slice(&body).map_err(|_|"Core returned an invalid response".into())
}
fn field(v:&Value,key:&str)->R<String>{v[key].as_str().filter(|s|!s.is_empty()&&s.len()<1024).map(str::to_owned).ok_or_else(||format!("Core response is missing {key}"))}
async fn context(url:&str,token:&str)->R<Value>{response(client()?.get(format!("{url}/api/context")).bearer_auth(token).send().await.map_err(|_|"Core is unavailable. The selected collection has not changed.")?).await}
fn session(url:String,token:String,v:&Value)->R<Session>{
    let min=v["api"]["min"].as_u64().ok_or("Missing API compatibility")?;let max=v["api"]["max"].as_u64().ok_or("Missing API compatibility")?;
    if min>1||max<1{return Err("Core and desktop API versions are incompatible".into());}
    Ok(Session{url,token,library_id:field(&v["library"],"id")?,device_id:field(&v["core"]["device"],"id")?,api_min:1,api_max:1,instance_id:field(&v["core"],"instance_id")?,pid:0})
}
pub async fn connect(entry:&Connection)->R<Session>{
    let url=endpoint(&entry.url)?;let token=credential_read(&entry.id)?;
    let s=session(url.clone(),token.clone(),&context(&url,&token).await?)?;
    if s.library_id!=entry.library_id||s.device_id!=entry.device_id{return Err("A different collection or Core device answered. Pair it explicitly; the saved connection was not replaced.".into());}Ok(s)
}
/// Recover an explicitly supplied address without replacing the pinned identity or credential.
/// Failed validation leaves the entire saved selection and endpoint unchanged.
pub async fn update_endpoint(base:&Path,id:&str,raw:&str)->R<Connections>{
    let mut saved=read(base)?;
    let entry=saved.entries.iter_mut().find(|entry|entry.id==id).ok_or("Connection not found")?;
    let mut candidate=entry.clone();candidate.url=endpoint(raw)?;
    connect(&candidate).await?;
    entry.url=candidate.url;
    save(base,&saved)?;Ok(saved)
}
pub async fn pair(base:&Path,raw:&str,code:&str)->R<Connections>{
    let mut saved=read(base)?;if saved.entries.len()>=32{return Err("Remove an unused connection before adding another".into());}
    let url=endpoint(raw)?;let code=code.trim().replace('-',"").to_uppercase();
    if code.len()!=12||!code.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Enter the 12-character pairing code".into());}
    let reply=response(client()?.post(format!("{url}/api/access/pair")).header("Origin",&url).json(&json!({"code":code})).send().await.map_err(|_|"Pairing response was interrupted. Obtain a new code; it was not retried automatically.")?).await?;
    let token=field(&reply,"token")?;
    let result=async {
        let v=context(&url,&token).await?;let s=session(url.clone(),token.clone(),&v)?;
        let entry=Connection{id:db::id(),name:field(&v["library"],"name")?,url:url.clone(),library_id:s.library_id,device_id:s.device_id};
        credential_write(&entry.id,&token)?;let id=entry.id.clone();saved.entries.push(entry);
        if let Err(e)=save(base,&saved){let _=credential_delete(&id);return Err(e);}Ok(saved)
    }.await;
    if result.is_err(){let _=client()?.post(format!("{url}/api/access/logout")).header("Origin",&url).bearer_auth(&token).json(&json!({})).send().await;}
    result
}
pub fn forget(base:&Path,id:&str)->R<Connections>{
    let mut saved=read(base)?;if saved.selected.as_deref()==Some(id){return Err("Switch to another collection before removing this connection".into());}
    if !saved.entries.iter().any(|e|e.id==id){return Err("Connection not found".into());}
    credential_delete(id)?;saved.entries.retain(|e|e.id!=id);save(base,&saved)?;Ok(saved)
}
pub async fn remove(base:&Path,id:&str)->R<Connections>{
    let mut saved=read(base)?;if saved.selected.as_deref()==Some(id){return Err("Switch to another collection before removing this connection".into());}
    let entry=saved.entries.iter().find(|e|e.id==id).ok_or("Connection not found")?;
    // Identity must match before revocation; failures preserve the saved entry for retry.
    let s=connect(entry).await?;
    response(client()?.post(format!("{}/api/access/logout",s.url)).header("Origin",&s.url).bearer_auth(&s.token).json(&json!({})).send().await.map_err(|_|"Could not confirm revocation; the connection remains saved")?).await?;
    credential_delete(id)?;saved.entries.retain(|e|e.id!=id);save(base,&saved)?;Ok(saved)
}
#[cfg(windows)]fn target(id:&str)->R<Vec<u16>>{if !valid_id(id){return Err("Invalid credential reference".into());}Ok(format!("Galroon/paired/{id}\0").encode_utf16().collect())}
#[cfg(windows)]fn credential_write(id:&str,token:&str)->R<()>{
    use windows_sys::Win32::Security::Credentials::*;
    let mut target=target(id)?;let mut user:Vec<u16>="Galroon\0".encode_utf16().collect();let mut blob=token.as_bytes().to_vec();
    let c=CREDENTIALW{Type:CRED_TYPE_GENERIC,TargetName:target.as_mut_ptr(),CredentialBlobSize:blob.len() as u32,CredentialBlob:blob.as_mut_ptr(),Persist:CRED_PERSIST_LOCAL_MACHINE,UserName:user.as_mut_ptr(),..Default::default()};
    let ok=unsafe{CredWriteW(&c,0)};blob.fill(0);if ok==0{return Err("Windows Credential Manager could not save the pairing".into());}Ok(())
}
#[cfg(windows)]fn credential_read(id:&str)->R<String>{
    use windows_sys::Win32::Security::Credentials::*;
    let target=target(id)?;let mut ptr=std::ptr::null_mut();if unsafe{CredReadW(target.as_ptr(),CRED_TYPE_GENERIC,0,&mut ptr)}==0{return Err("Pairing credential is unavailable. Pair this Core again.".into());}
    let result=unsafe {let c=&*ptr;if c.CredentialBlobSize==0||c.CredentialBlobSize>4096||c.CredentialBlob.is_null(){Err("Invalid saved pairing credential".into())}else{String::from_utf8(std::slice::from_raw_parts(c.CredentialBlob,c.CredentialBlobSize as usize).to_vec()).map_err(|_|"Invalid saved pairing credential".into())}};
    unsafe{CredFree(ptr.cast())};result
}
#[cfg(windows)]fn credential_delete(id:&str)->R<()>{
    use windows_sys::Win32::{Security::Credentials::*,Foundation::{GetLastError,ERROR_NOT_FOUND}};
    let t=target(id)?;if unsafe{CredDeleteW(t.as_ptr(),CRED_TYPE_GENERIC,0)}==0 && unsafe{GetLastError()}!=ERROR_NOT_FOUND{return Err("Windows could not remove the saved credential".into());}Ok(())
}
#[cfg(not(windows))]fn credential_read(_: &str)->R<String>{Err("Pairing credentials require Windows".into())}
#[cfg(not(windows))]fn credential_write(_: &str,_:&str)->R<()>{Err("Pairing credentials require Windows".into())}
#[cfg(not(windows))]fn credential_delete(_: &str)->R<()>{Err("Pairing credentials require Windows".into())}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn endpoints_do_not_expand_access(){assert_eq!(endpoint("http://localhost:14800/").unwrap(),"http://127.0.0.1:14800");for bad in ["https://example.com","http://192.168.1.2:14800","http://127.0.0.1:0","http://user@127.0.0.1","http://127.0.0.1/path","http://127.0.0.1/?x=y"]{assert!(endpoint(bad).is_err(),"{bad}");}}
    #[test]fn malformed_selection_never_falls_back(){let t=tempfile::tempdir().unwrap();save(t.path(),&Connections{selected:Some("missing".into()),entries:vec![]}).unwrap();assert!(read(t.path()).is_err());}
    #[cfg(windows)]#[test]fn windows_credential_roundtrip(){let id=db::id();credential_write(&id,"generated-test-token").unwrap();let actual=credential_read(&id).unwrap();let deleted=credential_delete(&id);assert!(actual=="generated-test-token");deleted.unwrap();assert!(credential_read(&id).is_err());}
}

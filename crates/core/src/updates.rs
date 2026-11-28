//! Device-local release notices and signed NAS Docker self-updates.
use std::{fs,io::{Read,Write},path::{Component,Path,PathBuf},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
use axum::{extract::State,http::HeaderMap,Json};
use futures_util::StreamExt;
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use crate::{App,ApiError};
type R<T>=Result<T,String>;
struct Checking<'a>(&'a AtomicBool);
impl Drop for Checking<'_>{fn drop(&mut self){self.0.store(false,Ordering::Release);}}
const DAY:i64=24*60*60;
const RELEASES:&str="https://github.com/llpgf/galroon/releases";
const DOWNLOAD_BASE:&str="https://github.com/llpgf/galroon/releases/download";
pub const UPDATE_PUBKEY:&str="RWRBhE8CXpD1tWkT7vhtdZQWZ2yG9pOvoL3Wc38zqN03CAuHSnu+4MRu";
pub const UPDATE_EXIT_CODE:i32=42;
const MAX_MANIFEST_BYTES:usize=64*1024;
const MAX_SIG_BYTES:usize=16*1024;
const MAX_ARCHIVE_BYTES:u64=256*1024*1024;
const MAX_UNPACKED_BYTES:u64=512*1024*1024;
const MAX_TAR_ENTRIES:usize=20_000;
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release{version:String,name:String,url:String,notes:String,published_at:Option<String>}
#[derive(Clone,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
struct Stored{version:u32,enabled:bool,last_checked:Option<i64>,last_error:Option<String>,etag:Option<String>,latest:Option<Release>}
impl Default for Stored{fn default()->Self{Self{version:1,enabled:true,last_checked:None,last_error:None,etag:None,latest:None}}}
#[derive(Clone,Debug,Serialize)]
pub struct ApplyStatus{pub state:&'static str,pub target_version:Option<String>,pub downloaded_bytes:u64,pub total_bytes:Option<u64>,pub error:Option<String>}
impl Default for ApplyStatus{fn default()->Self{Self{state:"idle",target_version:None,downloaded_bytes:0,total_bytes:None,error:None}}}
impl ApplyStatus{fn is_active(&self)->bool{matches!(self.state,"downloading"|"verifying"|"staging"|"restarting")}}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct NasManifest{pub version:String,pub url:String,pub sig_url:String,pub sha256:String,pub size:u64}
pub struct Updates{path:PathBuf,stored:Mutex<Stored>,apply:Mutex<ApplyStatus>,gate:Arc<tokio::sync::Mutex<()>>,wake:tokio::sync::Notify,checking:AtomicBool}
fn validate(path:&Path)->R<()>{crate::plans::validate_chain(path)?;if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){return Err("update-check.json must not be a symbolic link".into());}Ok(())}
fn load(path:&Path)->R<Stored>{
 validate(path)?;let bytes=match fs::read(path){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Stored::default()),Err(e)=>return Err(e.to_string())};
 let mut value:Stored=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;if value.version!=1{return Err("Unsupported update-check.json version".into());}
 if let Some(latest)=&mut value.latest{latest.url=trusted_url(&latest.url);latest.notes=latest.notes.chars().take(20_000).collect();}Ok(value)
}
fn save(path:&Path,value:&Stored)->R<()>{
 validate(path)?;let temp=path.with_file_name(format!(".update-check-{}.tmp",crate::db::id()));crate::plans::validate_chain(&temp)?;
 let result=(||{let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
  file.write_all(&serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);validate(path)?;fs::rename(&temp,path).map_err(|e|e.to_string())
 })();if result.is_err(){let _=fs::remove_file(temp);}result
}
fn write_atomic_text(path:&Path,text:&str)->R<()>{
 if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;crate::plans::validate_chain(parent)?;}
 if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){return Err("Refusing to write through symbolic link".into());}
 let name=path.file_name().and_then(|n|n.to_str()).unwrap_or("file");
 let temp=path.with_file_name(format!(".{name}-{}.tmp",crate::db::id()));
 let result=(||{let mut f=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
  f.write_all(text.as_bytes()).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;drop(f);fs::rename(&temp,path).map_err(|e|e.to_string())
 })();if result.is_err(){let _=fs::remove_file(temp);}result
}
fn trusted_url(url:&str)->String{if url.starts_with("https://github.com/llpgf/galroon/releases/"){url.into()}else{RELEASES.into()}}
fn full_version(tag:&str)->Option<semver::Version>{semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok().filter(|v|v.pre.is_empty())}
fn is_newer_version(candidate:&str,current:&str)->bool{
 match(full_version(candidate),semver::Version::parse(current).ok()){(Some(v),Some(c))=>v.cmp_precedence(&c).is_gt(),_=>false}
}
fn newer(release:&Release)->bool{is_newer_version(&release.version,env!("CARGO_PKG_VERSION"))}
fn api_base()->String{std::env::var("GALROON_UPDATE_API").unwrap_or_else(|_|"https://api.github.com".into())}
fn download_base()->String{std::env::var("GALROON_UPDATE_DOWNLOAD_BASE").unwrap_or_else(|_|DOWNLOAD_BASE.into())}
fn install_kind()->&'static str{if std::env::var("GALROON_INSTALL_KIND").is_ok_and(|v|v=="docker"){"docker"}else if cfg!(windows){"desktop"}else{"standalone"}}
fn runtime_dir()->Option<PathBuf>{std::env::var_os("GALROON_RUNTIME_DIR").filter(|v|!v.is_empty()).map(PathBuf::from)}
pub fn can_self_update()->bool{install_kind()=="docker"&&std::env::var("GALROON_LAUNCHER").is_ok_and(|v|v=="1")&&runtime_dir().is_some()}
fn read_trimmed(path:&Path)->Option<String>{
 if fs::symlink_metadata(path).ok().is_some_and(|m|m.file_type().is_symlink()){return None;}
 fs::read_to_string(path).ok().map(|s|s.trim().to_owned()).filter(|s|!s.is_empty())
}
fn consume_last_failed(rt:&Path)->Option<ApplyStatus>{
 let path=rt.join("last-failed");
 if fs::symlink_metadata(&path).ok().is_some_and(|m|m.file_type().is_symlink()){return None;}
 let content=fs::read_to_string(&path).ok()?;
 let _=fs::remove_file(&path);
 let mut lines=content.lines();
 let ver=lines.next()?.trim().to_owned();
 let msg=lines.next().map(|l|l.trim().to_owned()).filter(|s|!s.is_empty()).unwrap_or_else(||format!("Update to {ver} failed during startup and rolled back automatically."));
 crate::diagnostics::record("warn","updates.rollback",&msg,None);
 Some(ApplyStatus{state:"failed",target_version:if ver.is_empty(){None}else{Some(ver)},downloaded_bytes:0,total_bytes:None,error:Some(msg)})
}
pub fn mark_runtime_healthy(){
 if !std::env::var("GALROON_LAUNCHER").is_ok_and(|v|v=="1"){return;}
 let Some(rt)=runtime_dir() else{return;};
 let _=mark_runtime_healthy_at(&rt,env!("CARGO_PKG_VERSION"));
}
pub(crate) fn mark_runtime_healthy_at(rt:&Path,current:&str)->R<()>{
 let attempt=read_trimmed(&rt.join("attempt"));
 if attempt.as_deref()==Some(current){
  write_atomic_text(&rt.join("healthy"),&format!("{current}\n"))?;
  let previous=read_trimmed(&rt.join("current")).filter(|v|v!=current);
  if let Some(prev)=&previous{let _=write_atomic_text(&rt.join("previous"),&format!("{prev}\n"));}
  write_atomic_text(&rt.join("current"),&format!("{current}\n"))?;
  let _=fs::remove_file(rt.join("attempt"));
  prune_releases(rt,current,previous.as_deref());
 }else if rt.join("releases").join(current).join("galroon-core").is_file(){
  write_atomic_text(&rt.join("healthy"),&format!("{current}\n"))?;
 }
 Ok(())
}
fn prune_releases(rt:&Path,keep_current:&str,keep_previous:Option<&str>){
 let releases=rt.join("releases");
 let Ok(entries)=fs::read_dir(&releases) else{return;};
 for entry in entries.flatten(){
  let Some(name)=entry.file_name().to_str().map(str::to_owned) else{continue;};
  if name==keep_current||keep_previous==Some(name.as_str()){continue;}
  let _=fs::remove_dir_all(entry.path());
 }
}
fn valid_version_segment(v:&str)->bool{
 !v.is_empty()&&!v.contains('/')&&!v.contains('\\')&&!v.contains("..")&&full_version(v).is_some()
}
fn is_trusted_download_url(url:&str,base:&str)->bool{
 let prefix=format!("{}/",base.trim_end_matches('/'));
 url.starts_with(&prefix)
}
fn build_download_client(base:&str,timeout:Duration)->R<reqwest::Client>{
 let local_test_host=reqwest::Url::parse(base).ok().filter(|u|u.scheme()=="http"&&matches!(u.host_str(),Some("127.0.0.1"|"localhost"))).and_then(|u|u.host_str().map(str::to_owned));
 let policy=reqwest::redirect::Policy::custom(move|attempt|{
  if attempt.previous().len()>=5{return attempt.error("Too many redirects");}
  let url=attempt.url();
  if let Some(local)=&local_test_host{
   if url.scheme()=="http"&&url.host_str()==Some(local.as_str()){return attempt.follow();}
  }
  if url.scheme()=="https"&&matches!(url.host_str(),Some("github.com"|"objects.githubusercontent.com"|"release-assets.githubusercontent.com"|"codeload.github.com")){
   return attempt.follow();
  }
  attempt.error("Blocked redirect to untrusted host")
 });
 reqwest::Client::builder().timeout(timeout).redirect(policy).build().map_err(|e|e.to_string())
}
fn decode_base64(input:&str)->Option<Vec<u8>>{
 let mut out=Vec::new();let mut buf=0u32;let mut bits=0u8;
 for b in input.bytes(){
  if b.is_ascii_whitespace(){continue;}
  if b==b'=' {break;}
  let v=match b{b'A'..=b'Z'=>b-b'A',b'a'..=b'z'=>b-b'a'+26,b'0'..=b'9'=>b-b'0'+52,b'+'=>62,b'/'=>63,_=>return None};
  buf=(buf<<6)|(v as u32);bits+=6;
  if bits>=8{bits-=8;out.push((buf>>bits) as u8);buf&=(1<<bits)-1;}
 }
 Some(out)
}
fn decode_signature(sig_text:&str)->R<minisign_verify::Signature>{
 if let Ok(sig)=minisign_verify::Signature::decode(sig_text){return Ok(sig);}
 // `tauri signer sign` writes a base64-encoded wrapper around the 4-line minisign box.
 let decoded=decode_base64(sig_text.trim()).and_then(|b|String::from_utf8(b).ok()).ok_or_else(||"Invalid update signature format".to_string())?;
 minisign_verify::Signature::decode(&decoded).map_err(|_|"Invalid update signature format".to_string())
}
fn verify_signature_bytes(pubkey_b64:&str,sig_text:&str,data:&[u8])->R<()>{
 let pk=minisign_verify::PublicKey::from_base64(pubkey_b64).map_err(|_|"Invalid embedded update public key".to_string())?;
 let sig=decode_signature(sig_text)?;
 pk.verify(data,&sig,true).map_err(|_|"Update signature verification failed".to_string())
}
fn safe_extract_tar_gz(archive_path:&Path,dest_dir:&Path)->R<()>{
 fs::create_dir_all(dest_dir).map_err(|e|format!("Cannot create staging directory: {e}"))?;
 let file=fs::File::open(archive_path).map_err(|_|"Cannot open downloaded update archive".to_string())?;
 let decoder=flate2::read::GzDecoder::new(file);
 let mut archive=tar::Archive::new(decoder);
 let mut count=0usize;
 let mut total_bytes=0u64;
 let entries=archive.entries().map_err(|_|"Update archive is corrupted".to_string())?;
 for entry in entries{
  count+=1;if count>MAX_TAR_ENTRIES{return Err("Update archive has too many entries".into());}
  let mut entry=entry.map_err(|_|"Update archive entry is corrupted".to_string())?;
  let etype=entry.header().entry_type();
  if !etype.is_file()&&!etype.is_dir(){return Err("Update archive contains unsupported file type".into());}
  let rel=entry.path().map_err(|_|"Update archive contains invalid path".to_string())?.into_owned();
  let mut clean=PathBuf::new();
  for comp in rel.components(){
   match comp{
    Component::Normal(seg)=>clean.push(seg),
    Component::CurDir=>{},
    Component::ParentDir|Component::RootDir|Component::Prefix(_)=>return Err("Update archive contains unsafe path".into()),
   }
  }
  if clean.as_os_str().is_empty(){continue;}
  let out=dest_dir.join(&clean);
  if etype.is_dir(){
   fs::create_dir_all(&out).map_err(|_|"Cannot create directory during update extraction".to_string())?;
  }else{
   if let Some(parent)=out.parent(){fs::create_dir_all(parent).map_err(|_|"Cannot create parent directory during update extraction".to_string())?;}
   if fs::symlink_metadata(&out).is_ok_and(|m|m.file_type().is_symlink()){return Err("Update target contains symbolic link".into());}
   let mut dst=fs::OpenOptions::new().write(true).create_new(true).open(&out).map_err(|_|"Cannot write extracted update file".to_string())?;
   let mut buf=[0u8;32*1024];
   loop{
    let n=entry.read(&mut buf).map_err(|_|"Failed reading update archive".to_string())?;
    if n==0{break;}
    total_bytes=total_bytes.saturating_add(n as u64);
    if total_bytes>MAX_UNPACKED_BYTES{return Err("Update archive exceeds unpacked size limit".into());}
    dst.write_all(&buf[..n]).map_err(|_|"Failed writing extracted update file".to_string())?;
   }
   dst.sync_all().map_err(|_|"Failed flushing extracted update file".to_string())?;
  }
 }
 let core_bin=dest_dir.join("galroon-core");
 let web_index=dest_dir.join("web").join("index.html");
 let core_meta=fs::symlink_metadata(&core_bin).map_err(|_|"Update archive is missing galroon-core".to_string())?;
 if !core_meta.is_file()||core_meta.len()==0{return Err("Update archive contains invalid galroon-core".into());}
 let web_meta=fs::symlink_metadata(&web_index).map_err(|_|"Update archive is missing web UI assets".to_string())?;
 if !web_meta.is_file()||web_meta.len()==0{return Err("Update archive contains invalid web UI assets".into());}
 #[cfg(unix)]
 {
  use std::os::unix::fs::PermissionsExt;
  fs::set_permissions(&core_bin,fs::Permissions::from_mode(0o755)).map_err(|_|"Cannot set execute permission on galroon-core".to_string())?;
 }
 Ok(())
}
impl Updates{
 pub fn load(device_dir:&Path)->Arc<Self>{
  let path=device_dir.join("update-check.json");
  let stored=load(&path).unwrap_or_else(|e|{crate::diagnostics::record("warn","updates.state",&format!("Ignoring damaged update-check.json: {e}"),None);Stored::default()});
  let apply=if can_self_update(){runtime_dir().and_then(|rt|consume_last_failed(&rt)).unwrap_or_default()}else{ApplyStatus::default()};
  Arc::new(Self{path,stored:Mutex::new(stored),apply:Mutex::new(apply),gate:Arc::new(tokio::sync::Mutex::new(())),wake:tokio::sync::Notify::new(),checking:AtomicBool::new(false)})
 }
 pub fn snapshot(&self)->Value{
  let state=self.stored.lock().unwrap();
  let apply=self.apply.lock().unwrap().clone();
  let latest=state.latest.as_ref().filter(|r|newer(r));
  json!({"current":env!("CARGO_PKG_VERSION"),"install_kind":install_kind(),"can_self_update":can_self_update(),"enabled":state.enabled,"checking":self.checking.load(Ordering::Acquire),"last_checked":state.last_checked,"last_error":state.last_error,"update_available":latest.is_some(),"latest":latest,"apply":apply})
 }
 fn settings(&self,enabled:bool)->R<()>{let mut state=self.stored.lock().unwrap();let mut next=state.clone();next.enabled=enabled;save(&self.path,&next)?;*state=next;self.wake.notify_one();Ok(())}
 async fn check(self:&Arc<Self>,api:&str,automatic:bool,timeout:Duration)->Value{
  let gate=self.gate.clone().lock_owned().await;
  let previous={let state=self.stored.lock().unwrap();let interval=if !automatic{60}else if state.last_error.is_some(){3600}else{DAY};if automatic&&!state.enabled||state.last_checked.is_some_and(|t|crate::db::now().saturating_sub(t)<interval){drop(gate);drop(state);return self.snapshot();}state.clone()};
  // Once sent, a check finishes even if one HTTP caller disconnects. Other callers
  // wait on the same gate and reuse its result rather than issuing a second request.
  let updates=self.clone();let api=api.to_owned();let task=tokio::spawn(async move{
  updates.checking.store(true,Ordering::Release);let checking=Checking(&updates.checking);
  let result=fetch(&api,previous.etag.as_deref(),timeout).await;
  {let mut state=updates.stored.lock().unwrap();state.last_checked=Some(crate::db::now());match result{
   Ok(Some((latest,etag)))=>{state.latest=latest;state.etag=etag;state.last_error=None;},Ok(None)=>state.last_error=None,Err(e)=>state.last_error=Some(e)
  }if let Err(e)=save(&updates.path,&state){crate::diagnostics::record("warn","updates.save",&e,None);state.last_error=Some(format!("Cannot save update check: {e}"));}}
  drop(checking);drop(gate);updates.wake.notify_one();
  });if let Err(e)=task.await{crate::diagnostics::record("error","updates.check",&e.to_string(),None);}self.snapshot()
 }
 fn set_apply(&self,f:impl FnOnce(&mut ApplyStatus)){let mut s=self.apply.lock().unwrap();f(&mut s);}
 pub(crate) async fn apply_release(self:&Arc<Self>,rt:PathBuf,dl_base:String,pubkey:String,target_version:String,current_version:String,restart_tx:Option<tokio::sync::oneshot::Sender<()>>,restart_delay:Duration)->R<()>{
  let result=self.apply_release_inner(&rt,&dl_base,&pubkey,&target_version,&current_version,restart_tx,restart_delay).await;
  if let Err(err)=&result{
   crate::diagnostics::record("error","updates.apply",err,None);
   self.set_apply(|s|{s.state="failed";s.error=Some(err.clone());});
  }
  result
 }
 async fn apply_release_inner(&self,rt:&Path,dl_base:&str,pubkey:&str,target_version:&str,current_version:&str,restart_tx:Option<tokio::sync::oneshot::Sender<()>>,restart_delay:Duration)->R<()>{
  if !valid_version_segment(target_version)||!is_newer_version(target_version,current_version){
   return Err("Target version must be a newer full release".into());
  }
  self.set_apply(|s|{s.state="downloading";s.target_version=Some(target_version.to_owned());s.downloaded_bytes=0;s.total_bytes=None;s.error=None;});
  let client=build_download_client(dl_base,Duration::from_secs(300))?;
  let ua=format!("Galroon/{current_version}");
  let manifest_url=format!("{}/v{target_version}/nas-latest.json",dl_base.trim_end_matches('/'));
  let manifest_resp=client.get(&manifest_url).header("User-Agent",&ua).send().await.map_err(|_|"Failed to download NAS release manifest".to_string())?;
  if !manifest_resp.status().is_success(){return Err(format!("NAS release manifest unavailable (HTTP {})",manifest_resp.status()));}
  let manifest_bytes=manifest_resp.bytes().await.map_err(|_|"Failed reading NAS release manifest".to_string())?;
  if manifest_bytes.len()>MAX_MANIFEST_BYTES{return Err("NAS release manifest exceeds size limit".into());}
  let manifest:NasManifest=serde_json::from_slice(&manifest_bytes).map_err(|_|"Invalid NAS release manifest".to_string())?;
  let manifest_ver=full_version(&manifest.version).ok_or_else(||"Manifest version is invalid".to_string())?.to_string();
  if manifest_ver!=target_version||!is_newer_version(&manifest_ver,current_version){
   return Err("Manifest version does not match a newer release".into());
  }
  if manifest.size==0||manifest.size>MAX_ARCHIVE_BYTES{return Err("Update archive size is out of bounds".into());}
  if manifest.sha256.len()!=64||!manifest.sha256.bytes().all(|b|matches!(b,b'0'..=b'9'|b'a'..=b'f')){
   return Err("Manifest SHA-256 digest is invalid".into());
  }
  if !is_trusted_download_url(&manifest.url,dl_base)||!is_trusted_download_url(&manifest.sig_url,dl_base){
   return Err("Manifest download URL is outside the trusted release origin".into());
  }
  let sig_resp=client.get(&manifest.sig_url).header("User-Agent",&ua).send().await.map_err(|_|"Failed to download update signature".to_string())?;
  if !sig_resp.status().is_success(){return Err(format!("Update signature unavailable (HTTP {})",sig_resp.status()));}
  let sig_bytes=sig_resp.bytes().await.map_err(|_|"Failed reading update signature".to_string())?;
  if sig_bytes.len()>MAX_SIG_BYTES{return Err("Update signature exceeds size limit".into());}
  let sig_text=String::from_utf8(sig_bytes.to_vec()).map_err(|_|"Update signature is not valid UTF-8".to_string())?;
  let _=decode_signature(&sig_text)?;
  let _=minisign_verify::PublicKey::from_base64(pubkey).map_err(|_|"Invalid embedded update public key".to_string())?;

  let staging_root=rt.join("staging");
  fs::create_dir_all(&staging_root).map_err(|_|"Cannot create runtime staging directory".to_string())?;
  crate::plans::validate_chain(&staging_root).map_err(|_|"Runtime staging directory is unsafe".to_string())?;
  let run_dir=staging_root.join(format!("apply-{}",crate::db::id()));
  fs::create_dir_all(&run_dir).map_err(|_|"Cannot create update work directory".to_string())?;
  struct Cleanup(PathBuf);
  impl Drop for Cleanup{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
  let _cleanup=Cleanup(run_dir.clone());

  let archive_path=run_dir.join("bundle.tar.gz");
  self.set_apply(|s|{s.total_bytes=Some(manifest.size);});
  let resp=client.get(&manifest.url).header("User-Agent",&ua).send().await.map_err(|_|"Failed to download update bundle".to_string())?;
  if !resp.status().is_success(){return Err(format!("Update bundle unavailable (HTTP {})",resp.status()));}
  let mut out=fs::OpenOptions::new().write(true).create_new(true).open(&archive_path).map_err(|_|"Cannot create update archive file".to_string())?;
  let mut hasher=Sha256::new();
  let mut downloaded=0u64;
  let mut stream=resp.bytes_stream();
  while let Some(chunk)=stream.next().await{
   let chunk=chunk.map_err(|_|"Interrupted while downloading update bundle".to_string())?;
   downloaded=downloaded.saturating_add(chunk.len() as u64);
   if downloaded>manifest.size||downloaded>MAX_ARCHIVE_BYTES{return Err("Downloaded update exceeds declared size".into());}
   out.write_all(&chunk).map_err(|_|"Failed writing update bundle to disk".to_string())?;
   hasher.update(&chunk);
   self.set_apply(|s|{s.downloaded_bytes=downloaded;});
  }
  out.sync_all().map_err(|_|"Failed syncing update bundle".to_string())?;
  drop(out);
  if downloaded!=manifest.size{return Err("Downloaded update size does not match manifest".into());}
  let actual_sha=hex::encode(hasher.finalize());
  if actual_sha!=manifest.sha256{return Err("Update SHA-256 verification failed".into());}

  self.set_apply(|s|{s.state="verifying";});
  let verify_path=archive_path.clone();
  let verify_pk=pubkey.to_owned();
  let verify_sig=sig_text.clone();
  tokio::task::spawn_blocking(move||{
   let data=fs::read(&verify_path).map_err(|_|"Cannot read downloaded update archive".to_string())?;
   verify_signature_bytes(&verify_pk,&verify_sig,&data)
  }).await.map_err(|_|"Update verification task failed".to_string())??;

  self.set_apply(|s|{s.state="staging";});
  let extract_dir=run_dir.join("unpacked");
  let extract_archive=archive_path.clone();
  let extract_target=extract_dir.clone();
  tokio::task::spawn_blocking(move||safe_extract_tar_gz(&extract_archive,&extract_target))
   .await.map_err(|_|"Update extraction task failed".to_string())??;

  let releases_dir=rt.join("releases");
  fs::create_dir_all(&releases_dir).map_err(|_|"Cannot create runtime releases directory".to_string())?;
  crate::plans::validate_chain(&releases_dir).map_err(|_|"Runtime releases directory is unsafe".to_string())?;
  let final_dir=releases_dir.join(&manifest_ver);
  if final_dir.exists(){fs::remove_dir_all(&final_dir).map_err(|_|"Cannot replace existing staged release".to_string())?;}
  fs::rename(&extract_dir,&final_dir).map_err(|_|"Cannot move staged release into place".to_string())?;
  write_atomic_text(&rt.join("pending"),&format!("{manifest_ver}\n")).map_err(|_|"Cannot write pending release pointer".to_string())?;

  self.set_apply(|s|{s.state="restarting";});
  crate::diagnostics::record("info","updates.apply",&format!("Staged verified update {manifest_ver}; requesting graceful restart"),None);
  if !restart_delay.is_zero(){tokio::time::sleep(restart_delay).await;}
  if let Some(tx)=restart_tx{let _=tx.send(());}
  Ok(())
 }
 fn startup_delay(&self,startup:Duration)->Duration{let state=self.stored.lock().unwrap();startup.max(Duration::from_secs(state.last_checked.map(|t|t.saturating_add(DAY).saturating_sub(crate::db::now()).max(0) as u64).unwrap_or(0)))}
 fn delay(&self)->Option<Duration>{let state=self.stored.lock().unwrap();if !state.enabled{return None;}let interval=if state.last_error.is_some(){3600}else{DAY};Some(Duration::from_secs(state.last_checked.map(|t|(t.saturating_add(interval).saturating_sub(crate::db::now())).max(0) as u64).unwrap_or(0)))}
}
#[derive(Deserialize)]struct GithubRelease{tag_name:String,#[serde(default)]name:Option<String>,html_url:String,#[serde(default)]body:Option<String>,#[serde(default)]published_at:Option<String>,#[serde(default)]draft:bool,#[serde(default)]prerelease:bool}
async fn fetch(api:&str,etag:Option<&str>,timeout:Duration)->R<Option<(Option<Release>,Option<String>)>>{
 let client=reqwest::Client::builder().timeout(timeout).redirect(reqwest::redirect::Policy::none()).build().map_err(|e|e.to_string())?;
 let mut request=client.get(format!("{}/repos/llpgf/galroon/releases/latest",api.trim_end_matches('/'))).header("User-Agent",format!("Galroon/{}",env!("CARGO_PKG_VERSION"))).header("Accept","application/vnd.github+json");if let Some(etag)=etag{request=request.header("If-None-Match",etag);}
 let response=request.send().await.map_err(|e|e.to_string())?;if response.status()==reqwest::StatusCode::NOT_MODIFIED{return Ok(None);}
 if response.status()==reqwest::StatusCode::NOT_FOUND{return Ok(Some((None,None)));}
 if !response.status().is_success(){return Err(format!("Release check failed: HTTP {}",response.status()));}
 let etag=response.headers().get("etag").and_then(|v|v.to_str().ok()).map(str::to_owned);let release:GithubRelease=response.json().await.map_err(|e|e.to_string())?;
 let latest=full_version(&release.tag_name).filter(|_|!release.draft&&!release.prerelease).map(|version|Release{version:version.to_string(),name:release.name.unwrap_or(release.tag_name),url:trusted_url(&release.html_url),notes:release.body.unwrap_or_default().chars().take(20_000).collect(),published_at:release.published_at});Ok(Some((latest,etag)))
}
pub struct Worker{task:tokio::task::JoinHandle<()>}
impl Worker{pub fn request_stop(&self){self.task.abort();}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();}}
pub fn start(updates:Arc<Updates>)->Worker{
 // Tests exercise the real worker with an explicit fixture base. Other Core tests
 // must never gain an unsolicited public-network request by starting Services.
 #[cfg(test)]if std::env::var_os("GALROON_UPDATE_API").is_none(){return Worker{task:tokio::spawn(std::future::pending())};}
 start_with(updates,api_base(),Duration::from_secs(30))
}
fn start_with(updates:Arc<Updates>,api:String,startup:Duration)->Worker{
 Worker{task:tokio::spawn(async move{
  // Settings wake the worker immediately; an ordinary startup waits at least 30 seconds.
  tokio::select!{_=tokio::time::sleep(updates.startup_delay(startup))=>{},_=updates.wake.notified()=>{}}
  loop{match updates.delay(){
   Some(delay)=>{tokio::select!{_=tokio::time::sleep(delay)=>{updates.check(&api,true,Duration::from_secs(20)).await;},_=updates.wake.notified()=>{}}},
   None=>updates.wake.notified().await
  }}
 })}
}
pub async fn get(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{crate::auth(&a,&h)?;Ok(Json(a.updates.snapshot()))}
pub async fn check(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(Json(a.updates.check(&api_base(),false,Duration::from_secs(20)).await))}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Settings{enabled:bool}
pub async fn settings(State(a):State<App>,h:HeaderMap,Json(v):Json<Settings>)->crate::Result<Value>{let who=crate::access::authenticate(&a,&h)?;if who.role=="web"{return Err(ApiError("Desktop access required".into()));}a.updates.settings(v.enabled)?;Ok(Json(a.updates.snapshot()))}
#[derive(Deserialize,Default)]#[serde(deny_unknown_fields)]pub struct ApplyRequest{#[serde(default)]force:bool}
pub async fn apply(State(a):State<App>,h:HeaderMap,Json(v):Json<ApplyRequest>)->crate::Result<Value>{
 let who=crate::access::authenticate(&a,&h)?;
 if who.role=="web"{return Err(ApiError("Desktop access required".into()));}
 if !can_self_update(){return Err(ApiError("Self-update is only available in the Docker launcher container".into()));}
 let rt=runtime_dir().ok_or_else(||ApiError("Runtime directory is not configured".into()))?;
 {
  let current_apply=a.updates.apply.lock().unwrap();
  if current_apply.is_active(){return Ok(Json(a.updates.snapshot()));}
 }
 if !v.force{
  let active:i64={
   let c=a.db.lock().map_err(|e|ApiError(e.to_string()))?;
   c.query_row("SELECT (SELECT count(*) FROM jobs WHERE state IN ('queued','running','pausing','cancelling'))+(SELECT count(*) FROM plans WHERE state='executing')",[],|r|r.get(0))?
  };
  if active>0{return Err(ApiError("Active background tasks are running. Wait for them to finish or confirm pausing before updating.".into()));}
 }
 let target={
  let state=a.updates.stored.lock().unwrap();
  state.latest.as_ref().filter(|r|newer(r)).map(|r|r.version.clone())
 }.ok_or_else(||ApiError("No newer release is available".into()))?;
 let restart_tx=a.self_update_restart.lock().map_err(|e|ApiError(e.to_string()))?.take();
 a.updates.set_apply(|s|{s.state="downloading";s.target_version=Some(target.clone());s.downloaded_bytes=0;s.total_bytes=None;s.error=None;});
 let updates=a.updates.clone();
 let dl_base=download_base();
 tokio::spawn(async move{
  let _=updates.apply_release(rt,dl_base,UPDATE_PUBKEY.into(),target,env!("CARGO_PKG_VERSION").into(),restart_tx,Duration::from_millis(250)).await;
 });
 Ok(Json(a.updates.snapshot()))
}

#[cfg(test)]mod tests{
 use super::*;
 use axum::{http::{StatusCode,Request},response::IntoResponse,body::{Body,to_bytes},routing::get,Router};
 use tower::ServiceExt;
 #[derive(Clone)]struct Reply{status:StatusCode,body:Value,delay:Duration}
 struct Fixture{api:String,reply:Arc<Mutex<Reply>>,requests:Arc<Mutex<Vec<HeaderMap>>>,task:tokio::task::JoinHandle<()>}
 impl Drop for Fixture{fn drop(&mut self){self.task.abort();}}
 impl Fixture{
  async fn new()->Self{
   let reply=Arc::new(Mutex::new(Reply{status:StatusCode::OK,body:release("v9.0.0"),delay:Duration::ZERO}));let requests=Arc::new(Mutex::new(Vec::new()));let r=reply.clone();let seen=requests.clone();
   let router=Router::new().route("/repos/llpgf/galroon/releases/latest",get(move|headers:HeaderMap|{let r=r.lock().unwrap().clone();seen.lock().unwrap().push(headers);async move{tokio::time::sleep(r.delay).await;let mut response=(r.status,Json(r.body)).into_response();response.headers_mut().insert("etag","\"fixture\"".parse().unwrap());response}}));
   let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=format!("http://{}",listener.local_addr().unwrap());let task=tokio::spawn(async move{axum::serve(listener,router).await.unwrap()});Self{api,reply,requests,task}
  }
  fn count(&self)->usize{self.requests.lock().unwrap().len()}
 }
 fn release(tag:&str)->Value{json!({"tag_name":tag,"name":"Fixture release","html_url":"https://github.com/llpgf/galroon/releases/tag/fixture","body":"<b>Plain notes</b>","published_at":"2026-10-12T00:00:00Z","draft":false,"prerelease":false})}
 fn expire(updates:&Updates){updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-DAY-1);}
 async fn checked(updates:&Updates){tokio::time::timeout(Duration::from_secs(2),async{while updates.snapshot()["last_checked"].is_null()||updates.snapshot()["checking"]==true{tokio::time::sleep(Duration::from_millis(2)).await;}}).await.unwrap();}
 #[tokio::test]async fn updates_full_versions_only_and_trusted_plain_text(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  for (tag,available) in [("v9.0.0",true),(env!("CARGO_PKG_VERSION"),false),("0.0.1",false),("v0.6.0-beta.1",false),("garbage",false)]{
   fixture.reply.lock().unwrap().body=release(tag);expire(&updates);let state=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(state["update_available"],available,"{tag}");assert_eq!(state["latest"].is_null(),!available);assert!(state["last_error"].is_null());
  }
  for field in ["draft","prerelease"]{let mut body=release("v9.0.0");body[field]=json!(true);fixture.reply.lock().unwrap().body=body;expire(&updates);assert_eq!(updates.check(&fixture.api,false,Duration::from_secs(1)).await["update_available"],false);}
  let mut body=release("v9.0.0");body["html_url"]=json!("https://evil.example/releases/9");body["body"]=json!("界".repeat(20_001));fixture.reply.lock().unwrap().body=body;expire(&updates);
  let state=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(state["latest"]["url"],RELEASES);assert_eq!(state["latest"]["notes"].as_str().unwrap().chars().count(),20_000);
  assert_eq!(updates.snapshot()["current"],env!("CARGO_PKG_VERSION"));let requests=fixture.requests.lock().unwrap();assert_eq!(requests[0]["user-agent"],format!("Galroon/{}",env!("CARGO_PKG_VERSION")));assert_eq!(requests[0]["accept"],"application/vnd.github+json");
 }
 #[tokio::test]async fn updates_etag_304_404_and_failures_preserve_previous_release(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());let first=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(first["latest"]["notes"],"<b>Plain notes</b>");
  fixture.reply.lock().unwrap().status=StatusCode::NOT_MODIFIED;expire(&updates);let unchanged=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(unchanged["latest"],first["latest"]);assert!(unchanged["last_error"].is_null());assert_eq!(fixture.requests.lock().unwrap()[1]["if-none-match"],"\"fixture\"");
  fixture.reply.lock().unwrap().status=StatusCode::INTERNAL_SERVER_ERROR;expire(&updates);let failed=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(failed["latest"],first["latest"]);assert!(failed["last_error"].as_str().unwrap().contains("500"));
  let persisted=Updates::load(t.path()).snapshot();assert_eq!(persisted["last_error"],failed["last_error"]);assert_eq!(persisted["latest"],first["latest"]);
  {let mut reply=fixture.reply.lock().unwrap();reply.status=StatusCode::OK;reply.delay=Duration::from_millis(100);}expire(&updates);let timeout=updates.check(&fixture.api,false,Duration::from_millis(20)).await;assert!(!timeout["last_error"].is_null());assert_eq!(timeout["latest"],first["latest"]);
  {let mut reply=fixture.reply.lock().unwrap();reply.status=StatusCode::NOT_FOUND;reply.delay=Duration::ZERO;}expire(&updates);let missing=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert!(!missing["last_checked"].is_null());assert!(missing["last_error"].is_null());assert!(missing["latest"].is_null());assert_eq!(missing["update_available"],false);assert!(updates.stored.lock().unwrap().etag.is_none());
 }
 #[tokio::test]async fn updates_concurrent_checks_share_one_request_and_throttle(){
  let fixture=Fixture::new().await;fixture.reply.lock().unwrap().delay=Duration::from_millis(50);let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  let (a,b)=tokio::join!(updates.check(&fixture.api,false,Duration::from_secs(1)),updates.check(&fixture.api,false,Duration::from_secs(1)));assert_eq!(a,b);assert_eq!(fixture.count(),1);assert_eq!(a["checking"],false);
  updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),1);
  updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-61);updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),2);
  expire(&updates);fixture.reply.lock().unwrap().status=StatusCode::INTERNAL_SERVER_ERROR;updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);
  updates.check(&fixture.api,true,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);updates.stored.lock().unwrap().last_checked=Some(crate::db::now()-120);updates.check(&fixture.api,true,Duration::from_secs(1)).await;assert_eq!(fixture.count(),3);assert!(updates.delay().unwrap()>Duration::from_secs(3400));
  // A manual check after a failure waits only for the 60 s window, not the hourly retry.
  updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),4);fixture.reply.lock().unwrap().status=StatusCode::OK;
  expire(&updates);fixture.reply.lock().unwrap().status=StatusCode::OK;let shared=updates.clone();let api=fixture.api.clone();let caller=tokio::spawn(async move{shared.check(&api,false,Duration::from_secs(1)).await});
  tokio::time::timeout(Duration::from_secs(1),async{while fixture.count()<5{tokio::task::yield_now().await;}}).await.unwrap();caller.abort();let result=updates.check(&fixture.api,false,Duration::from_secs(1)).await;assert_eq!(fixture.count(),5);assert!(result["last_error"].is_null());assert_eq!(result["checking"],false);
 }
 #[tokio::test]async fn updates_disabled_worker_manual_check_and_setting_survive_initialize(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let device=t.path().join("device");let catalog=t.path().join("catalog");let app=crate::initialize_with_device(catalog.clone(),&device).unwrap();app.updates.settings(false).unwrap();
  let worker=start_with(app.updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(30)).await;assert_eq!(fixture.count(),0);
  assert_eq!(app.updates.check(&fixture.api,false,Duration::from_secs(1)).await["update_available"],true);assert_eq!(fixture.count(),1);worker.request_stop();drop(worker);drop(app);
  let app=crate::initialize_with_device(catalog,&device).unwrap();assert_eq!(app.updates.snapshot()["enabled"],false);assert_eq!(app.updates.snapshot()["latest"]["version"],"9.0.0");assert!(!app.state_dir.join("update-check.json").exists());
  let backup=crate::backup::export(&app.db,t.path()).unwrap();assert!(!backup.join("update-check.json").exists());
 }
 #[tokio::test]async fn updates_worker_wakes_on_enable_skips_recent_and_stops_promptly(){
  let fixture=Fixture::new().await;let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());updates.settings(false).unwrap();
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(10)).await;updates.settings(true).unwrap();checked(&updates).await;assert_eq!(fixture.count(),1);
  updates.settings(false).unwrap();updates.settings(true).unwrap();tokio::time::sleep(Duration::from_millis(20)).await;assert_eq!(fixture.count(),1);assert!(updates.delay().unwrap()>Duration::from_secs(DAY as u64-10));
  worker.request_stop();tokio::time::timeout(Duration::from_secs(1),async{while !worker.task.is_finished(){tokio::task::yield_now().await;}}).await.unwrap();
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::ZERO);tokio::time::sleep(Duration::from_millis(20)).await;assert_eq!(fixture.count(),1);worker.request_stop();
 }
 #[tokio::test]async fn updates_worker_delays_startup_and_exposes_inflight_state(){
  let fixture=Fixture::new().await;fixture.reply.lock().unwrap().delay=Duration::from_millis(80);let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());
  let worker=start_with(updates.clone(),fixture.api.clone(),Duration::from_millis(50));tokio::time::sleep(Duration::from_millis(10)).await;assert_eq!(fixture.count(),0);
  tokio::time::timeout(Duration::from_secs(1),async{while fixture.count()==0{tokio::task::yield_now().await;}}).await.unwrap();assert_eq!(updates.snapshot()["checking"],true);updates.settings(false).unwrap();checked(&updates).await;assert_eq!(updates.snapshot()["enabled"],false);worker.request_stop();
 }
 #[tokio::test]async fn updates_damaged_state_and_role_boundaries(){
  let t=tempfile::tempdir().unwrap();let dir=t.path().join("state");fs::create_dir(&dir).unwrap();fs::write(dir.join("update-check.json"),b"damaged").unwrap();let app=crate::initialize(dir).unwrap();assert_eq!(app.updates.snapshot()["enabled"],true);
  let web="w".repeat(72);let desktop="d".repeat(72);let cookie=format!("galroon_sid_{}={web}",&hex::encode(Sha256::digest(app.state_dir.to_string_lossy().as_bytes()))[..16]);
  for (role,token) in [("web",&web),("desktop",&desktop)]{app.db.lock().unwrap().execute("INSERT INTO sessions(token_hash,role,created,id,name,expires) VALUES(?1,?2,0,?2,?2,?3)",rusqlite::params![hex::encode(Sha256::digest(token.as_bytes())),role,crate::db::now()+3600]).unwrap();}
  // Seed a recent check so the authenticated POST handler cannot use the public network.
  app.updates.stored.lock().unwrap().last_checked=Some(crate::db::now());let router=crate::router(app.clone());
  for (role,method,path,status,body) in [("web","GET","/api/updates",200,json!(null)),("web","POST","/api/updates/check",403,json!({})),("web","POST","/api/updates/settings",403,json!({"enabled":false})),("web","POST","/api/updates/apply",403,json!({})),("owner","POST","/api/updates/settings",200,json!({"enabled":false})),("desktop","POST","/api/updates/settings",200,json!({"enabled":true})),("desktop","POST","/api/updates/check",200,json!({})),("owner","POST","/api/updates/check",200,json!({})),("owner","POST","/api/updates/settings",422,json!({"enabled":false,"extra":1})),("owner","POST","/api/updates/apply",422,json!({"extra":1}))]{
   let mut request=Request::builder().method(method).uri(path).header("Host","localhost").header("Content-Type","application/json");request=if role=="web"{request.header("Cookie",&cookie)}else{request.header("Authorization",format!("Bearer {}",if role=="owner"{app.token.as_str()}else{desktop.as_str()}))};
   let response=router.clone().oneshot(request.body(Body::from(body.to_string())).unwrap()).await.unwrap();assert_eq!(response.status().as_u16(),status,"{role} {path}");if status==200{let value:Value=serde_json::from_slice(&to_bytes(response.into_body(),10_000).await.unwrap()).unwrap();assert!(value.get("current").is_some());assert!(value.get("etag").is_none());}
  }
  let response=router.oneshot(Request::builder().uri("/api/updates").header("Host","localhost").body(Body::empty()).unwrap()).await.unwrap();assert_eq!(response.status(),StatusCode::UNAUTHORIZED);
 }
 #[test]fn updates_startup_and_retry_schedule(){
  let t=tempfile::tempdir().unwrap();let updates=Updates::load(t.path());let startup=Duration::from_secs(30);assert_eq!(updates.startup_delay(startup),startup);assert_eq!(updates.delay(),Some(Duration::ZERO));
  {let mut state=updates.stored.lock().unwrap();state.last_checked=Some(crate::db::now()-120);state.last_error=Some("Offline".into());}
  assert!(updates.startup_delay(startup)>Duration::from_secs(DAY as u64-130));let retry=updates.delay().unwrap();assert!(retry>Duration::from_secs(3400)&&retry<=Duration::from_secs(3480));updates.settings(false).unwrap();assert_eq!(updates.delay(),None);
 }
 #[cfg(unix)]#[test]fn updates_refuse_symlinks_and_recover_invalid_schema(){
  let t=tempfile::tempdir().unwrap();let outside=t.path().join("outside");fs::write(&outside,b"unchanged").unwrap();let device=t.path().join("device");fs::create_dir(&device).unwrap();let path=device.join("update-check.json");std::os::unix::fs::symlink(&outside,&path).unwrap();let updates=Updates::load(&device);assert!(updates.settings(false).is_err());assert_eq!(fs::read(&outside).unwrap(),b"unchanged");fs::remove_file(&path).unwrap();
  for bytes in [r#"{"version":2,"enabled":true}"#,r#"{"version":1,"enabled":"yes"}"#]{fs::write(&path,bytes).unwrap();assert_eq!(Updates::load(&device).snapshot()["enabled"],true);}
 }
 #[test]fn embedded_update_pubkey_is_valid_minisign(){
  assert!(minisign_verify::PublicKey::from_base64(UPDATE_PUBKEY).is_ok());
 }
 #[test]fn verifies_tauri_signer_base64_and_raw_minisign_signatures(){
  // Generated with `npx tauri signer generate --ci` + `npx tauri signer sign` on b"hello world" using a throwaway key.
  let pk="RWStLktbI0BLrVYBFiuQrSy7+HYXc/EdgVj7VCiPqFv89vGe8JMDgtw2";
  let tauri_b64_sig="dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTdExrdGJJMEJMclV3bTRqRkk2V1pTenh1Sk43eFNndUsvMnh2Q24yeW9DQ0xlcjcvbDlTTENXZStZaGo0NkhPQ2FKb25GbXhFakZzTW40Vmh5N3BvOWNwWmhpU2syTGdzPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk1ODM4MTI4CWZpbGU6c2FtcGxlLmJpbgpPQVF0SXg0N0ZmRkpUT3duOUlndWl6NHBldWxKYUtSQXZHeEVSeUpJd3pGaU5LakNUSFV4d1B6elc4eUlrYjRyN2ZFZklVRnRqOVkwWHpVM0xVLzBEUT09Cg==";
  let raw_sig=String::from_utf8(decode_base64(tauri_b64_sig).unwrap()).unwrap();
  assert!(verify_signature_bytes(pk,tauri_b64_sig,b"hello world").is_ok());
  assert!(verify_signature_bytes(pk,&raw_sig,b"hello world").is_ok());
  assert!(verify_signature_bytes(pk,tauri_b64_sig,b"hello worl!").is_err());
  assert!(verify_signature_bytes(UPDATE_PUBKEY,tauri_b64_sig,b"hello world").is_err());
 }
 #[test]fn safe_extract_rejects_traversal_symlinks_and_missing_files(){
  let t=tempfile::tempdir().unwrap();
  let make_tar=|build:&dyn Fn(&mut tar::Builder<flate2::write::GzEncoder<fs::File>>)|->PathBuf{
   let p=t.path().join(format!("{}.tar.gz",crate::db::id()));
   let f=fs::File::create(&p).unwrap();
   let gz=flate2::write::GzEncoder::new(f,flate2::Compression::fast());
   let mut ar=tar::Builder::new(gz);
   build(&mut ar);
   ar.into_inner().unwrap().finish().unwrap();
   p
  };
  let add_file=|ar:&mut tar::Builder<flate2::write::GzEncoder<fs::File>>,path:&str,data:&[u8]|{
   let mut h=tar::Header::new_gnu();h.set_size(data.len() as u64);h.set_mode(0o644);h.set_cksum();
   ar.append_data(&mut h,path,data).unwrap();
  };
  // Valid archive succeeds
  let good=make_tar(&|ar|{add_file(ar,"galroon-core",b"#!/bin/sh\n");add_file(ar,"web/index.html",b"<html></html>");});
  let out=t.path().join("good");
  safe_extract_tar_gz(&good,&out).unwrap();
  assert!(out.join("galroon-core").is_file());
  assert!(out.join("web/index.html").is_file());
  // Missing web/index.html fails
  let no_web=make_tar(&|ar|{add_file(ar,"galroon-core",b"bin");});
  assert!(safe_extract_tar_gz(&no_web,&t.path().join("no-web")).is_err());
  // Symlink entry fails
  let symlink_tar=make_tar(&|ar|{
   add_file(ar,"galroon-core",b"bin");add_file(ar,"web/index.html",b"html");
   let mut h=tar::Header::new_gnu();h.set_entry_type(tar::EntryType::Symlink);h.set_size(0);h.set_cksum();
   ar.append_link(&mut h,"evil","/etc/passwd").unwrap();
  });
  assert!(safe_extract_tar_gz(&symlink_tar,&t.path().join("sym")).is_err());
 }
 #[test]fn runtime_health_promotes_and_prunes_old_releases(){
  let t=tempfile::tempdir().unwrap();
  let rt=t.path().join("runtime");
  for v in ["0.1.0","0.2.0","0.3.0"]{
   let d=rt.join("releases").join(v);fs::create_dir_all(&d).unwrap();fs::write(d.join("galroon-core"),b"bin").unwrap();
  }
  write_atomic_text(&rt.join("current"),"0.2.0\n").unwrap();
  write_atomic_text(&rt.join("attempt"),"0.3.0\n").unwrap();
  mark_runtime_healthy_at(&rt,"0.3.0").unwrap();
  assert_eq!(read_trimmed(&rt.join("healthy")).as_deref(),Some("0.3.0"));
  assert_eq!(read_trimmed(&rt.join("current")).as_deref(),Some("0.3.0"));
  assert_eq!(read_trimmed(&rt.join("previous")).as_deref(),Some("0.2.0"));
  assert!(!rt.join("attempt").exists());
  assert!(rt.join("releases/0.3.0").exists());
  assert!(rt.join("releases/0.2.0").exists());
  assert!(!rt.join("releases/0.1.0").exists());
 }
 #[tokio::test]async fn apply_rejects_downgrade_bad_sha_untrusted_url_and_bad_signature(){
  let t=tempfile::tempdir().unwrap();
  let updates=Updates::load(t.path());
  let rt=t.path().join("runtime");
  // Downgrade rejected before any network call
  let err=updates.apply_release(rt.clone(),"http://127.0.0.1:1".into(),UPDATE_PUBKEY.into(),"0.0.1".into(),env!("CARGO_PKG_VERSION").into(),None,Duration::ZERO).await.unwrap_err();
  assert!(err.contains("newer"),"{err}");
  // Mock HTTP server serving valid tar.gz but tampered signature
  let archive_bytes=b"not-a-valid-signed-archive".to_vec();
  let sha=hex::encode(Sha256::digest(&archive_bytes));
  let bad_sig="untrusted comment: signature from minisign secret key\nRWTAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\ntrusted comment: test\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==\n".to_string();
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base=format!("http://{}",listener.local_addr().unwrap());
  let manifest=json!({"version":"9.9.9","url":format!("{base}/v9.9.9/galroon-nas-x86_64.tar.gz"),"sig_url":format!("{base}/v9.9.9/galroon-nas-x86_64.tar.gz.sig"),"sha256":sha,"size":archive_bytes.len()});
  let m_bytes=serde_json::to_vec(&manifest).unwrap();
  let b_bytes=archive_bytes.clone();
  let s_bytes=bad_sig.into_bytes();
  let router=Router::new()
   .route("/v9.9.9/nas-latest.json",get(move||async move{m_bytes.clone()}))
   .route("/v9.9.9/galroon-nas-x86_64.tar.gz",get(move||async move{b_bytes.clone()}))
   .route("/v9.9.9/galroon-nas-x86_64.tar.gz.sig",get(move||async move{s_bytes.clone()}));
  let srv=tokio::spawn(async move{axum::serve(listener,router).await.unwrap()});
  let err=updates.apply_release(rt.clone(),base,UPDATE_PUBKEY.into(),"9.9.9".into(),env!("CARGO_PKG_VERSION").into(),None,Duration::ZERO).await.unwrap_err();
  assert!(err.to_lowercase().contains("signature"),"{err}");
  assert!(!rt.join("pending").exists());
  assert_eq!(updates.snapshot()["apply"]["state"],"failed");
  srv.abort();
 }
}

//! Local bounded JSONL diagnostics. Never records request bodies, headers or query strings.
use std::{fs,io::Write,path::{Path,PathBuf},sync::{Mutex,OnceLock}};
use axum::{extract::{Request,MatchedPath,State},middleware::Next,response::Response,http::HeaderMap,Json};
use serde_json::{Value,json};
const LIMIT:u64=1024*1024;
static LOG:OnceLock<Mutex<Log>>=OnceLock::new();
tokio::task_local!{static REQUEST_ID:String;}
struct Log{dir:PathBuf,last_error:Option<String>}
fn scrub(message:&str)->String{
 let urls=regex::Regex::new(r"https?://[^\s]+" ).unwrap().replace_all(message,"[URL omitted]").into_owned();
 let bearer=regex::Regex::new(r"(?i)bearer\s+[^\s,;]+" ).unwrap().replace_all(&urls,"Bearer [redacted]").into_owned();
 regex::Regex::new(r#"(?i)["']?\b(password|token|secret|authorization|cookie|pairing_code|access_token|refresh_token|api_key)["']?\s*[:=]\s*(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,;]+)"#).unwrap().replace_all(&bearer,"$1=[redacted]").chars().take(1500).collect()
}
impl Log{
 fn new(dir:PathBuf)->std::io::Result<Self>{fs::create_dir_all(&dir)?;Ok(Self{dir,last_error:None})}
 fn write(&mut self,entry:&Value)->std::io::Result<()>{
  let path=self.dir.join("core.jsonl");let mut bytes=serde_json::to_vec(entry)?;bytes.push(b'\n');
  if fs::metadata(&path).map(|m|m.len()).unwrap_or(0)+bytes.len() as u64>LIMIT{
   let oldest=self.dir.join("core.2.jsonl");if oldest.exists(){fs::remove_file(&oldest)?;}
   let previous=self.dir.join("core.1.jsonl");if previous.exists(){fs::rename(&previous,&oldest)?;}
   if path.exists(){fs::rename(&path,&previous)?;}
  }
  fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(&bytes)
 }
}
pub fn init(base:&Path)->std::io::Result<()>{let dir=base.join("logs");crate::plans::validate_chain(&dir).map_err(std::io::Error::other)?;let log=Log::new(dir)?;let _=LOG.set(Mutex::new(log));record("info","core.start", "Core starting",None);
 let previous=std::panic::take_hook();std::panic::set_hook(Box::new(move|info|{let location=info.location().map(|v|format!("{}:{}",v.file(),v.line())).unwrap_or_default();record("error","core.panic",&format!("Panic at {location}"),None);previous(info);}));Ok(())}
pub fn record(level:&str,code:&str,message:&str,job:Option<&str>){
 if let Some(log)=LOG.get(){if let Ok(mut log)=log.lock(){let id=REQUEST_ID.try_with(Clone::clone).ok();let entry=json!({"time":crate::db::now(),"version":env!("CARGO_PKG_VERSION"),"level":level,"code":code,"message":scrub(message),"job_id":job,"request_id":id});if let Err(error)=log.write(&entry){log.last_error=Some(error.to_string());}}}
}
pub async fn requests(request:Request,next:Next)->Response{
 let route=request.extensions().get::<MatchedPath>().map(|m|m.as_str().to_owned()).unwrap_or_else(||"unmatched".into());let method=request.method().to_string();let id=crate::db::id();let start=std::time::Instant::now();
 REQUEST_ID.scope(id.clone(),async move{let mut response=next.run(request).await;if response.status().is_client_error()||response.status().is_server_error(){record("error","http.failed",&format!("{method} {route}: {} ({} ms)",response.status(),start.elapsed().as_millis()),None);}if let Ok(id)=id.parse(){response.headers_mut().insert("x-galroon-request-id",id);}response}).await
}
pub async fn read(State(a):State<crate::App>,h:HeaderMap)->crate::Result<Value>{
 let who=crate::access::authenticate(&a,&h)?;if who.role!="owner"{return Err(crate::ApiError("Local owner access required".into()));}
 Ok(Json(snapshot().map_err(crate::ApiError)?))
}
pub fn snapshot()->Result<Value,String>{
 let Some(log)=LOG.get()else{return Ok(json!({"available":false,"error":"File logging has not been initialized","text":""}));};let log=log.lock().map_err(|e|e.to_string())?;
 let mut text=String::new();for name in ["core.2.jsonl","core.1.jsonl","core.jsonl"]{let path=log.dir.join(name);if path.exists(){if fs::metadata(&path).map_err(|e|e.to_string())?.len()>LIMIT{return Err("Diagnostic file exceeds its size limit".into());}let bytes=fs::read(path).map_err(|e|e.to_string())?;text.push_str(&String::from_utf8_lossy(&bytes));}}
 Ok(json!({"available":true,"error":log.last_error,"text":text,"max_bytes":3*LIMIT}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn credentials_and_urls_are_removed_and_messages_bounded(){
  let clean=scrub("Failed token=secret-token password='hidden value' Authorization: Bearer abc123 cookie=session-secret https://example.invalid/?password=hidden");
  for secret in ["secret-token","hidden value","abc123","session-secret","example.invalid"]{assert!(!clean.contains(secret));}assert!(clean.contains("Failed"));assert!(scrub(&"界".repeat(5000)).chars().count()<=1500);
 }
 #[test]fn quoted_and_escaped_credentials_are_removed(){
  let message=r#"Failed {"password":"first\"second value","access_token":"access-value","refresh_token":"refresh-value","api_key":"api-value",'cookie':'session value'}"#;
  let clean=scrub(message);
  for secret in ["first","second","access-value","refresh-value","api-value","session value"]{assert!(!clean.contains(secret),"{clean}");}
  assert!(clean.contains("Failed"));
 }
 #[test]fn logs_rotate_as_complete_json_lines_and_keep_three_files(){
  let t=tempfile::tempdir().unwrap();let mut log=Log::new(t.path().join("logs")).unwrap();for i in 0..4000{log.write(&json!({"seq":i,"message":"x".repeat(1100)})).unwrap();}
  assert_eq!(fs::read_dir(&log.dir).unwrap().count(),3);for entry in fs::read_dir(&log.dir).unwrap(){let path=entry.unwrap().path();assert!(fs::metadata(&path).unwrap().len()<=LIMIT);for line in fs::read_to_string(path).unwrap().lines(){serde_json::from_str::<Value>(line).unwrap();}}
 }
}

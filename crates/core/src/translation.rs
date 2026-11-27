//! LLM-backed translation for work descriptions and character biographies via `genai`.
use axum::{extract::{Path as ApiPath,Query,State},http::HeaderMap,Json};
use genai::{adapter::AdapterKind,chat::{ChatMessage,ChatRequest},resolver::{AuthData,Endpoint},Client,ModelIden,ServiceTarget};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::{Path,PathBuf},sync::{Arc,Mutex},time::{Duration,Instant}};
use crate::{db,plans,ApiError,App};

type R<T>=Result<T,String>;
const DEFAULT_LANG:&str="zh-TW";
const MAX_SOURCE_CHARS:usize=24_000;
const MAX_PROMPT_CHARS:usize=4_000;
const BATCH_PACING_MS:u64=400;

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Provider{Openai,Gemini,Claude}
impl Provider{
 pub fn as_str(&self)->&'static str{match self{Self::Openai=>"openai",Self::Gemini=>"gemini",Self::Claude=>"claude"}}
 fn adapter_kind(&self)->AdapterKind{match self{Self::Openai=>AdapterKind::OpenAI,Self::Gemini=>AdapterKind::Gemini,Self::Claude=>AdapterKind::Anthropic}}
 fn default_endpoint(&self)->&'static str{match self{Self::Openai=>"https://api.openai.com/v1/",Self::Gemini=>"https://generativelanguage.googleapis.com/v1beta/",Self::Claude=>"https://api.anthropic.com/v1/"}}
 fn default_model(&self)->&'static str{match self{Self::Openai=>"gpt-4o-mini",Self::Gemini=>"gemini-2.5-flash",Self::Claude=>"claude-haiku-4-5"}}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct StoredSettings{
 #[serde(default="version_one")]pub version:u32,
 #[serde(default="default_provider")]pub provider:Provider,
 #[serde(default)]pub base_url:String,
 #[serde(default)]pub model:String,
 #[serde(default="default_lang")]pub target_language:String,
 #[serde(default)]pub system_prompt:String,
 #[serde(default)]pub credential_id:Option<String>,
}
fn version_one()->u32{1}
fn default_provider()->Provider{Provider::Openai}
fn default_lang()->String{DEFAULT_LANG.into()}
impl Default for StoredSettings{
 fn default()->Self{Self{version:1,provider:Provider::Openai,base_url:String::new(),model:String::new(),target_language:DEFAULT_LANG.into(),system_prompt:String::new(),credential_id:None}}
}

#[derive(Default,Clone,Debug,Serialize)]
pub struct BatchStatus{
 pub running:bool,pub total:usize,pub translated:usize,pub failed:usize,pub skipped:usize,
 pub current_title:Option<String>,pub last_error:Option<String>,pub started_at:Option<i64>,pub finished_at:Option<i64>,
}
#[derive(Default)]
pub struct BatchRunner{state:Mutex<BatchStatus>,abort:Mutex<Option<tokio::task::AbortHandle>>}
impl BatchRunner{
 pub fn snapshot(&self)->BatchStatus{self.state.lock().unwrap().clone()}
 pub fn cancel(&self)->bool{
  let handle=self.abort.lock().unwrap().take();
  if let Some(h)=handle{h.abort();let mut s=self.state.lock().unwrap();s.running=false;s.current_title=None;s.finished_at=Some(db::now());true}else{false}
 }
}

pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS translations(target_kind TEXT NOT NULL CHECK(target_kind IN ('work','character')),target_id TEXT NOT NULL,target_lang TEXT NOT NULL,source_hash TEXT NOT NULL,translated_text TEXT NOT NULL,provider TEXT NOT NULL,model TEXT NOT NULL,updated INTEGER NOT NULL,PRIMARY KEY(target_kind,target_id,target_lang));")
}

// Credential storage follows bangumi/credentials.rs: OS keychain on Windows/macOS, 0600 secret_files on Linux/NAS.
#[cfg(any(windows,target_os="macos"))]
fn cred_entry(id:&str)->R<keyring::Entry>{keyring::Entry::new("Galroon.Translation",id).map_err(|_|"System credential storage is unavailable".into())}
#[cfg(any(windows,target_os="macos"))]
fn cred_write(dir:&Path,id:&str,key:&str)->R<()>{
 // Fallback to owner-only file if OS keychain is unavailable (e.g. headless CI/SSH sessions).
 if let Ok(entry)=cred_entry(id){if entry.set_password(key).is_ok(){return Ok(());}}
 crate::secret_files::write(dir,"translation",id,key)
}
#[cfg(any(windows,target_os="macos"))]
fn cred_read(dir:&Path,id:&str)->R<String>{
 if let Ok(entry)=cred_entry(id){if let Ok(v)=entry.get_password(){if !v.is_empty(){return Ok(v);}}}
 crate::secret_files::read(dir,"translation",id)
}
#[cfg(any(windows,target_os="macos"))]
fn cred_delete(dir:&Path,id:&str)->R<()>{
 if let Ok(entry)=cred_entry(id){let _=entry.delete_credential();}
 crate::secret_files::delete(dir,"translation",id)
}
#[cfg(not(any(windows,target_os="macos")))]
fn cred_write(dir:&Path,id:&str,key:&str)->R<()>{crate::secret_files::write(dir,"translation",id,key)}
#[cfg(not(any(windows,target_os="macos")))]
fn cred_read(dir:&Path,id:&str)->R<String>{crate::secret_files::read(dir,"translation",id)}
#[cfg(not(any(windows,target_os="macos")))]
fn cred_delete(dir:&Path,id:&str)->R<()>{crate::secret_files::delete(dir,"translation",id)}

fn settings_path(device_dir:&Path)->PathBuf{device_dir.join("translation.json")}
pub fn load_settings(device_dir:&Path)->StoredSettings{
 let path=settings_path(device_dir);
 if plans::validate_chain(&path).is_err()||!path.exists(){return StoredSettings::default();}
 fs::read(&path).ok().and_then(|bytes|serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}
fn save_settings(device_dir:&Path,settings:&StoredSettings)->R<()>{
 plans::validate_chain(device_dir)?;fs::create_dir_all(device_dir).map_err(|e|e.to_string())?;
 let path=settings_path(device_dir);plans::validate_chain(&path)?;
 let tmp=device_dir.join(format!("translation-{}.tmp",db::id()));
 let mut options=fs::OpenOptions::new();options.create_new(true).write(true);
 #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
 let result=(||{let mut f=options.open(&tmp)?;f.write_all(&serde_json::to_vec_pretty(settings).map_err(|e|std::io::Error::new(std::io::ErrorKind::Other,e))?)?;f.sync_all()?;drop(f);fs::rename(&tmp,&path)})();
 if result.is_err(){let _=fs::remove_file(&tmp);}
 result.map_err(|e|e.to_string())
}

pub fn normalize_base_url(raw:&str)->R<String>{
 let trimmed=raw.trim();
 if trimmed.is_empty(){return Ok(String::new());}
 let parsed=reqwest::Url::parse(trimmed).map_err(|_|"Base URL must be a valid http:// or https:// URL".to_owned())?;
 if !matches!(parsed.scheme(),"http"|"https"){return Err("Base URL must use http:// or https://".into());}
 if !parsed.username().is_empty()||parsed.password().is_some()||parsed.query().is_some()||parsed.fragment().is_some(){
  return Err("Base URL must not contain credentials, query parameters, or fragments".into());
 }
 let mut out=parsed.to_string();
 if !out.ends_with('/'){out.push('/');}
 Ok(out)
}
fn is_loopback_url(url:&str)->bool{
 reqwest::Url::parse(url).ok().and_then(|u|u.host_str().map(|h|matches!(h,"localhost"|"127.0.0.1"|"[::1]"))).unwrap_or(false)
}
pub fn source_hash(text:&str)->String{hex::encode(Sha256::digest(text.trim().as_bytes()))}
fn default_system_prompt(target_lang:&str)->String{
 let lang_name=match target_lang{
  "zh-TW"=>"Traditional Chinese (繁體中文)",
  "zh-CN"=>"Simplified Chinese (简体中文)",
  "ja"=>"Japanese (日本語)",
  "en"=>"English",
  other=>other,
 };
 format!(
  "You are a professional visual novel and galgame translator. Translate the user's description into natural, fluent {lang_name}.\n\
   Rules:\n\
   1. Preserve BBCode tags like [spoiler]...[/spoiler] and [url=...]...[/url] intact while translating the inner text.\n\
   2. Keep paragraph breaks and formatting faithful to the original.\n\
   3. Output ONLY the translated text without explanations, quotes, or markdown code fences."
 )
}

fn has_api_key(device_dir:&Path,settings:&StoredSettings)->bool{
 settings.credential_id.as_deref().is_some_and(|id|cred_read(device_dir,id).is_ok_and(|k|!k.trim().is_empty()))
}
fn is_configured(device_dir:&Path,settings:&StoredSettings)->bool{
 let effective_url=if settings.base_url.is_empty(){settings.provider.default_endpoint()}else{&settings.base_url};
 !settings.model.trim().is_empty()&&(has_api_key(device_dir,settings)||is_loopback_url(effective_url))
}
fn settings_json(device_dir:&Path,settings:&StoredSettings,untranslated_works:i64)->Value{
 let has_key=has_api_key(device_dir,settings);
 json!({
  "provider":settings.provider.as_str(),
  "base_url":settings.base_url,
  "default_base_url":settings.provider.default_endpoint(),
  "model":settings.model,
  "default_model":settings.provider.default_model(),
  "target_language":settings.target_language,
  "system_prompt":settings.system_prompt,
  "has_api_key":has_key,
  "configured":is_configured(device_dir,settings),
  "untranslated_works":untranslated_works,
 })
}

fn build_genai_client(settings:&StoredSettings,api_key:String)->R<(Client,String)>{
 let _=rustls::crypto::ring::default_provider().install_default();
 let model_name=if settings.model.trim().is_empty(){settings.provider.default_model().to_owned()}else{settings.model.trim().to_owned()};
 let endpoint_url=if settings.base_url.trim().is_empty(){settings.provider.default_endpoint().to_owned()}else{normalize_base_url(&settings.base_url)?};
 let adapter_kind=settings.provider.adapter_kind();
 let auth_key=if api_key.trim().is_empty(){
  if is_loopback_url(&endpoint_url){"local".to_owned()}else{return Err("API key is required for this provider".into());}
 }else{api_key};
 let http=reqwest13::Client::builder().timeout(Duration::from_secs(90)).connect_timeout(Duration::from_secs(15)).build().map_err(|e|format!("HTTP client error: {e}"))?;
 let target_endpoint=Endpoint::from_owned(endpoint_url);
 let target_model=ModelIden::new(adapter_kind,model_name.clone());
 let target_auth=AuthData::from_single(auth_key);
 let client=Client::builder().with_reqwest(http).with_service_target_resolver_fn(move |_|Ok(ServiceTarget{endpoint:target_endpoint.clone(),auth:target_auth.clone(),model:target_model.clone()})).build().map_err(|e|format!("Failed to build LLM client: {e}"))?;
 Ok((client,model_name))
}

async fn execute_llm_translation(device_dir:&Path,settings:&StoredSettings,text:&str)->R<(String,String)>{
 let trimmed=text.trim();
 if trimmed.is_empty(){return Err("Source description is empty".into());}
 if trimmed.chars().count()>MAX_SOURCE_CHARS{return Err(format!("Source text is too long (max {MAX_SOURCE_CHARS} characters)"));}
 let api_key=settings.credential_id.as_deref().and_then(|id|cred_read(device_dir,id).ok()).unwrap_or_default();
 let (client,model_name)=build_genai_client(settings,api_key)?;
 let prompt=if settings.system_prompt.trim().is_empty(){default_system_prompt(&settings.target_language)}else{settings.system_prompt.trim().to_owned()};
 let req=ChatRequest::from_system(prompt).append_message(ChatMessage::user(trimmed));
 let res=client.exec_chat(&model_name,req,None).await.map_err(|e|format!("LLM translation failed: {e}"))?;
 let translated=res.into_first_text().map(|s|s.trim().to_owned()).filter(|s|!s.is_empty()).ok_or_else(||"LLM returned an empty translation".to_owned())?;
 Ok((translated,model_name))
}

fn require_writable(a:&App,h:&HeaderMap)->Result<(),ApiError>{
 if crate::access::authenticate(a,h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}
 Ok(())
}
fn validate_kind(kind:&str)->R<&'static str>{match kind{"work"=>Ok("work"),"character"=>Ok("character"),_=>Err("Invalid translation target kind".into())}}
fn validate_id(id:&str)->R<&str>{let t=id.trim();if t.is_empty()||t.len()>128||t.chars().any(char::is_control){return Err("Invalid target id".into());}Ok(t)}

fn count_untranslated_works(c:&Connection,lang:&str)->i64{
 c.query_row("SELECT count(*) FROM works w WHERE trim(w.description)!='' AND NOT EXISTS(SELECT 1 FROM translations t WHERE t.target_kind='work' AND t.target_id=w.id AND t.target_lang=?1)",[lang],|r|r.get(0)).unwrap_or(0)
}

pub async fn get_settings(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let settings=load_settings(&a.device_dir);
 let untranslated=count_untranslated_works(&a.db.lock().unwrap(),&settings.target_language);
 Ok(Json(settings_json(&a.device_dir,&settings,untranslated)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch{
 pub provider:Option<Provider>,
 pub base_url:Option<String>,
 pub model:Option<String>,
 pub target_language:Option<String>,
 pub system_prompt:Option<String>,
 pub api_key:Option<String>,
 #[serde(default)]pub clear_api_key:bool,
}

pub async fn save_settings_route(State(a):State<App>,h:HeaderMap,Json(patch):Json<SettingsPatch>)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let mut settings=load_settings(&a.device_dir);
 if let Some(p)=patch.provider{settings.provider=p;}
 if let Some(url)=patch.base_url{settings.base_url=normalize_base_url(&url)?;}
 if let Some(model)=patch.model{
  let m=model.trim();if m.len()>160||m.chars().any(char::is_control){return Err(ApiError("Invalid model name".into()));}
  settings.model=m.to_owned();
 }
 if let Some(lang)=patch.target_language{
  let l=lang.trim();if l.is_empty()||l.len()>32||l.chars().any(char::is_control){return Err(ApiError("Invalid target language".into()));}
  settings.target_language=l.to_owned();
 }
 if let Some(prompt)=patch.system_prompt{
  if prompt.chars().count()>MAX_PROMPT_CHARS{return Err(ApiError(format!("System prompt exceeds {MAX_PROMPT_CHARS} characters")));}
  settings.system_prompt=prompt.trim().to_owned();
 }
 if patch.clear_api_key{
  if let Some(old_id)=settings.credential_id.take(){let _=cred_delete(&a.device_dir,&old_id);}
 }else if let Some(key)=patch.api_key{
  let k=key.trim();
  if !k.is_empty(){
   let id=settings.credential_id.clone().unwrap_or_else(||format!("llm-{}",uuid::Uuid::new_v4().simple()));
   cred_write(&a.device_dir,&id,k)?;
   settings.credential_id=Some(id);
  }
 }
 save_settings(&a.device_dir,&settings)?;
 let untranslated=count_untranslated_works(&a.db.lock().unwrap(),&settings.target_language);
 Ok(Json(settings_json(&a.device_dir,&settings,untranslated)))
}

#[derive(Deserialize,Default)]
#[serde(deny_unknown_fields)]
pub struct TestRequest{
 pub provider:Option<Provider>,
 pub base_url:Option<String>,
 pub model:Option<String>,
 pub api_key:Option<String>,
 pub target_language:Option<String>,
}

pub async fn test_connection(State(a):State<App>,h:HeaderMap,Json(req):Json<TestRequest>)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let mut settings=load_settings(&a.device_dir);
 if let Some(p)=req.provider{settings.provider=p;}
 if let Some(url)=req.base_url{settings.base_url=normalize_base_url(&url)?;}
 if let Some(m)=req.model{settings.model=m.trim().to_owned();}
 if let Some(l)=req.target_language{if !l.trim().is_empty(){settings.target_language=l.trim().to_owned();}}
 let api_key=match req.api_key.as_deref().map(str::trim).filter(|s|!s.is_empty()){
  Some(k)=>k.to_owned(),
  None=>settings.credential_id.as_deref().and_then(|id|cred_read(&a.device_dir,id).ok()).unwrap_or_default(),
 };
 let (client,model_name)=build_genai_client(&settings,api_key)?;
 let started=Instant::now();
 let chat=ChatRequest::from_system(default_system_prompt(&settings.target_language)).append_message(ChatMessage::user("A short visual novel story about summer memories."));
 let res=client.exec_chat(&model_name,chat,None).await.map_err(|e|ApiError(format!("Connection test failed: {e}")))?;
 let sample=res.into_first_text().map(|s|s.trim().to_owned()).filter(|s|!s.is_empty()).ok_or_else(||ApiError("Provider returned an empty reply".into()))?;
 Ok(Json(json!({"ok":true,"provider":settings.provider.as_str(),"model":model_name,"latency_ms":started.elapsed().as_millis() as u64,"sample":sample})))
}

#[derive(Deserialize,Default)]
pub struct GetQuery{pub lang:Option<String>,pub source_hash:Option<String>}

async fn get_translation_inner(a:App,h:HeaderMap,kind:&str,id:String,q:GetQuery)->crate::Result<Value>{
 crate::auth(&a,&h)?;
 let kind=validate_kind(kind)?;let id=validate_id(&id)?;
 let settings=load_settings(&a.device_dir);
 let lang=q.lang.as_deref().map(str::trim).filter(|s|!s.is_empty()).unwrap_or(&settings.target_language).to_owned();
 let configured=is_configured(&a.device_dir,&settings);
 let c=a.db.lock().unwrap();
 let current_hash=if kind=="work"{
  c.query_row("SELECT description FROM works WHERE id=?1",[id],|r|r.get::<_,String>(0)).optional()?.map(|d|source_hash(&d))
 }else{q.source_hash.clone()};
 let row=c.query_row(
  "SELECT target_lang,source_hash,translated_text,provider,model,updated FROM translations WHERE target_kind=?1 AND target_id=?2 AND target_lang=?3",
  params![kind,id,&lang],
  |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?)),
 ).optional()?;
 let translation=row.map(|(tlang,shash,text,provider,model,updated)|{
  let stale=current_hash.as_deref().is_some_and(|h|!h.is_empty()&&h!=shash);
  json!({"target_kind":kind,"target_id":id,"target_lang":tlang,"source_hash":shash,"translated_text":text,"provider":provider,"model":model,"updated":updated,"stale":stale})
 });
 Ok(Json(json!({"configured":configured,"target_language":lang,"translation":translation})))
}

pub async fn get_work_translation(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Query(q):Query<GetQuery>)->crate::Result<Value>{get_translation_inner(a,h,"work",id,q).await}
pub async fn get_character_translation(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Query(q):Query<GetQuery>)->crate::Result<Value>{get_translation_inner(a,h,"character",id,q).await}

#[derive(Deserialize,Default)]
#[serde(deny_unknown_fields)]
pub struct TranslateRequest{pub source_text:Option<String>,pub lang:Option<String>,#[serde(default)]pub force:bool}

async fn translate_one_inner(a:App,h:HeaderMap,kind:&str,id:String,req:TranslateRequest)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let kind=validate_kind(kind)?;let id=validate_id(&id)?.to_owned();
 let mut settings=load_settings(&a.device_dir);
 if let Some(l)=req.lang.as_deref().map(str::trim).filter(|s|!s.is_empty()){settings.target_language=l.to_owned();}
 let source_text=if let Some(text)=req.source_text.filter(|s|!s.trim().is_empty()){
  text
 }else if kind=="work"{
  a.db.lock().unwrap().query_row("SELECT description FROM works WHERE id=?1",[&id],|r|r.get::<_,String>(0)).optional()?.ok_or_else(||ApiError("Work not found".into()))?
 }else{
  return Err(ApiError("source_text is required for character translation".into()));
 };
 let shash=source_hash(&source_text);
 if !req.force{
  let cached=a.db.lock().unwrap().query_row(
   "SELECT target_lang,source_hash,translated_text,provider,model,updated FROM translations WHERE target_kind=?1 AND target_id=?2 AND target_lang=?3 AND source_hash=?4",
   params![kind,&id,&settings.target_language,&shash],
   |r|Ok(json!({"target_kind":kind,"target_id":&id,"target_lang":r.get::<_,String>(0)?,"source_hash":r.get::<_,String>(1)?,"translated_text":r.get::<_,String>(2)?,"provider":r.get::<_,String>(3)?,"model":r.get::<_,String>(4)?,"updated":r.get::<_,i64>(5)?,"stale":false,"cached":true})),
  ).optional()?;
  if let Some(hit)=cached{return Ok(Json(json!({"configured":true,"target_language":settings.target_language,"translation":hit})));}
 }
 let (translated,model_name)=execute_llm_translation(&a.device_dir,&settings,&source_text).await?;
 let now=db::now();
 {
  let c=a.db.lock().unwrap();
  c.execute(
   "INSERT INTO translations(target_kind,target_id,target_lang,source_hash,translated_text,provider,model,updated) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) \
    ON CONFLICT(target_kind,target_id,target_lang) DO UPDATE SET source_hash=excluded.source_hash,translated_text=excluded.translated_text,provider=excluded.provider,model=excluded.model,updated=excluded.updated",
   params![kind,&id,&settings.target_language,&shash,&translated,settings.provider.as_str(),&model_name,now],
  )?;
  let _=crate::catalog_changes::bump(&c);
 }
 Ok(Json(json!({
  "configured":true,
  "target_language":settings.target_language,
  "translation":{"target_kind":kind,"target_id":id,"target_lang":settings.target_language,"source_hash":shash,"translated_text":translated,"provider":settings.provider.as_str(),"model":model_name,"updated":now,"stale":false,"cached":false}
 })))
}

pub async fn translate_work(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Json(req):Json<TranslateRequest>)->crate::Result<Value>{translate_one_inner(a,h,"work",id,req).await}
pub async fn translate_character(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Json(req):Json<TranslateRequest>)->crate::Result<Value>{translate_one_inner(a,h,"character",id,req).await}

#[derive(Deserialize,Default)]
pub struct DeleteQuery{pub lang:Option<String>}

async fn delete_translation_inner(a:App,h:HeaderMap,kind:&str,id:String,q:DeleteQuery)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let kind=validate_kind(kind)?;let id=validate_id(&id)?;
 let settings=load_settings(&a.device_dir);
 let lang=q.lang.as_deref().map(str::trim).filter(|s|!s.is_empty()).unwrap_or(&settings.target_language);
 let c=a.db.lock().unwrap();
 let deleted=c.execute("DELETE FROM translations WHERE target_kind=?1 AND target_id=?2 AND target_lang=?3",params![kind,id,lang])?;
 if deleted>0{let _=crate::catalog_changes::bump(&c);}
 Ok(Json(json!({"deleted":deleted>0})))
}

pub async fn delete_work_translation(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Query(q):Query<DeleteQuery>)->crate::Result<Value>{delete_translation_inner(a,h,"work",id,q).await}
pub async fn delete_character_translation(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>,Query(q):Query<DeleteQuery>)->crate::Result<Value>{delete_translation_inner(a,h,"character",id,q).await}

pub async fn batch_status(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
 require_writable(&a,&h)?;
 Ok(Json(serde_json::to_value(a.translation_batch.snapshot()).unwrap()))
}

#[derive(Deserialize,Default)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest{pub limit:Option<usize>,#[serde(default)]pub force:bool}

pub async fn start_batch(State(a):State<App>,h:HeaderMap,Json(req):Json<BatchRequest>)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let settings=load_settings(&a.device_dir);
 if !is_configured(&a.device_dir,&settings){return Err(ApiError("Configure an LLM provider and API key first".into()));}
 {
  let s=a.translation_batch.state.lock().unwrap();
  if s.running{return Err(ApiError("A batch translation is already running".into()));}
 }
 let works:Vec<(String,String,String)>={
  let c=a.db.lock().unwrap();
  let mut stmt=c.prepare("SELECT w.id,w.title,w.description,t.source_hash FROM works w LEFT JOIN translations t ON t.target_kind='work' AND t.target_id=w.id AND t.target_lang=?1 WHERE trim(w.description)!='' ORDER BY w.title")?;
  let rows=stmt.query_map([&settings.target_language],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
  rows.into_iter().filter(|(_,_,desc,existing)|req.force||existing.as_deref()!=Some(source_hash(desc).as_str())).take(req.limit.unwrap_or(usize::MAX)).map(|(id,title,desc,_)|(id,title,desc)).collect()
 };
 let total=works.len();
 {
  let mut s=a.translation_batch.state.lock().unwrap();
  *s=BatchStatus{running:total>0,total,translated:0,failed:0,skipped:0,current_title:None,last_error:None,started_at:Some(db::now()),finished_at:if total==0{Some(db::now())}else{None}};
 }
 if total==0{return Ok(Json(serde_json::to_value(a.translation_batch.snapshot()).unwrap()));}
 let app=a.clone();
 let runner=Arc::clone(&a.translation_batch);
 let handle=tokio::spawn(async move{
  for (id,title,desc) in works{
   {runner.state.lock().unwrap().current_title=Some(title.clone());}
   let shash=source_hash(&desc);
   match execute_llm_translation(&app.device_dir,&settings,&desc).await{
    Ok((translated,model_name))=>{
     let now=db::now();
     let saved={
      let c=app.db.lock().unwrap();
      let res=c.execute("INSERT INTO translations(target_kind,target_id,target_lang,source_hash,translated_text,provider,model,updated) VALUES('work',?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(target_kind,target_id,target_lang) DO UPDATE SET source_hash=excluded.source_hash,translated_text=excluded.translated_text,provider=excluded.provider,model=excluded.model,updated=excluded.updated",params![&id,&settings.target_language,&shash,&translated,settings.provider.as_str(),&model_name,now]);
      if res.is_ok(){let _=crate::catalog_changes::bump(&c);}
      res
     };
     let mut s=runner.state.lock().unwrap();
     match saved{Ok(_)=>s.translated+=1,Err(e)=>{s.failed+=1;s.last_error=Some(e.to_string());}}
    }
    Err(e)=>{let mut s=runner.state.lock().unwrap();s.failed+=1;s.last_error=Some(e);}
   }
   tokio::time::sleep(Duration::from_millis(BATCH_PACING_MS)).await;
  }
  let mut s=runner.state.lock().unwrap();s.running=false;s.current_title=None;s.finished_at=Some(db::now());
  *runner.abort.lock().unwrap()=None;
 });
 *a.translation_batch.abort.lock().unwrap()=Some(handle.abort_handle());
 Ok(Json(serde_json::to_value(a.translation_batch.snapshot()).unwrap()))
}

pub async fn cancel_batch(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
 require_writable(&a,&h)?;
 let cancelled=a.translation_batch.cancel();
 Ok(Json(json!({"cancelled":cancelled,"status":a.translation_batch.snapshot()})))
}

#[cfg(test)]
mod tests{
 use super::*;
 use axum::{routing::post as axum_post,Router};

 async fn mock_llm_server()->(String,tokio::task::JoinHandle<()>){
  let app=Router::new()
   .route("/v1/chat/completions",axum_post(|Json(body):Json<Value>|async move{
    let text=body["messages"].as_array().and_then(|m|m.last()).and_then(|m|m["content"].as_str()).unwrap_or("");
    Json(json!({"id":"cmpl-1","object":"chat.completion","model":"gpt-test","choices":[{"index":0,"message":{"role":"assistant","content":format!("OPENAI_ZH:{text}")},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":5,"total_tokens":10}}))
   }))
   .route("/v1/messages",axum_post(|Json(body):Json<Value>|async move{
    let text=body["messages"].as_array().and_then(|m|m.last()).and_then(|m|m["content"].as_str()).unwrap_or("");
    Json(json!({"id":"msg-1","type":"message","role":"assistant","model":"claude-test","content":[{"type":"text","text":format!("CLAUDE_ZH:{text}")}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":5}}))
   }))
   .route("/v1beta/models/{model_action}",axum_post(|Json(body):Json<Value>|async move{
    let text=body["contents"].as_array().and_then(|c|c.first()).and_then(|c|c["parts"].as_array()).and_then(|p|p.first()).and_then(|p|p["text"].as_str()).unwrap_or("");
    Json(json!({"candidates":[{"content":{"role":"model","parts":[{"text":format!("GEMINI_ZH:{text}")}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":5,"totalTokenCount":10}}))
   }));
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr=listener.local_addr().unwrap();
  let task=tokio::spawn(async move{axum::serve(listener,app).await.unwrap()});
  (format!("http://{addr}"),task)
 }

 #[test]
 fn base_url_normalization_and_validation(){
  assert_eq!(normalize_base_url("https://api.example.com/v1").unwrap(),"https://api.example.com/v1/");
  assert_eq!(normalize_base_url("http://127.0.0.1:11434/v1/").unwrap(),"http://127.0.0.1:11434/v1/");
  assert_eq!(normalize_base_url("  ").unwrap(),"");
  assert!(normalize_base_url("ftp://example.com").is_err());
  assert!(normalize_base_url("https://user:pass@example.com/v1").is_err());
  assert!(normalize_base_url("https://example.com/v1?key=1").is_err());
 }

 #[tokio::test]
 async fn genai_supports_openai_claude_and_gemini_protocols_with_caching_and_staleness(){
  let (base,server)=mock_llm_server().await;
  let t=tempfile::tempdir().unwrap();
  let app=crate::initialize(t.path().join("state")).unwrap();
  app.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,description) VALUES('w1','Summer','サマー','Hello [spoiler]secret[/spoiler]')",[]).unwrap();
  let mut owner=HeaderMap::new();
  owner.insert(axum::http::header::AUTHORIZATION,format!("Bearer {}",app.token).parse().unwrap());

  // 1. OpenAI-compatible
  let saved=save_settings_route(State(app.clone()),owner.clone(),Json(SettingsPatch{provider:Some(Provider::Openai),base_url:Some(format!("{base}/v1")),model:Some("gpt-test".into()),target_language:Some("zh-TW".into()),system_prompt:None,api_key:Some("test-openai-key".into()),clear_api_key:false})).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(saved["has_api_key"],true);assert_eq!(saved["configured"],true);
  let res=translate_work(State(app.clone()),owner.clone(),ApiPath("w1".into()),Json(TranslateRequest::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(res["translation"]["translated_text"],"OPENAI_ZH:Hello [spoiler]secret[/spoiler]");
  assert_eq!(res["translation"]["cached"],false);
  // Second call returns cached without hitting provider
  let hit=translate_work(State(app.clone()),owner.clone(),ApiPath("w1".into()),Json(TranslateRequest::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(hit["translation"]["cached"],true);
  // Updating description marks translation stale
  app.db.lock().unwrap().execute("UPDATE works SET description='Updated text' WHERE id='w1'",[]).unwrap();
  let fetched=get_work_translation(State(app.clone()),owner.clone(),ApiPath("w1".into()),Query(GetQuery::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(fetched["translation"]["stale"],true);

  // 2. Claude-compatible
  let _=save_settings_route(State(app.clone()),owner.clone(),Json(SettingsPatch{provider:Some(Provider::Claude),base_url:Some(format!("{base}/v1")),model:Some("claude-test".into()),target_language:None,system_prompt:None,api_key:Some("test-claude-key".into()),clear_api_key:false})).await.map_err(|e|e.0).unwrap();
  let res=translate_work(State(app.clone()),owner.clone(),ApiPath("w1".into()),Json(TranslateRequest{source_text:None,lang:None,force:true})).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(res["translation"]["translated_text"],"CLAUDE_ZH:Updated text");
  assert_eq!(res["translation"]["provider"],"claude");

  // 3. Gemini-compatible + character translation
  let _=save_settings_route(State(app.clone()),owner.clone(),Json(SettingsPatch{provider:Some(Provider::Gemini),base_url:Some(format!("{base}/v1beta")),model:Some("gemini-2.5-flash".into()),target_language:None,system_prompt:None,api_key:Some("test-gemini-key".into()),clear_api_key:false})).await.map_err(|e|e.0).unwrap();
  let res=translate_character(State(app.clone()),owner.clone(),ApiPath("c123".into()),Json(TranslateRequest{source_text:Some("Heroine bio".into()),lang:None,force:false})).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(res["translation"]["translated_text"],"GEMINI_ZH:Heroine bio");
  assert_eq!(res["translation"]["provider"],"gemini");

  // 4. Test connection endpoint
  let tested=test_connection(State(app.clone()),owner.clone(),Json(TestRequest::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(tested["ok"],true);assert_eq!(tested["provider"],"gemini");

  // 5. Batch translation translates untranslated work
  app.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,description) VALUES('w2','Winter','ウィンター','Cold snow')",[]).unwrap();
  let started=start_batch(State(app.clone()),owner.clone(),Json(BatchRequest::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(started["total"],1);
  for _ in 0..30{if !app.translation_batch.snapshot().running{break;}tokio::time::sleep(Duration::from_millis(50)).await;}
  let done=app.translation_batch.snapshot();
  assert!(!done.running);assert_eq!(done.translated,1);assert_eq!(done.failed,0);
  let w2=get_work_translation(State(app.clone()),owner.clone(),ApiPath("w2".into()),Query(GetQuery::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(w2["translation"]["translated_text"],"GEMINI_ZH:Cold snow");

  // 6. Delete translation & clear key
  let del=delete_character_translation(State(app.clone()),owner.clone(),ApiPath("c123".into()),Query(DeleteQuery::default())).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(del["deleted"],true);
  let cleared=save_settings_route(State(app.clone()),owner.clone(),Json(SettingsPatch{provider:None,base_url:Some("https://api.openai.com/v1".into()),model:None,target_language:None,system_prompt:None,api_key:None,clear_api_key:true})).await.map_err(|e|e.0).unwrap().0;
  assert_eq!(cleared["has_api_key"],false);assert_eq!(cleared["configured"],false);

  server.abort();
 }
}

//! Device-local backup schedule, shared export gate, and conservative retention.
use crate::{App,ApiError,backup::{self,Mode,Origin},db,plans};
use axum::{extract::{State,Path as ApiPath},http::HeaderMap,Json};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{fs,io::Write,path::{Path,PathBuf},sync::{Arc,Mutex},time::Duration};
type R<T>=Result<T,String>;
const DAY:i64=86400;
#[derive(Clone,Copy,Debug,Deserialize,Serialize)]#[serde(rename_all="lowercase")]pub enum Frequency{Daily,Weekly}
#[derive(Clone,Deserialize,Serialize)]#[serde(deny_unknown_fields)]pub struct Settings{enabled:bool,destination:String,frequency:Frequency,minute_utc:u16,weekday:Option<u8>,keep:u8,mode:Mode,artwork:bool}
#[derive(Clone,Deserialize,Serialize)]#[serde(deny_unknown_fields)]struct Stored{version:u32,enabled:bool,destination:String,frequency:Frequency,minute_utc:u16,weekday:Option<u8>,keep:u8,mode:Mode,artwork:bool,last_started:Option<i64>,last_success:Option<i64>,last_error:Option<String>,last_path:Option<String>}
impl Default for Stored{fn default()->Self{Self{version:1,enabled:false,destination:String::new(),frequency:Frequency::Daily,minute_utc:1230,weekday:None,keep:14,mode:Mode::Full,artwork:true,last_started:None,last_success:None,last_error:None,last_path:None}}}
impl Stored{
 fn period(&self)->i64{match self.frequency{Frequency::Daily=>DAY,Frequency::Weekly=>7*DAY}}
 // weekday follows UTC Sunday=0, matching the browser's getUTCDay().
 fn slot(&self,now:i64)->i64{let day=now.div_euclid(DAY);let mut slot=day*DAY+i64::from(self.minute_utc)*60;if let Frequency::Weekly=self.frequency{slot-=(day+4-i64::from(self.weekday.unwrap_or(0))).rem_euclid(7)*DAY;}if slot>now{slot-=self.period();}slot}
 fn validate(&self)->R<()>{if self.version!=1{return Err("Unsupported backup-schedule.json version".into());}if self.minute_utc>=1440||!(1..=100).contains(&self.keep)||self.weekday.is_some_and(|v|v>6)||matches!(self.frequency,Frequency::Weekly)&&self.weekday.is_none(){return Err("Invalid backup schedule time, weekday or keep (1-100)".into());}Ok(())}
 fn configure(&mut self,v:Settings){self.enabled=v.enabled;self.destination=v.destination;self.frequency=v.frequency;self.minute_utc=v.minute_utc;self.weekday=v.weekday;self.keep=v.keep;self.mode=v.mode;self.artwork=v.artwork;}
}
fn validate_file(path:&Path)->R<()>{plans::validate_chain(path)?;if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){return Err("backup-schedule.json must not be a symbolic link".into());}Ok(())}
fn load(path:&Path)->R<Stored>{validate_file(path)?;let bytes=match fs::read(path){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Stored::default()),Err(e)=>return Err(e.to_string())};let value:Stored=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;value.validate()?;Ok(value)}
fn save(path:&Path,value:&Stored)->R<()>{
 validate_file(path)?;let temp=path.with_file_name(format!(".backup-schedule-{}.tmp",db::id()));plans::validate_chain(&temp)?;
 let result=(||{let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;file.write_all(&serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);validate_file(path)?;fs::rename(&temp,path).map_err(|e|e.to_string())})();if result.is_err(){let _=fs::remove_file(temp);}result
}
struct Pending{since:i64,retry:i64,requested:bool}
struct Runtime{stored:Stored,started:i64,observed:i64,catchup_checked:bool,pending:Option<Pending>}
type Clock=Arc<dyn Fn()->i64+Send+Sync>;
type Space=Arc<dyn Fn(&Path)->R<u64>+Send+Sync>;
pub struct Schedule{path:PathBuf,state:Mutex<Runtime>,gate:Arc<tokio::sync::Mutex<()>>,clock:Clock,space:Space}
impl Schedule{
 pub fn load(device_dir:&Path)->Arc<Self>{Self::with_sources(device_dir,Arc::new(db::now),Arc::new(|p|fs2::available_space(p).map_err(|e|e.to_string())))}
 fn with_sources(device_dir:&Path,clock:Clock,space:Space)->Arc<Self>{let path=device_dir.join("backup-schedule.json");let stored=load(&path).unwrap_or_else(|e|{crate::diagnostics::record("warn","backups.state",&format!("Ignoring damaged backup-schedule.json: {e}"),None);Stored::default()});let now=clock();Arc::new(Self{path,state:Mutex::new(Runtime{stored,started:now,observed:now,catchup_checked:false,pending:None}),gate:Default::default(),clock,space})}
 fn snapshot(&self)->Value{
  let state=self.state.lock().unwrap();let s=&state.stored;let now=(self.clock)();let mut value=serde_json::to_value(s).unwrap();value["running"]=json!(self.gate.try_lock().is_err());
  let slot=s.slot(now);let mut next=if slot>state.observed&&s.last_success.is_none_or(|t|t<slot){now}else{slot+s.period()};
  if !state.catchup_checked&&s.last_success.is_none_or(|t|now-t>=s.period()){next=next.min((state.started+300).max(now));}
  if s.last_error.is_some(){if let Some(t)=s.last_started{next=(t+3600).max(now);}}if let Some(p)=&state.pending{next=if s.last_error.is_some(){next.max(p.retry)}else{p.retry.max(now)};}
  value["next_run"]=json!((s.enabled||state.pending.as_ref().is_some_and(|p|p.requested)).then_some(next));value["available_bytes"]=json!((self.space)(Path::new(&s.destination)).ok());value
 }
 fn configure(&self,a:&App,v:Settings)->R<()>{let _gate=self.gate.try_lock().map_err(|_|"A backup is already running. Retry in a moment.")?;let mut state=self.state.lock().unwrap();let mut next=state.stored.clone();next.configure(v);next.validate()?;next.destination=destination(a,Path::new(&next.destination))?.to_string_lossy().into();save(&self.path,&next)?;state.stored=next;state.pending=None;state.observed=(self.clock)();Ok(())}
 fn tick(&self,a:&App)->R<Option<PathBuf>>{self.run(a,None)}
 fn run(&self,a:&App,role:Option<&str>)->R<Option<PathBuf>>{
  let _gate=match self.gate.try_lock(){Ok(g)=>g,Err(_) if role.is_none()=>return Ok(None),Err(_)=>return Err("A backup is already running. Retry in a moment.".into())};let now=(self.clock)();
  let settings={let mut state=self.state.lock().unwrap();let s=&state.stored;
   if let Some(role)=role{permission(role,s.mode)?;}else if !s.enabled&&!state.pending.as_ref().is_some_and(|p|p.requested){state.pending=None;state.observed=now;return Ok(None);}
   // A failed attempt is retried hourly, including across restarts and manual Run clicks.
   if s.last_error.is_some()&&s.last_started.is_some_and(|t|now-t<3600){return if role.is_some(){Err("Backup retry is limited to once per hour".into())}else{Ok(None)};}
   let due=s.slot(now)>state.observed&&s.last_success.is_none_or(|t|t<s.slot(now));
   let catchup=!state.catchup_checked&&now>=state.started+300&&s.last_success.is_none_or(|t|now-t>=s.period());
   let retry=s.last_error.is_some()&&s.last_started.is_some_and(|t|now-t>=3600);
   state.observed=now;if now>=state.started+300{state.catchup_checked=true;}
   if role.is_some()||due||catchup||retry{state.pending.get_or_insert(Pending{since:now,retry:now,requested:role.is_some()});}
   let Some(p)=&state.pending else{return Ok(None)};if now<p.retry{return Ok(None);}state.stored.clone()
  };
  // Plan execution holds mutation_lock throughout, so holding it from this check to the end of the export keeps plans from starting mid-backup.
  let mut mutation=a.mutation_lock.try_lock().ok();
  let executing=if mutation.is_none(){Ok(true)}else{a.db.lock().map_err(|e|e.to_string()).and_then(|c|c.query_row("SELECT EXISTS(SELECT 1 FROM plans WHERE state='executing')",[],|r|r.get::<_,bool>(0)).map_err(|e|e.to_string()))};
  {let mut state=self.state.lock().unwrap();let p=state.pending.as_mut().unwrap();if executing.as_ref().is_ok_and(|v|*v)&&now-p.since<6*3600{p.retry=now+600;return Ok(None);}state.pending=None;state.stored.last_started=Some(now);}
  if mutation.is_none(){mutation=a.mutation_lock.lock().ok();}let _mutation=mutation;let executing=executing.map(|_|());
  let result:R<PathBuf>=(||{save(&self.path,&self.state.lock().unwrap().stored)?;executing?;preflight(a,Path::new(&settings.destination),&*self.space)?;let path=backup::export_with(&a.db,Path::new(&settings.destination),Some(&a.device_dir),backup::Options{mode:settings.mode,origin:Origin::Scheduled,artwork:settings.artwork})?;retain(a,Path::new(&settings.destination),settings.keep,Some(&path))?;Ok(path)})();
  let mut state=self.state.lock().unwrap();match &result{Ok(path)=>{state.stored.last_success=Some((self.clock)());state.stored.last_error=None;state.stored.last_path=Some(path.to_string_lossy().into());},Err(e)=>{state.stored.last_error=Some(e.clone());crate::diagnostics::record("error","backups.run",e,None);}}
  if let Err(e)=save(&self.path,&state.stored){state.stored.last_error=Some(e.clone());crate::diagnostics::record("error","backups.save",&e,None);return Err(e);}result.map(Some)
 }
 pub(crate) fn export(&self,a:&App,path:&Path,mode:Mode)->R<PathBuf>{let _gate=self.gate.try_lock().map_err(|_|"A backup is already running. Retry in a moment.")?;let _mutation=a.mutation_lock.try_lock().map_err(|_|"Another change is in progress. Retry in a moment.")?;preflight(a,path,&*self.space)?;backup::export_with(&a.db,path,Some(&a.device_dir),backup::Options{mode,..Default::default()})}
}
fn destination(a:&App,path:&Path)->R<PathBuf>{
 plans::validate_chain(path)?;let path=path.canonicalize().map_err(|_|"Choose an existing backup destination folder")?;if !path.is_dir(){return Err("Choose an existing backup destination folder".into());}
 if path.starts_with(a.state_dir.canonicalize().map_err(|e|e.to_string())?){return Err("Backup destination must be outside the data directory".into());}
 let roots={let c=a.db.lock().map_err(|e|e.to_string())?;let mut q=c.prepare("SELECT path FROM roots").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;rows};
 for root in roots{let root=Path::new(&root);let root=root.canonicalize().unwrap_or_else(|_|root.to_path_buf());if path.starts_with(root){return Err("Backup destination must be outside library roots".into());}}
 let probe=path.join(format!(".galroon-backup-probe-{}",db::id()));let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&probe).map_err(|e|format!("Backup destination is not writable: {e}"))?;let result=file.write_all(b"backup preflight").and_then(|_|file.sync_all());drop(file);let removed=fs::remove_file(probe);result.and(removed).map_err(|e|e.to_string())?;Ok(path)
}
fn preflight(a:&App,path:&Path,space:&(dyn Fn(&Path)->R<u64>+Send+Sync))->R<()>{
 let path=destination(a,path)?;
 // page_count includes committed WAL pages that have not reached the main file yet.
 let catalog={let c=a.db.lock().map_err(|e|e.to_string())?;c.query_row("SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()",[],|r|r.get::<_,i64>(0).map(|v|v as u64)).map_err(|e|e.to_string())?};
 let artwork=if a.state_dir.join(crate::artwork_library::FILE).exists(){rusqlite::Connection::open_with_flags(a.state_dir.join(crate::artwork_library::FILE),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?.query_row("SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()",[],|r|r.get::<_,i64>(0).map(|v|v as u64)).map_err(|e|e.to_string())?}else{0};
 let required=(catalog+artwork).saturating_mul(6).div_ceil(5).saturating_add(1<<30);let available=space(&path)?;if available<required{return Err(format!("Insufficient backup space: need {required} bytes, available {available}"));}Ok(())
}
fn folder_manifest(path:&Path,library:&str,origin:Origin)->R<backup::Manifest>{
 if !path.file_name().is_some_and(|n|n.to_string_lossy().starts_with("Galroon-backup-")){return Err("Not a Galroon backup folder".into());}plans::validate_chain(path)?;let meta=fs::symlink_metadata(path).map_err(|e|e.to_string())?;if !meta.is_dir()||meta.file_type().is_symlink(){return Err("Backup must be a non-linked folder".into());}
 let file=path.join("manifest.json");plans::validate_chain(&file)?;let meta=fs::symlink_metadata(&file).map_err(|e|e.to_string())?;if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>16384{return Err("Invalid backup manifest file".into());}let m:backup::Manifest=serde_json::from_slice(&fs::read(file).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if m.format!="galroon-catalog-backup"||!(1..=4).contains(&m.version)||m.origin!=origin||m.library.as_ref().is_none_or(|l|l.id!=library){return Err("Backup origin or library does not match".into());}Ok(m)
}
fn library_id(a:&App)->R<String>{Ok(crate::context::library(&*a.db.lock().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?.id)}
fn retain(a:&App,path:&Path,keep:u8,newest:Option<&Path>)->R<()>{let path=destination(a,path)?;let library=library_id(a)?;let mut candidates=Vec::new();for entry in fs::read_dir(path).map_err(|e|e.to_string())?{let path=entry.map_err(|e|e.to_string())?.path();if let Ok(m)=folder_manifest(&path,&library,Origin::Scheduled){let latest=newest==Some(path.as_path());candidates.push((m.created,latest,path));}}candidates.sort_by(|a,b|b.cmp(a));for (_,_,path) in candidates.into_iter().skip(keep as usize){folder_manifest(&path,&library,Origin::Scheduled)?;destination(a,&path)?;fs::remove_dir_all(&path).map_err(|e|e.to_string())?;mark_deleted(a,&path)?;}Ok(())}
fn mark_deleted(a:&App,path:&Path)->R<()>{a.db.lock().map_err(|e|e.to_string())?.execute("UPDATE backup_history SET deleted=1 WHERE path=?1",[path.to_string_lossy().as_ref()]).map_err(|e|e.to_string())?;Ok(())}
fn delete_manual(a:&App,id:&str)->R<()>{
 let _gate=a.backup_schedule.gate.try_lock().map_err(|_|"A backup is already running. Retry in a moment.")?;
 let(path,origin,deleted):(String,String,bool)=a.db.lock().map_err(|e|e.to_string())?.query_row("SELECT path,origin,deleted FROM backup_history WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
 if origin!="manual"{return Err("Only manual backups can be deleted".into());}if deleted{return Ok(());}let path=Path::new(&path);destination(a,path.parent().ok_or("Backup has no parent")?)?;folder_manifest(path,&library_id(a)?,Origin::Manual)?;destination(a,path)?;fs::remove_dir_all(path).map_err(|e|e.to_string())?;mark_deleted(a,path)
}
fn permission(role:&str,mode:Mode)->R<()>{if !matches!(role,"owner"|"desktop"|"web_owner"){return Err("Desktop access required".into());}if mode==Mode::Full&&role!="owner"{return Err("Owner access required for full backups".into());}Ok(())}
pub struct Worker{task:tokio::task::JoinHandle<()>}
impl Worker{pub fn request_stop(&self){self.task.abort();}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();}}
pub fn start(a:App)->Worker{Worker{task:tokio::spawn(async move{loop{tokio::time::sleep(Duration::from_secs(60)).await;let a=a.clone();let _=tokio::task::spawn_blocking(move||a.backup_schedule.tick(&a)).await;}})}}
pub async fn get(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{permission(&crate::access::authenticate(&a,&h)?.role,Mode::Portable)?;Ok(Json(a.backup_schedule.snapshot()))}
pub async fn settings(State(a):State<App>,h:HeaderMap,Json(v):Json<Settings>)->crate::Result<Value>{permission(&crate::access::authenticate(&a,&h)?.role,v.mode)?;let result=tokio::task::spawn_blocking(move||{a.backup_schedule.configure(&a,v)?;Ok::<_,String>(a.backup_schedule.snapshot())}).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(result))}
pub async fn run(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{let role=crate::access::authenticate(&a,&h)?.role;permission(&role,Mode::Portable)?;let path=tokio::task::spawn_blocking(move||a.backup_schedule.run(&a,Some(&role))).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"deferred":path.is_none(),"path":path})))}
pub async fn delete(State(a):State<App>,h:HeaderMap,ApiPath(id):ApiPath<String>)->crate::Result<Value>{permission(&crate::access::authenticate(&a,&h)?.role,Mode::Portable)?;tokio::task::spawn_blocking(move||delete_manual(&a,&id)).await.map_err(|e|ApiError(e.to_string()))??;Ok(Json(json!({"deleted":true})))}

#[cfg(test)]mod tests{
 use super::*;
 use std::sync::atomic::{AtomicI64,AtomicU64,Ordering};
 use axum::{body::{Body,to_bytes},http::Request};
 use tower::ServiceExt;
 struct Fixture{_temp:tempfile::TempDir,app:App,target:PathBuf,clock:Arc<AtomicI64>,space:Arc<AtomicU64>}
 impl Fixture{
  fn new()->Self{let temp=tempfile::tempdir().unwrap();let mut app=crate::initialize(temp.path().join("state")).unwrap();let target=temp.path().join("backups");fs::create_dir(&target).unwrap();let clock=Arc::new(AtomicI64::new(10*DAY));let space=Arc::new(AtomicU64::new(u64::MAX));let c=clock.clone();let s=space.clone();app.backup_schedule=Schedule::with_sources(&app.device_dir,Arc::new(move||c.load(Ordering::SeqCst)),Arc::new(move |_|Ok(s.load(Ordering::SeqCst))));let f=Self{_temp:temp,app,target,clock,space};f.configure();f}
  fn config(&self)->Settings{Settings{enabled:true,destination:self.target.to_string_lossy().into(),frequency:Frequency::Daily,minute_utc:1,weekday:None,keep:3,mode:Mode::Portable,artwork:true}}
  fn configure(&self){self.app.backup_schedule.configure(&self.app,self.config()).unwrap();}
  fn time(&self,t:i64){self.clock.store(10*DAY+t,Ordering::SeqCst);}
  fn tick(&self)->R<Option<PathBuf>>{self.app.backup_schedule.tick(&self.app)}
  fn count(&self)->usize{fs::read_dir(&self.target).unwrap().count()}
  fn plan(&self){self.app.db.lock().unwrap().execute_batch("INSERT INTO plans(id,kind,state,items,digest,created) VALUES('p','move','executing','[]','test',0)").unwrap();}
 }
 #[test]fn daily_due_runs_once_and_weekly_uses_utc_weekday(){
  let f=Fixture::new();f.time(59);assert!(f.tick().unwrap().is_none());f.time(60);let path=f.tick().unwrap().unwrap();assert_eq!(backup::inspect(&path).unwrap().origin,Origin::Scheduled);assert_eq!(f.count(),1);for t in [61,120,300,DAY-1]{f.time(t);assert!(f.tick().unwrap().is_none());}f.time(DAY+60);assert!(f.tick().unwrap().is_some());assert_eq!(f.count(),2);
  let mut s=Stored::default();s.frequency=Frequency::Weekly;s.weekday=Some(0);s.minute_utc=60;assert_eq!(s.slot(10*DAY+3599),3*DAY+3600);assert_eq!(s.slot(10*DAY+3600),10*DAY+3600);assert_eq!(s.slot(17*DAY+3600),17*DAY+3600);
 }
 #[test]fn startup_catches_up_once_after_five_minutes_and_recent_success_skips_it(){
  let f=Fixture::new();{let mut state=f.app.backup_schedule.state.lock().unwrap();state.stored.minute_utc=0;state.stored.last_success=Some(DAY);}
  f.time(299);assert!(f.tick().unwrap().is_none());f.time(300);assert!(f.tick().unwrap().is_some());for t in [360,600,3600]{f.time(t);assert!(f.tick().unwrap().is_none());}assert_eq!(f.count(),1);
  let loaded=Schedule::with_sources(&f.app.device_dir,f.app.backup_schedule.clock.clone(),f.app.backup_schedule.space.clone());f.time(4000);assert!(loaded.tick(&f.app).unwrap().is_none());assert_eq!(f.count(),1);
 }
 #[test]fn insufficient_space_fails_without_partial_output_and_retries_hourly(){
  let f=Fixture::new();crate::artwork_library::Library::open(&f.app.state_dir).unwrap().save_profile("exploration.v4.work.v1.1",&json!({"large":"x".repeat(100_000)})).unwrap();f.space.store(1<<30,Ordering::SeqCst);f.time(60);assert!(f.tick().unwrap_err().contains("Insufficient backup space"));assert_eq!(f.count(),0);assert!(backup::history(&f.app.db).unwrap().as_array().unwrap().is_empty());
  let state=load(&f.app.device_dir.join("backup-schedule.json")).unwrap();assert_eq!(state.last_started,Some(10*DAY+60));assert!(state.last_error.unwrap().contains("Insufficient"));assert!(state.last_success.is_none());
  f.space.store(u64::MAX,Ordering::SeqCst);for t in [120,300,3659]{f.time(t);assert!(f.tick().unwrap().is_none());}assert!(f.app.backup_schedule.run(&f.app,Some("owner")).is_err());f.time(3660);assert!(f.tick().unwrap().is_some());assert!(f.app.backup_schedule.snapshot()["last_error"].is_null());assert_eq!(f.count(),1);
 }
 #[test]fn artwork_space_is_counted_even_when_artwork_is_disabled(){
  let f=Fixture::new();let mut settings=f.config();settings.artwork=false;f.app.backup_schedule.configure(&f.app,settings).unwrap();let catalog=f.app.db.lock().unwrap().query_row("SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()",[],|r|r.get::<_,i64>(0).map(|v|v as u64)).unwrap();f.space.store((catalog*6).div_ceil(5)+(1<<30),Ordering::SeqCst);
  preflight(&f.app,&f.target,&*f.app.backup_schedule.space).unwrap();crate::artwork_library::Library::open(&f.app.state_dir).unwrap().save_profile("exploration.v4.work.v1.1",&json!({"profile":"x".repeat(100_000)})).unwrap();assert!(preflight(&f.app,&f.target,&*f.app.backup_schedule.space).unwrap_err().contains("Insufficient"));assert_eq!(f.count(),0);
 }
 #[test]fn executing_plan_defers_every_ten_minutes_for_at_most_six_hours(){
  let f=Fixture::new();f.plan();f.time(60);assert!(f.tick().unwrap().is_none());assert_eq!(f.app.backup_schedule.state.lock().unwrap().pending.as_ref().unwrap().retry,10*DAY+660);assert_eq!(f.count(),0);
  for t in [120,659,660,6*3600]{f.time(t);assert!(f.tick().unwrap().is_none());}f.time(6*3600+600);assert!(f.tick().unwrap().is_some());assert_eq!(f.count(),1);
 }
 #[test]fn held_mutation_lock_defers_scheduled_and_manual_backups(){
  let f=Fixture::new();f.time(60);{let _held=f.app.mutation_lock.lock().unwrap();assert!(f.tick().unwrap().is_none());assert_eq!(f.app.backup_schedule.state.lock().unwrap().pending.as_ref().unwrap().retry,10*DAY+660);assert!(f.app.backup_schedule.export(&f.app,&f.target,Mode::Portable).unwrap_err().ends_with("Retry in a moment."));assert_eq!(f.count(),0);}
  f.time(660);assert!(f.tick().unwrap().is_some());assert_eq!(f.count(),1);
 }
 #[test]fn deferred_explicit_run_works_when_disabled_and_resumes_after_plan_finishes(){
  let f=Fixture::new();let mut settings=f.config();settings.enabled=false;f.app.backup_schedule.configure(&f.app,settings).unwrap();f.plan();assert!(f.app.backup_schedule.run(&f.app,Some("owner")).unwrap().is_none());f.app.db.lock().unwrap().execute("UPDATE plans SET state='completed'",[]).unwrap();f.time(599);assert!(f.tick().unwrap().is_none());f.time(600);assert!(f.tick().unwrap().is_some());assert_eq!(f.count(),1);
 }
 fn sample(f:&Fixture,name:&str,created:i64,origin:Origin,library:&str)->PathBuf{
  let path=f.target.join(name);fs::create_dir(&path).unwrap();let m=backup::Manifest{format:"galroon-catalog-backup".into(),version:4,created,database:"collection.sqlite".into(),bytes:0,sha256:String::new(),includes_game_files:false,includes_credentials:false,library:Some(crate::context::Library{id:library.into(),..crate::context::library(&f.app.db.lock().unwrap()).unwrap()}),schema:Some(crate::db::SCHEMA_VERSION),tables:vec![],verification:"integrity-and-hash".into(),mode:Mode::Portable,origin,identity:None,core_version:None,artwork:None};fs::write(path.join("manifest.json"),serde_json::to_vec(&m).unwrap()).unwrap();path
 }
 #[test]fn retention_keeps_three_own_scheduled_and_preserves_everything_else(){
  let f=Fixture::new();let library=library_id(&f.app).unwrap();let mut own=Vec::new();for i in 0..5{own.push(sample(&f,&format!("Galroon-backup-own-{i}"),i,Origin::Scheduled,&library));}
  let manual=sample(&f,"Galroon-backup-manual",-10,Origin::Manual,&library);let other=sample(&f,"Galroon-backup-other",-10,Origin::Scheduled,"another-library");let unrelated=sample(&f,"unrelated-folder",-10,Origin::Scheduled,&library);let broken=f.target.join("Galroon-backup-broken");fs::create_dir(&broken).unwrap();fs::write(broken.join("manifest.json"),b"bad").unwrap();let file=f.target.join("Galroon-backup-file");fs::write(&file,b"file").unwrap();
  #[cfg(unix)]{std::os::unix::fs::symlink(&own[0],f.target.join("Galroon-backup-link")).unwrap();let linked=f.target.join("Galroon-backup-linked-manifest");fs::create_dir(&linked).unwrap();std::os::unix::fs::symlink(own[0].join("manifest.json"),linked.join("manifest.json")).unwrap();}
  // Retention scans manifests even when the restored history has no corresponding rows.
  assert_eq!(backup::history(&f.app.db).unwrap(),json!([]));retain(&f.app,&f.target,3,None).unwrap();for (i,path) in own.iter().enumerate(){assert_eq!(path.exists(),i>=2);}for path in [manual,other,unrelated,broken,file]{assert!(path.exists());}
  #[cfg(unix)]{assert!(fs::symlink_metadata(f.target.join("Galroon-backup-link")).unwrap().file_type().is_symlink());assert!(f.target.join("Galroon-backup-linked-manifest").is_dir());}
 }
 #[test]fn successful_run_prunes_to_keep_and_marks_existing_history_deleted(){
  let f=Fixture::new();for i in 0..4{f.time(i*DAY+60);assert!(f.tick().unwrap().unwrap().exists());}assert_eq!(f.count(),3);let h=backup::history(&f.app.db).unwrap();assert_eq!(h.as_array().unwrap().len(),4);assert_eq!(h.as_array().unwrap().iter().filter(|v|v["deleted"]==true).count(),1);assert!(f.app.backup_schedule.snapshot()["last_error"].is_null());
 }
 #[test]fn failed_run_does_not_prune_old_backups(){let f=Fixture::new();let library=library_id(&f.app).unwrap();for i in 0..5{sample(&f,&format!("Galroon-backup-{i}"),i,Origin::Scheduled,&library);}f.space.store(0,Ordering::SeqCst);f.time(60);assert!(f.tick().is_err());assert_eq!(f.count(),5);}
 #[test]fn config_validates_before_save_and_rejects_data_roots_and_links(){
  let f=Fixture::new();let path=f.app.device_dir.join("backup-schedule.json");let before=fs::read(&path).unwrap();for change in 0..7{let mut v=f.config();match change{0=>v.keep=0,1=>v.keep=101,2=>v.minute_utc=1440,3=>v.frequency=Frequency::Weekly,4=>v.weekday=Some(7),5=>v.destination=f.app.state_dir.to_string_lossy().into(),_=>v.destination=f.target.join("missing").to_string_lossy().into()}assert!(f.app.backup_schedule.configure(&f.app,v).is_err());assert_eq!(fs::read(&path).unwrap(),before);}
  f.app.db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'test')",[f.target.to_string_lossy().as_ref()]).unwrap();assert!(f.app.backup_schedule.configure(&f.app,f.config()).unwrap_err().contains("library roots"));f.app.db.lock().unwrap().execute("DELETE FROM roots",[]).unwrap();
  #[cfg(unix)]{let link=f.target.join("linked");std::os::unix::fs::symlink(&f.app.state_dir,&link).unwrap();assert!(destination(&f.app,&link).is_err());}
 }
 #[test]fn corrupt_state_defaults_and_atomic_save_refuses_symlinks(){
  let f=Fixture::new();let path=f.app.device_dir.join("backup-schedule.json");fs::write(&path,b"broken").unwrap();let loaded=Schedule::load(&f.app.device_dir);assert_eq!(loaded.snapshot()["enabled"],false);assert_eq!(loaded.snapshot()["mode"],"full");loaded.configure(&f.app,f.config()).unwrap();assert!(load(&path).unwrap().enabled);assert!(!fs::read_dir(&f.app.device_dir).unwrap().any(|e|e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
  #[cfg(unix)]{let target=f.target.join("do-not-change");fs::write(&target,b"untouched").unwrap();fs::remove_file(&path).unwrap();std::os::unix::fs::symlink(&target,&path).unwrap();assert!(!Schedule::load(&f.app.device_dir).state.lock().unwrap().stored.enabled);assert!(loaded.configure(&f.app,f.config()).is_err());assert_eq!(fs::read(target).unwrap(),b"untouched");}
 }
 #[tokio::test]async fn stop_interrupts_minute_wait_immediately(){let f=Fixture::new();let worker=start(f.app.clone());tokio::task::yield_now().await;worker.request_stop();tokio::time::timeout(Duration::from_secs(1),async{while !worker.task.is_finished(){tokio::task::yield_now().await;}}).await.unwrap();assert_eq!(f.count(),0);}
 #[test]fn manual_and_scheduled_exports_share_one_gate(){let f=Fixture::new();let _guard=f.app.backup_schedule.gate.try_lock().unwrap();assert!(f.app.backup_schedule.export(&f.app,&f.target,Mode::Portable).unwrap_err().contains("already running"));assert!(f.app.backup_schedule.run(&f.app,Some("owner")).unwrap_err().contains("already running"));f.time(60);assert!(f.tick().unwrap().is_none());assert_eq!(f.count(),0);drop(_guard);assert!(f.tick().unwrap().is_some());}
 fn credentials(a:&App,role:&str)->(&'static str,String){
  use sha2::{Digest,Sha256};let token=format!("{}{}",db::id(),db::id());let hash=hex::encode(Sha256::digest(token.as_bytes()));a.db.lock().unwrap().execute("INSERT INTO sessions(token_hash,role,created,id,expires) VALUES(?1,?2,0,?3,?4)",rusqlite::params![hash,role,db::id(),db::now()+3600]).unwrap();if role=="desktop"{("Authorization",format!("Bearer {token}"))}else{let hash=hex::encode(Sha256::digest(a.state_dir.to_string_lossy().as_bytes()));("Cookie",format!("galroon_sid_{}={token}",&hash[..16]))}
 }
 async fn request(a:&App,auth:(&str,&str),method:&str,path:&str,body:Value)->(u16,Value){let req=Request::builder().method(method).uri(path).header("Host","localhost").header("Origin","http://localhost").header("Content-Type","application/json").header(auth.0,auth.1).body(Body::from(body.to_string())).unwrap();let response=crate::router(a.clone()).oneshot(req).await.unwrap();let status=response.status().as_u16();let bytes=to_bytes(response.into_body(),usize::MAX).await.unwrap();(status,serde_json::from_slice(&bytes).unwrap_or_else(|_|json!(String::from_utf8_lossy(&bytes))))}
 #[tokio::test]async fn web_denied_every_new_route_and_full_is_owner_only(){
  let f=Fixture::new();let (key,token)=credentials(&f.app,"web");let auth=(key,token.as_str());for (method,path) in [("GET","/api/backups/schedule"),("POST","/api/backups/schedule"),("POST","/api/backups/run"),("POST","/api/backups"),("DELETE","/api/backups/unknown")]{let(status,body)=request(&f.app,auth,method,path,json!({})).await;assert_eq!(status,403,"{method} {path}: {body}");}
  for role in ["desktop","web_owner"]{let (key,token)=credentials(&f.app,role);let auth=(key,token.as_str());let mut config=serde_json::to_value(f.config()).unwrap();config["mode"]=json!("full");for (path,body) in [("/api/backups/schedule",config),("/api/backups",json!({"destination":f.target,"mode":"full"}))]{let(status,body)=request(&f.app,auth,"POST",path,body).await;assert_eq!(status,403,"{role}: {body}");}
   f.app.backup_schedule.state.lock().unwrap().stored.mode=Mode::Full;assert_eq!(request(&f.app,auth,"POST","/api/backups/run",json!({})).await.0,403);f.app.backup_schedule.state.lock().unwrap().stored.mode=Mode::Portable;
   assert_eq!(request(&f.app,auth,"GET","/api/backups/schedule",json!({})).await.0,200);assert_eq!(request(&f.app,auth,"POST","/api/backups/schedule",serde_json::to_value(f.config()).unwrap()).await.0,200);let(status,body)=request(&f.app,auth,"POST","/api/backups/run",json!({})).await;assert_eq!(status,200,"{role}: {body}");assert_eq!(body["deferred"],false);
   let(status,body)=request(&f.app,auth,"POST","/api/backups",json!({"destination":f.target})).await;assert_eq!(status,200,"{role}: {body}");let h=backup::history(&f.app.db).unwrap();let id=h.as_array().unwrap().iter().find(|v|v["path"]==body["path"]).unwrap()["id"].as_str().unwrap();assert_eq!(request(&f.app,auth,"DELETE",&format!("/api/backups/{id}"),json!({})).await.0,200);
  }
 }
 #[tokio::test]async fn api_config_rejects_unknown_fields_and_state_injection_owner_runs_full(){
  let f=Fixture::new();let bearer=format!("Bearer {}",f.app.token);let auth=("Authorization",bearer.as_str());let config=serde_json::to_value(f.config()).unwrap();for field in ["unexpected","last_success","last_started","version","running"]{let mut v=config.clone();v[field]=json!(1);assert_eq!(request(&f.app,auth,"POST","/api/backups/schedule",v).await.0,422);}
  let mut v=config.clone();v["destination"]=json!(f.app.state_dir);assert_eq!(request(&f.app,auth,"POST","/api/backups/schedule",v).await.0,400);let mut v=config;v["mode"]=json!("full");assert_eq!(request(&f.app,auth,"POST","/api/backups/schedule",v).await.0,200);let(status,body)=request(&f.app,auth,"POST","/api/backups/run",json!({})).await;assert_eq!(status,200,"{body}");assert_eq!(backup::inspect(Path::new(body["path"].as_str().unwrap())).unwrap().mode,Mode::Full);
 }
 #[test]fn manual_delete_marks_history_and_refuses_scheduled_foreign_or_replaced_folders(){
  let f=Fixture::new();let path=f.app.backup_schedule.export(&f.app,&f.target,Mode::Portable).unwrap();let h=backup::history(&f.app.db).unwrap();let id=h[0]["id"].as_str().unwrap();let manifest=fs::read(path.join("manifest.json")).unwrap();let mut m:Value=serde_json::from_slice(&manifest).unwrap();m["library"]["id"]=json!("foreign");fs::write(path.join("manifest.json"),m.to_string()).unwrap();assert!(delete_manual(&f.app,id).is_err());assert!(path.exists());fs::write(path.join("manifest.json"),&manifest).unwrap();
  f.app.db.lock().unwrap().execute("INSERT INTO roots(id,path,label) VALUES('r',?1,'test')",[path.to_string_lossy().as_ref()]).unwrap();assert!(delete_manual(&f.app,id).is_err());f.app.db.lock().unwrap().execute("DELETE FROM roots",[]).unwrap();
  #[cfg(unix)]{let moved=f.target.join("moved");fs::rename(&path,&moved).unwrap();std::os::unix::fs::symlink(&moved,&path).unwrap();assert!(delete_manual(&f.app,id).is_err());assert!(moved.exists());fs::remove_file(&path).unwrap();fs::rename(moved,&path).unwrap();}
  delete_manual(&f.app,id).unwrap();assert!(!path.exists());assert_eq!(backup::history(&f.app.db).unwrap()[0]["deleted"],true);delete_manual(&f.app,id).unwrap();f.time(60);f.tick().unwrap();let h=backup::history(&f.app.db).unwrap();let scheduled=h.as_array().unwrap().iter().find(|v|v["origin"]=="scheduled").unwrap();assert!(delete_manual(&f.app,scheduled["id"].as_str().unwrap()).is_err());assert!(Path::new(scheduled["path"].as_str().unwrap()).exists());
 }
}

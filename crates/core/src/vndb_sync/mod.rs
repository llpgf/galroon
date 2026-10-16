//! Two-way play status and personal rating sync. Local file availability is independent of collection state.
mod credentials;
mod remote;
pub(crate) mod store;
#[cfg(test)] mod tests;
pub use store::migrate;
use axum::{extract::State,http::HeaderMap,Json};
use rusqlite::{params,OptionalExtension};
use serde::Deserialize;
use serde_json::{Value,json};
use crate::{App,ApiError};
use store::{Account,Row};

#[derive(Debug)] pub struct Error(pub String);
impl From<rusqlite::Error> for Error {fn from(e:rusqlite::Error)->Self{Self(e.to_string())}}
impl From<serde_json::Error> for Error {fn from(e:serde_json::Error)->Self{Self(e.to_string())}}
impl From<Error> for ApiError {fn from(e:Error)->Self{Self(e.0)}}
type Result<T>=std::result::Result<T,Error>;
fn writable(a:&App,h:&HeaderMap)->std::result::Result<(),ApiError>{if crate::access::authenticate(a,h)?.role=="web"{return Err(ApiError("Desktop access required".into()));}Ok(())}
fn connected(a:&App)->Result<Account>{store::account(&a.db.lock().unwrap())?.ok_or_else(||Error("Connect a VNDB account first".into()))}
fn client(a:&App,account:&Account)->Result<remote::Client>{remote::Client::new(credentials::read(&a.device_dir,&account.credential)?)}
fn state(a:&App)->Result<Value> {
    let c=a.db.lock().unwrap();let account=store::account(&c)?;
    let rows=store::rows(&c,account.as_ref().map_or("full",|a|a.mode.as_str()))?;
    Ok(json!({"account":account.as_ref().map(|a|json!({"user":a.user,"mode":a.mode,"automatic":a.automatic,"initialized":a.initialized,"last_sync":a.last_sync,"error":a.error})),"preview":account.as_ref().map(|a|&a.preview),"rows":rows,"results":account.as_ref().map(|a|&a.results),"running":a.vndb_sync_gate.try_lock().is_err()}))
}
pub async fn get(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{writable(&a,&h)?;Ok(Json(state(&a)?))}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Connect{token:String}
pub async fn connect(State(a):State<App>,h:HeaderMap,Json(input):Json<Connect>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.vndb_sync_gate.try_lock().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    let token=input.token.trim();if token.is_empty()||token.len()>4096{return Err(ApiError("Enter a valid VNDB personal token".into()));}
    let user=remote::Client::new(token.into())?.me().await?;
    let old=store::account(&a.db.lock().unwrap())?;
    if old.as_ref().is_some_and(|old|old.user.id!=user.id){return Err(ApiError("Disconnect the current VNDB account before switching accounts".into()));}
    let credential=crate::db::id();credentials::write(&a.device_dir,&credential,token)?;
    let saved=(||->Result<()>{
        let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
        let account=if let Some(old)=&old{Account{user,credential:credential.clone(),error:String::new(),..old.clone()}}else{
            tx.execute("UPDATE vndb_sync_links SET base_status=NULL,base_rating=NULL",[])?;
            tx.execute("DELETE FROM vndb_sync_remote",[])?;tx.execute("DELETE FROM vndb_sync_ignored",[])?;
            Account{user,credential:credential.clone(),mode:"full".into(),automatic:false,initialized:false,last_sync:None,preview:String::new(),error:String::new(),results:vec![]}
        };store::save_account(&tx,&account)?;tx.commit()?;Ok(())
    })();
    if let Err(error)=saved{let _=credentials::delete(&a.device_dir,&credential);return Err(error.into());}
    if let Some(old)=old{credentials::delete(&a.device_dir,&old.credential)?;}
    Ok(Json(state(&a)?))
}
pub async fn disconnect(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.vndb_sync_gate.try_lock().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    if let Some(account)=store::account(&a.db.lock().unwrap())?{credentials::delete(&a.device_dir,&account.credential)?;}
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    tx.execute("DELETE FROM settings WHERE key='vndb_sync.account'",[])?;tx.execute("DELETE FROM vndb_sync_remote",[])?;tx.execute("DELETE FROM vndb_sync_ignored",[])?;tx.execute("UPDATE vndb_sync_links SET base_status=NULL,base_rating=NULL",[])?;tx.commit()?;drop(c);
    Ok(Json(state(&a)?))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Settings{mode:String,automatic:bool}
pub async fn settings(State(a):State<App>,h:HeaderMap,Json(input):Json<Settings>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.vndb_sync_gate.try_lock().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    if !["full","existing"].contains(&input.mode.as_str()){return Err(ApiError("Invalid VNDB import mode".into()));}
    let mut account=connected(&a)?;if input.automatic&&!account.initialized{return Err(ApiError("Review the first sync before enabling automatic sync".into()));}
    account.mode=input.mode;account.automatic=input.automatic;account.preview.clear();store::save_account(&a.db.lock().unwrap(),&account)?;Ok(Json(state(&a)?))
}
async fn fetch(a:&App,account:&mut Account,client:&remote::Client)->Result<()> {
    let rows=client.collections(&account.user.id).await?;
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;store::replace_remote(&tx,&rows)?;
    account.preview=crate::db::id();account.error.clear();store::save_account(&tx,account)?;tx.commit()?;Ok(())
}
pub async fn preview(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
    writable(&a,&h)?;let gate=a.vndb_sync_gate.clone().try_lock_owned().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    let mut account=connected(&a)?;let client=client(&a,&account)?;account.preview.clear();account.results.clear();account.error.clear();store::save_account(&a.db.lock().unwrap(),&account)?;
    let response=state(&a)?;
    background(a,account,client,gate,None);
    Ok(Json(response))
}
#[derive(Clone,Deserialize)]#[serde(deny_unknown_fields)]pub struct Choice{subject_id:i64,field:String,fingerprint:String,action:String,work_id:Option<String>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Apply{preview:String,choices:Vec<Choice>}
async fn apply_choice(a:&App,account:&Account,client:&remote::Client,choice:&Choice)->Result<()> {
    let row=store::current(&a.db.lock().unwrap(),&account.mode,choice.subject_id,&choice.field,&choice.fingerprint)?;
    if !["local","remote","import","skip","match","same"].contains(&choice.action.as_str()){return Err(Error("Invalid VNDB sync action".into()));}
    if row.kind=="hidden"{return Err(Error("Restore this hidden card before syncing".into()));}
    if choice.action=="skip"{
        if row.work_id.is_some(){return Err(Error("Only unlinked subjects can be ignored".into()));}
        a.db.lock().unwrap().execute("INSERT OR IGNORE INTO vndb_sync_ignored VALUES(?1)",[choice.subject_id])?;return Ok(());
    }
    // Check the selected remote field again before applying a reviewed choice. VNDB has no CAS API.
    let collection=client.collection(&account.user.id,row.subject_id).await?;
    let actual=collection.as_ref().map(|c|c.value(&row.field));
    if actual!=row.remote || (row.field=="entry" && collection.as_ref().map(|c|c.vote.unwrap_or(0))!=row.remote_rating){return Err(Error("VNDB value changed after preview. Preview again.".into()));}
    if choice.action=="local"{
        let status=row.local.ok_or_else(||Error("Link a local work before uploading".into()))?;
        if row.field=="status"&&status==0&&actual.is_none(){return Err(Error("No VNDB status to clear".into()));}
        let subject=if actual.is_none(){Some(client.subject(row.subject_id).await?)}else{None};
        store::current(&a.db.lock().unwrap(),&account.mode,choice.subject_id,&choice.field,&choice.fingerprint)?;
        client.set_value(row.subject_id,&row.field,status).await?;
        // A local edit during HTTP remains pending against the status actually uploaded.
        let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
        if let Some(subject)=subject{tx.execute("INSERT INTO vndb_sync_remote(subject_id,status,rating,subject) VALUES(?1,0,0,?2) ON CONFLICT(subject_id) DO NOTHING",params![row.subject_id,serde_json::to_string(&subject)?])?;}
        store::baseline(&tx,row.subject_id,&row.field,status)?;tx.commit()?;return Ok(());
    }
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    let row=store::current(&tx,&account.mode,choice.subject_id,&choice.field,&choice.fingerprint)?;
    match choice.action.as_str(){
        "import"=>{if row.work_id.is_some(){return Err(Error("This subject already has a local card".into()));}store::import(&tx,row.subject_id)?;},
        "match"=>{
            if row.work_id.is_some(){return Err(Error("This subject already has a local card".into()));}
            let id=choice.work_id.as_ref().ok_or_else(||Error("Choose a local work".into()))?;
            if !row.candidates.iter().any(|candidate|&candidate.id==id){return Err(Error("Choose a matching candidate or link from the work page".into()));}
            tx.execute("INSERT INTO vndb_sync_links(work_id,subject_id) VALUES(?1,?2)",params![id,row.subject_id])?;
            tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[id])?;
            tx.execute("DELETE FROM vndb_sync_ignored WHERE subject_id=?1",[row.subject_id])?;
            // Matching establishes identity only; differing statuses remain an explicit initial conflict.
        },
        "remote"=>{store::pull(&tx,&row,row.remote.ok_or_else(||Error("VNDB has no collection status for this work".into()))?)?;},
        "same"=>{let remote=if row.field=="status"{Some(row.remote.unwrap_or(0))}else{row.remote};if row.local!=remote||row.local.is_none(){return Err(Error("Values are no longer equal".into()));}store::baseline(&tx,row.subject_id,&row.field,row.local.unwrap())?;},
        _=>return Err(Error("Invalid VNDB sync action".into())),
    }
    tx.commit()?;Ok(())
}
async fn apply_choices(a:&App,account:&mut Account,client:&remote::Client,choices:&[Choice])->Result<Vec<Value>> {
    let mut results=Vec::new();
    for choice in choices {
        match apply_choice(a,account,client,choice).await{
            Ok(())=>results.push(json!({"subject_id":choice.subject_id,"field":choice.field,"ok":true})),
            Err(error)=>{results.push(json!({"subject_id":choice.subject_id,"field":choice.field,"ok":false,"error":error.0}));break;}
        }
        #[cfg(not(test))]
        tokio::time::sleep(std::time::Duration::from_millis(4800)).await;
    }

    let error=results.iter().find_map(|r|r["error"].as_str());account.error=error.unwrap_or("").into();
    if error.is_none(){account.initialized=true;account.last_sync=Some(crate::db::now());}
    account.results=results.clone();store::save_account(&a.db.lock().unwrap(),account)?;Ok(results)
}
pub async fn apply(State(a):State<App>,h:HeaderMap,Json(input):Json<Apply>)->crate::Result<Value>{
    writable(&a,&h)?;let gate=a.vndb_sync_gate.clone().try_lock_owned().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    let mut account=connected(&a)?;
    if input.preview.is_empty()||input.preview!=account.preview{return Err(ApiError("VNDB preview expired. Preview again.".into()));}
    let client=client(&a,&account)?;account.results.clear();account.error.clear();store::save_account(&a.db.lock().unwrap(),&account)?;
    let response=state(&a)?;
    background(a,account,client,gate,Some(input.choices));
    Ok(Json(response))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Link{work_id:String,revision:i64,subject:String}
fn subject_id(value:&str)->Result<i64>{
    let value=value.trim();let id=if value.starts_with("https://") {let url=reqwest::Url::parse(value).map_err(|_|Error("Enter a VNDB URL or v ID".into()))?;if url.host_str()!=Some("vndb.org"){return Err(Error("Enter a VNDB visual novel URL".into()));}url.path().trim_matches('/').to_owned()}else{value.to_owned()};
    id.strip_prefix('v').and_then(|v|v.parse::<i64>().ok()).filter(|v|*v>0).ok_or_else(||Error("Enter a VNDB visual novel ID such as v17".into()))
}
pub async fn link(State(a):State<App>,h:HeaderMap,Json(input):Json<Link>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.vndb_sync_gate.try_lock().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    let account=connected(&a)?;
    let subject=if input.subject.trim().is_empty(){None}else{Some(client(&a,&account)?.subject(subject_id(&input.subject)?).await?)};
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    let revision:i64=tx.query_row("SELECT revision FROM works WHERE id=?1 AND merged_into IS NULL",[&input.work_id],|r|r.get(0))?;
    if revision!=input.revision{return Err(ApiError("This work changed. Reload before editing.".into()));}
    if let Some(subject)=&subject {
        let subject_id=subject.number()?;
        let vn:Option<String>=tx.query_row("SELECT vndb_id FROM works WHERE id=?1",[&input.work_id],|r|r.get(0))?;
        if vn.is_some_and(|v|!v.is_empty()&&v!=subject.id){return Err(ApiError("This card belongs to a different VNDB work".into()));}
        let other:Option<String>=tx.query_row("SELECT work_id FROM vndb_sync_links WHERE subject_id=?1",[subject_id],|r|r.get(0)).optional()?;
        if other.as_ref().is_some_and(|id|id!=&input.work_id){return Err(ApiError("This VNDB subject already has a card. Use Group works to attach the local resources to that card.".into()));}
        tx.execute("INSERT INTO vndb_sync_links(work_id,subject_id) VALUES(?1,?2) ON CONFLICT(work_id) DO UPDATE SET subject_id=excluded.subject_id,base_status=NULL,base_rating=NULL,hidden=0",params![input.work_id,subject_id])?;
        tx.execute("DELETE FROM vndb_sync_ignored WHERE subject_id=?1",[subject_id])?;
    }else{
        // An explicit unlink suppresses reimport of this subject until the user chooses otherwise.
        tx.execute("INSERT OR IGNORE INTO vndb_sync_ignored SELECT subject_id FROM vndb_sync_links WHERE work_id=?1",[&input.work_id])?;
        tx.execute("DELETE FROM vndb_sync_links WHERE work_id=?1",[&input.work_id])?;
    }
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[&input.work_id])?;tx.commit()?;drop(c);Ok(Json(state(&a)?))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Hide{work_id:String,hidden:bool}
pub async fn hide(State(a):State<App>,h:HeaderMap,Json(input):Json<Hide>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.vndb_sync_gate.try_lock().map_err(|_|ApiError("VNDB sync is busy. Retry in a moment.".into()))?;
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    if input.hidden&&tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_bindings b JOIN resource_files rf ON rf.resource_id=b.resource_id WHERE b.work_id=?1)",[&input.work_id],|r|r.get::<_,bool>(0))?{return Err(ApiError("Only cards without local resources can be hidden here".into()));}
    if tx.execute("UPDATE vndb_sync_links SET hidden=?2 WHERE work_id=?1",params![input.work_id,input.hidden])?!=1{return Err(ApiError("Work is not linked to VNDB".into()));}
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[&input.work_id])?;tx.commit()?;drop(c);Ok(Json(state(&a)?))
}

pub struct Worker{task:tokio::task::JoinHandle<()>,manual:std::sync::Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>>}
impl Worker{pub fn request_stop(&self){self.task.abort();if let Some(task)=self.manual.lock().unwrap().take(){task.abort();}}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();}}
fn automatic_choices(rows:Vec<Row>)->Vec<Choice>{rows.into_iter().filter_map(|row|{
    let action=match row.kind.as_str(){"import"=>"import","pull"=>"remote","push"=>"local","same" if row.base!=row.local=>"same",_=>return None};
    Some(Choice{subject_id:row.subject_id,field:row.field,fingerprint:row.fingerprint,action:action.into(),work_id:None})
}).collect()}
async fn sync(a:&App)->Result<()> {
    let account=connected(a)?;let client=client(a,&account)?;sync_client(a,&client).await
}
async fn sync_client(a:&App,client:&remote::Client)->Result<()> {
    let mut account=connected(a)?;fetch(a,&mut account,client).await?;
    let rows=store::rows(&a.db.lock().unwrap(),&account.mode)?;
    apply_choices(a,&mut account,&client,&automatic_choices(rows)).await?;Ok(())
}
pub fn start(a:App)->Worker{
    let manual=a.vndb_sync_task.clone();
    let task=tokio::spawn(async move{
        let mut next=0;
        loop{
            if let Ok(_gate)=a.vndb_sync_gate.try_lock(){
                let account=store::account(&a.db.lock().unwrap()).ok().flatten();
                if let Some(account)=account.filter(|a|a.automatic&&a.initialized){
                    let dirty=store::rows(&a.db.lock().unwrap(),&account.mode).unwrap_or_default().iter().any(|r|r.kind=="push");
                    if crate::db::now()>=next||(dirty&&account.error.is_empty()) {
                        if let Err(error)=sync(&a).await{if let Ok(mut current)=connected(&a){current.error=error.0;let _=store::save_account(&a.db.lock().unwrap(),&current);}}
                        next=crate::db::now()+300;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });Worker{task,manual}
}

#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Rating{work_id:String,revision:i64,rating:Option<i64>}
pub async fn rating(State(a):State<App>,h:HeaderMap,Json(input):Json<Rating>)->crate::Result<Value>{
 writable(&a,&h)?;remote::patch("rating",input.rating.unwrap_or(0))?;
 if input.rating==Some(0){return Err(ApiError("Use null to clear a rating".into()));}
 let c=a.db.lock().unwrap();if c.execute("UPDATE works SET personal_rating=?3,revision=revision+1 WHERE id=?1 AND revision=?2 AND merged_into IS NULL",params![input.work_id,input.revision,input.rating])?!=1{return Err(ApiError("This work changed. Reload before editing.".into()));}Ok(Json(json!({"ok":true})))
}

// Requests return immediately; the owned gate keeps account mutations serialized until completion.
fn background(a:App,mut account:Account,client:remote::Client,gate:tokio::sync::OwnedMutexGuard<()>,choices:Option<Vec<Choice>>){
 let manual=a.vndb_sync_task.clone();let task=tokio::spawn(async move{let _gate=gate;let result=match choices{Some(choices)=>apply_choices(&a,&mut account,&client,&choices).await.map(|_|()),None=>fetch(&a,&mut account,&client).await};if let Err(error)=result{account.error=error.0;let _=store::save_account(&a.db.lock().unwrap(),&account);}});
 *manual.lock().unwrap()=Some(task.abort_handle());
}

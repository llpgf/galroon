//! Two-way play status sync. Local file availability is independent of collection state.
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
fn connected(a:&App)->Result<Account>{store::account(&a.db.lock().unwrap())?.ok_or_else(||Error("Connect a Bangumi account first".into()))}
fn client(a:&App,account:&Account)->Result<remote::Client>{remote::Client::new(credentials::read(&a.device_dir,&account.credential)?)}
fn state(a:&App)->Result<Value> {
    let c=a.db.lock().unwrap();let account=store::account(&c)?;
    let rows=store::rows(&c,account.as_ref().map_or("full",|a|a.mode.as_str()))?;
    Ok(json!({"account":account.as_ref().map(|a|json!({"user":a.user,"mode":a.mode,"automatic":a.automatic,"initialized":a.initialized,"last_sync":a.last_sync,"error":a.error})),"preview":account.as_ref().map(|a|&a.preview),"rows":rows}))
}
pub async fn get(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{writable(&a,&h)?;Ok(Json(state(&a)?))}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Connect{token:String}
pub async fn connect(State(a):State<App>,h:HeaderMap,Json(input):Json<Connect>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    let token=input.token.trim();if token.is_empty()||token.len()>4096{return Err(ApiError("Enter a valid Bangumi personal token".into()));}
    let user=remote::Client::new(token.into())?.me().await?;
    let old=store::account(&a.db.lock().unwrap())?;
    if old.as_ref().is_some_and(|old|old.user.id!=user.id){return Err(ApiError("Disconnect the current Bangumi account before switching accounts".into()));}
    let credential=crate::db::id();credentials::write(&a.device_dir,&credential,token)?;
    let saved=(||->Result<()>{
        let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
        let account=if let Some(old)=&old{Account{user,credential:credential.clone(),error:String::new(),..old.clone()}}else{
            tx.execute("UPDATE bangumi_links SET base_status=NULL",[])?;
            tx.execute("DELETE FROM bangumi_remote",[])?;tx.execute("DELETE FROM bangumi_ignored",[])?;
            Account{user,credential:credential.clone(),mode:"full".into(),automatic:false,initialized:false,last_sync:None,preview:String::new(),error:String::new()}
        };store::save_account(&tx,&account)?;tx.commit()?;Ok(())
    })();
    if let Err(error)=saved{let _=credentials::delete(&a.device_dir,&credential);return Err(error.into());}
    if let Some(old)=old{credentials::delete(&a.device_dir,&old.credential)?;}
    Ok(Json(state(&a)?))
}
pub async fn disconnect(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    if let Some(account)=store::account(&a.db.lock().unwrap())?{credentials::delete(&a.device_dir,&account.credential)?;}
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    tx.execute("DELETE FROM settings WHERE key='bangumi.account'",[])?;tx.execute("DELETE FROM bangumi_remote",[])?;tx.execute("DELETE FROM bangumi_ignored",[])?;tx.execute("UPDATE bangumi_links SET base_status=NULL",[])?;tx.commit()?;drop(c);
    Ok(Json(state(&a)?))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Settings{mode:String,automatic:bool}
pub async fn settings(State(a):State<App>,h:HeaderMap,Json(input):Json<Settings>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    if !["full","existing"].contains(&input.mode.as_str()){return Err(ApiError("Invalid Bangumi import mode".into()));}
    let mut account=connected(&a)?;if input.automatic&&!account.initialized{return Err(ApiError("Review the first sync before enabling automatic sync".into()));}
    account.mode=input.mode;account.automatic=input.automatic;account.preview.clear();store::save_account(&a.db.lock().unwrap(),&account)?;Ok(Json(state(&a)?))
}
async fn fetch(a:&App,account:&mut Account,client:&remote::Client)->Result<()> {
    let rows=client.collections(&account.user.username).await?;
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;store::replace_remote(&tx,&rows)?;
    account.preview=crate::db::id();account.error.clear();store::save_account(&tx,account)?;tx.commit()?;Ok(())
}
pub async fn preview(State(a):State<App>,h:HeaderMap)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    let mut account=connected(&a)?;let client=client(&a,&account)?;fetch(&a,&mut account,&client).await?;Ok(Json(state(&a)?))
}
#[derive(Clone,Deserialize)]#[serde(deny_unknown_fields)]pub struct Choice{subject_id:i64,fingerprint:String,action:String,work_id:Option<String>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Apply{preview:String,choices:Vec<Choice>}
async fn apply_choice(a:&App,account:&Account,client:&remote::Client,choice:&Choice)->Result<()> {
    let row=store::current(&a.db.lock().unwrap(),&account.mode,choice.subject_id,&choice.fingerprint)?;
    if !["local","remote","import","skip","match","same"].contains(&choice.action.as_str()){return Err(Error("Invalid Bangumi sync action".into()));}
    if row.kind=="hidden"{return Err(Error("Restore this hidden card before syncing".into()));}
    if choice.action=="skip"{
        if row.work_id.is_some(){return Err(Error("Only unlinked subjects can be ignored".into()));}
        a.db.lock().unwrap().execute("INSERT OR IGNORE INTO bangumi_ignored VALUES(?1)",[choice.subject_id])?;return Ok(());
    }
    // Check the remote status again before applying a reviewed choice. Bangumi has no CAS API.
    let actual=client.status(&account.user.username,row.subject_id).await?;
    if actual!=row.remote{return Err(Error("Bangumi status changed after preview. Preview again.".into()));}
    if choice.action=="local"{
        let status=row.local.ok_or_else(||Error("Link a local work before uploading".into()))?;
        if status==0{return Err(Error("Bangumi cannot clear a collection status".into()));}
        let subject=if actual.is_none(){Some(client.subject(row.subject_id).await?)}else{None};
        store::current(&a.db.lock().unwrap(),&account.mode,choice.subject_id,&choice.fingerprint)?;
        client.set_status(row.subject_id,status,actual.is_some()).await?;
        // A local edit during HTTP remains pending against the status actually uploaded.
        let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
        if let Some(subject)=subject{tx.execute("INSERT INTO bangumi_remote(subject_id,status,subject) VALUES(?1,?2,?3) ON CONFLICT(subject_id) DO UPDATE SET status=excluded.status",params![row.subject_id,status,serde_json::to_string(&subject)?])?;}
        store::baseline(&tx,row.subject_id,status)?;tx.commit()?;return Ok(());
    }
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    let row=store::current(&tx,&account.mode,choice.subject_id,&choice.fingerprint)?;
    match choice.action.as_str(){
        "import"=>{if row.work_id.is_some(){return Err(Error("This subject already has a local card".into()));}store::import(&tx,row.subject_id)?;},
        "match"=>{
            if row.work_id.is_some(){return Err(Error("This subject already has a local card".into()));}
            let id=choice.work_id.as_ref().ok_or_else(||Error("Choose a local work".into()))?;
            if !row.candidates.iter().any(|candidate|&candidate.id==id){return Err(Error("Choose a matching candidate or link from the work page".into()));}
            tx.execute("INSERT INTO bangumi_links(work_id,subject_id) VALUES(?1,?2)",params![id,row.subject_id])?;
            tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[id])?;
            tx.execute("DELETE FROM bangumi_ignored WHERE subject_id=?1",[row.subject_id])?;
            // Matching establishes identity only; differing statuses remain an explicit initial conflict.
        },
        "remote"=>{store::pull(&tx,&row,row.remote.ok_or_else(||Error("Bangumi has no collection status for this work".into()))?)?;},
        "same"=>{if row.local!=Some(row.remote.unwrap_or(0)){return Err(Error("Statuses are no longer equal".into()));}store::baseline(&tx,row.subject_id,row.local.unwrap())?;},
        _=>return Err(Error("Invalid Bangumi sync action".into())),
    }
    tx.commit()?;Ok(())
}
async fn apply_choices(a:&App,account:&mut Account,client:&remote::Client,choices:&[Choice])->Result<Vec<Value>> {
    let mut results=Vec::new();
    for choice in choices {
        match apply_choice(a,account,client,choice).await{
            Ok(())=>results.push(json!({"subject_id":choice.subject_id,"ok":true})),
            Err(error)=>{results.push(json!({"subject_id":choice.subject_id,"ok":false,"error":error.0}));break;}
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let error=results.iter().find_map(|r|r["error"].as_str());account.error=error.unwrap_or("").into();
    if error.is_none(){account.initialized=true;account.last_sync=Some(crate::db::now());}
    store::save_account(&a.db.lock().unwrap(),account)?;Ok(results)
}
pub async fn apply(State(a):State<App>,h:HeaderMap,Json(input):Json<Apply>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    let mut account=connected(&a)?;
    if input.preview.is_empty()||input.preview!=account.preview{return Err(ApiError("Bangumi preview expired. Preview again.".into()));}
    let client=client(&a,&account)?;let results=apply_choices(&a,&mut account,&client,&input.choices).await?;
    let mut state=state(&a)?;state["results"]=json!(results);Ok(Json(state))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Link{work_id:String,revision:i64,subject:String}
fn subject_id(value:&str)->Result<i64>{
    let value=value.trim();let number=if value.bytes().all(|v|v.is_ascii_digit()){value.to_owned()}else{
        let url=reqwest::Url::parse(value).map_err(|_|Error("Enter a Bangumi subject URL or numeric ID".into()))?;
        if url.scheme()!="https"||!matches!(url.host_str(),Some("bgm.tv"|"bangumi.tv"|"chii.in")){return Err(Error("Enter a Bangumi subject URL".into()));}
        url.path().trim_end_matches('/').strip_prefix("/subject/").ok_or_else(||Error("Enter a Bangumi subject URL".into()))?.to_owned()
    };number.parse::<i64>().ok().filter(|id|*id>0).ok_or_else(||Error("Invalid Bangumi subject ID".into()))
}
pub async fn link(State(a):State<App>,h:HeaderMap,Json(input):Json<Link>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    let account=connected(&a)?;
    let subject=if input.subject.trim().is_empty(){None}else{Some(client(&a,&account)?.subject(subject_id(&input.subject)?).await?)};
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    let revision:i64=tx.query_row("SELECT revision FROM works WHERE id=?1 AND merged_into IS NULL",[&input.work_id],|r|r.get(0))?;
    if revision!=input.revision{return Err(ApiError("This work changed. Reload before editing.".into()));}
    if let Some(subject)=&subject {
        let other:Option<String>=tx.query_row("SELECT work_id FROM bangumi_links WHERE subject_id=?1",[subject.id],|r|r.get(0)).optional()?;
        if other.as_ref().is_some_and(|id|id!=&input.work_id){return Err(ApiError("This Bangumi subject already has a card. Use Group works to attach the local resources to that card.".into()));}
        tx.execute("INSERT INTO bangumi_links(work_id,subject_id) VALUES(?1,?2) ON CONFLICT(work_id) DO UPDATE SET subject_id=excluded.subject_id,base_status=NULL,hidden=0",params![input.work_id,subject.id])?;
        tx.execute("DELETE FROM bangumi_ignored WHERE subject_id=?1",[subject.id])?;
    }else{
        // An explicit unlink suppresses reimport of this subject until the user chooses otherwise.
        tx.execute("INSERT OR IGNORE INTO bangumi_ignored SELECT subject_id FROM bangumi_links WHERE work_id=?1",[&input.work_id])?;
        tx.execute("DELETE FROM bangumi_links WHERE work_id=?1",[&input.work_id])?;
    }
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[&input.work_id])?;tx.commit()?;drop(c);Ok(Json(state(&a)?))
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Hide{work_id:String,hidden:bool}
pub async fn hide(State(a):State<App>,h:HeaderMap,Json(input):Json<Hide>)->crate::Result<Value>{
    writable(&a,&h)?;let _gate=a.bangumi_gate.try_lock().map_err(|_|ApiError("Bangumi sync is busy. Retry in a moment.".into()))?;
    let mut c=a.db.lock().unwrap();let tx=c.transaction()?;
    if input.hidden&&tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_bindings b JOIN resource_files rf ON rf.resource_id=b.resource_id WHERE b.work_id=?1)",[&input.work_id],|r|r.get::<_,bool>(0))?{return Err(ApiError("Only cards without local resources can be hidden here".into()));}
    if tx.execute("UPDATE bangumi_links SET hidden=?2 WHERE work_id=?1",params![input.work_id,input.hidden])?!=1{return Err(ApiError("Work is not linked to Bangumi".into()));}
    tx.execute("UPDATE works SET revision=revision+1 WHERE id=?1",[&input.work_id])?;tx.commit()?;drop(c);Ok(Json(state(&a)?))
}

pub struct Worker{task:tokio::task::JoinHandle<()>}
impl Worker{pub fn request_stop(&self){self.task.abort();}}
impl Drop for Worker{fn drop(&mut self){self.request_stop();}}
fn automatic_choices(rows:Vec<Row>)->Vec<Choice>{rows.into_iter().filter_map(|row|{
    let action=match row.kind.as_str(){"import"=>"import","pull"=>"remote","push"=>"local","same" if row.base!=row.local=>"same",_=>return None};
    Some(Choice{subject_id:row.subject_id,fingerprint:row.fingerprint,action:action.into(),work_id:None})
}).collect()}
async fn sync(a:&App)->Result<()> {
    let account=connected(a)?;let client=client(a,&account)?;sync_client(a,&client).await
}
async fn sync_client(a:&App,client:&remote::Client)->Result<()> {
    let mut account=connected(a)?;fetch(a,&mut account,client).await?;
    let rows=store::rows(&a.db.lock().unwrap(),&account.mode)?;
    apply_choices(a,&mut account,&client,&automatic_choices(rows)).await?;Ok(())
}
fn dirty(c:&rusqlite::Connection)->rusqlite::Result<bool>{c.query_row("SELECT EXISTS(SELECT 1 FROM bangumi_links b JOIN works w ON w.id=b.work_id WHERE b.hidden=0 AND b.base_status IS NOT NULL AND EXISTS(SELECT 1 FROM bangumi_remote r WHERE r.subject_id=b.subject_id AND r.status=b.base_status) AND w.merged_into IS NULL AND w.status!='none' AND w.status!=CASE b.base_status WHEN 0 THEN 'none' WHEN 1 THEN 'backlog' WHEN 2 THEN 'completed' WHEN 3 THEN 'playing' WHEN 4 THEN 'on_hold' WHEN 5 THEN 'dropped' END)",[],|r|r.get::<_,bool>(0))}
pub fn start(a:App)->Worker{
    let task=tokio::spawn(async move{
        let mut next=0;
        loop{
            if let Ok(_gate)=a.bangumi_gate.try_lock(){
                let account=store::account(&a.db.lock().unwrap()).ok().flatten();
                if let Some(account)=account.filter(|a|a.automatic&&a.initialized){
                    let dirty=dirty(&a.db.lock().unwrap()).unwrap_or(false);
                    if crate::db::now()>=next||(dirty&&account.error.is_empty()) {
                        if let Err(error)=sync(&a).await{if let Ok(mut current)=connected(&a){current.error=error.0;let _=store::save_account(&a.db.lock().unwrap(),&current);}}
                        next=crate::db::now()+300;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });Worker{task}
}

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use super::{Error, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Subject {
    pub id: i64,
    pub name: String,
    #[serde(default)] pub name_cn: String,
    #[serde(default)] pub images: Value,
    #[serde(default)] pub date: Option<String>,
    #[serde(default,alias="short_summary")] pub summary: String,
    #[serde(rename="type",default)] pub kind: i64,
}
impl Subject {
    pub fn title(&self) -> &str { if self.name_cn.is_empty() { &self.name } else { &self.name_cn } }
    pub fn cover(&self) -> &str {
        self.images.get("large").or_else(||self.images.get("common")).and_then(Value::as_str).filter(|v|crate::artwork::allowed(v)).unwrap_or("")
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Collection { pub subject_id:i64, pub subject_type:i64, #[serde(rename="type")] pub status:i64, pub subject:Subject }
#[derive(Deserialize)] struct Page { total:usize, data:Vec<Collection> }
#[derive(Clone, Deserialize, Serialize)]
pub struct User { pub id:i64, pub username:String, #[serde(default)] pub nickname:String }

pub struct Client { http:reqwest::Client, token:String, base:String }
impl Client {
    pub fn new(token:String)->Result<Self> { Self::at(token,"https://api.bgm.tv".into()) }
    pub(super) fn at(token:String,base:String)->Result<Self> {
        let http=reqwest::Client::builder().user_agent(concat!("llpgf/Galroon/",env!("CARGO_PKG_VERSION")," (https://github.com/llpgf/galroon)"))
            .redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(30)).build().map_err(|_|Error("Cannot create Bangumi connection".into()))?;
        Ok(Self{http,token,base})
    }
    async fn send(&self,method:reqwest::Method,path:&str,body:Option<Value>)->Result<reqwest::Response> {
        let mut request=self.http.request(method,format!("{}{path}",self.base)).bearer_auth(&self.token);
        if let Some(body)=body { request=request.json(&body); }
        let response=request.send().await.map_err(|_|Error("Bangumi is unavailable. Your local changes are saved; retry sync later.".into()))?;
        match response.status().as_u16() {
            200..=299|404=>Ok(response),
            401|403=>Err(Error("Bangumi authorization failed. Reconnect with a valid personal token.".into())),
            429=>Err(Error("Bangumi rate limit reached. Retry sync later.".into())),
            code=>Err(Error(format!("Bangumi request failed ({code}). Retry sync later."))),
        }
    }
    async fn json<T:serde::de::DeserializeOwned>(response:reqwest::Response)->Result<T> {
        if response.status()==404 { return Err(Error("Bangumi item was not found".into())); }
        response.json().await.map_err(|_|Error("Bangumi returned an invalid response".into()))
    }
    fn user_segment(user:&str)->String {
        let mut url=reqwest::Url::parse("https://api.bgm.tv/").unwrap();url.path_segments_mut().unwrap().push(user);url.path().trim_start_matches('/').to_owned()
    }
    pub async fn me(&self)->Result<User> { Self::json(self.send(reqwest::Method::GET,"/v0/me",None).await?).await }
    pub async fn subject(&self,id:i64)->Result<Subject> {
        let subject:Subject=Self::json(self.send(reqwest::Method::GET,&format!("/v0/subjects/{id}"),None).await?).await?;
        if subject.kind!=4 {return Err(Error("Choose a Bangumi game subject".into()));} Ok(subject)
    }
    pub async fn collections(&self,user:&str)->Result<Vec<Collection>> {
        let user=Self::user_segment(user);
        let mut rows=Vec::new();
        loop {
            let page:Page=Self::json(self.send(reqwest::Method::GET,&format!("/v0/users/{user}/collections?subject_type=4&limit=100&offset={}",rows.len()),None).await?).await?;
            if page.data.is_empty() { if rows.len()<page.total {return Err(Error("Bangumi collection changed during paging. Preview again.".into()));} break; }
            rows.extend(page.data);
            if rows.len()>=page.total {break;}
        }
        for row in &rows { if !(1..=5).contains(&row.status){return Err(Error("Invalid Bangumi collection status".into()));}
            if row.subject_type!=4||row.subject.id!=row.subject_id {return Err(Error("Bangumi returned an invalid game collection".into()));} }
        Ok(rows)
    }
    pub async fn status(&self,user:&str,id:i64)->Result<Option<i64>> {
        let user=Self::user_segment(user);
        let response=self.send(reqwest::Method::GET,&format!("/v0/users/{user}/collections/{id}"),None).await?;
        if response.status()==404 {return Ok(None);}
        #[derive(Deserialize)]struct Status{#[serde(rename="type")]status:i64}
        let value:Status=Self::json(response).await?;if !(1..=5).contains(&value.status){return Err(Error("Invalid Bangumi collection status".into()));}Ok(Some(value.status))
    }
    pub async fn set_status(&self,id:i64,status:i64,exists:bool)->Result<()> {
        if !(1..=5).contains(&status){return Err(Error("Bangumi cannot clear a collection status".into()));}
        // Only status is patched. Existing ratings, comments, tags and visibility are untouched.
        let body=if exists {json!({"type":status})} else {json!({"type":status,"private":true})};
        let method=if exists{reqwest::Method::PATCH}else{reqwest::Method::POST};
        let response=self.send(method,&format!("/v0/users/-/collections/{id}"),Some(body)).await?;
        if !response.status().is_success(){return Err(Error("Bangumi collection changed. Preview again.".into()));}Ok(())
    }
}

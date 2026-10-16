use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use super::{Error,Result};
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Subject {pub id:String,pub title:String,#[serde(default)]pub description:Option<String>,#[serde(default)]pub image:Option<Value>,#[serde(default)]pub released:Option<String>}
impl Subject {pub fn number(&self)->Result<i64>{super::subject_id(&self.id)} pub fn cover(&self)->&str{self.image.as_ref().and_then(|i|i["url"].as_str()).filter(|u|crate::artwork::allowed(u)).unwrap_or("")}}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Label{pub id:i64}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Collection{pub id:String,pub vote:Option<i64>,pub labels:Vec<Label>,pub vn:Subject}
impl Collection {
 pub fn status(&self)->i64 {let active:Vec<_>=self.labels.iter().filter(|l|(1..=4).contains(&l.id)).collect();match active.as_slice(){[label]=>label.id,[] if self.labels.iter().any(|l|l.id==5)=>5,[]=>0,_=>-1}}
 pub fn value(&self,field:&str)->i64{if field=="rating"{self.vote.unwrap_or(0)}else{self.status()}}
}
#[derive(Clone,Deserialize,Serialize)]pub struct User{pub id:String,pub username:String,pub permissions:Vec<String>}
#[derive(Deserialize)]struct Page<T>{results:Vec<T>,more:bool}
pub struct Client{http:reqwest::Client,token:String,base:String}
impl Client {
 pub fn new(token:String)->Result<Self>{Self::at(token,"https://api.vndb.org/kana".into())}
 pub(super) fn at(token:String,base:String)->Result<Self>{Ok(Self{http:reqwest::Client::builder().user_agent(concat!("Galroon/",env!("CARGO_PKG_VERSION"))).redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(30)).build().map_err(|_|Error("Cannot create VNDB connection".into()))?,token,base})}
 async fn send(&self,method:reqwest::Method,path:&str,body:Option<Value>)->Result<reqwest::Response>{
  let mut request=self.http.request(method,format!("{}{path}",self.base)).header("Authorization",format!("Token {}",self.token));if let Some(body)=body{request=request.json(&body);}
  let response=request.send().await.map_err(|_|Error("VNDB is unavailable. Retry sync later.".into()))?;
  match response.status().as_u16(){200..=299=>Ok(response),401|403=>Err(Error("VNDB authorization failed. Reconnect with listread and listwrite permissions.".into())),429=>Err(Error("VNDB rate limit reached. Retry sync later.".into())),code=>Err(Error(format!("VNDB request failed ({code}). Retry sync later.")))}
 }
 async fn json<T:serde::de::DeserializeOwned>(&self,path:&str,body:Option<Value>)->Result<T>{let method=if body.is_some(){reqwest::Method::POST}else{reqwest::Method::GET};self.send(method,path,body).await?.json().await.map_err(|_|Error("VNDB returned an invalid response".into()))}
 pub async fn me(&self)->Result<User>{let user:User=self.json("/authinfo",None).await?;if !["listread","listwrite"].iter().all(|p|user.permissions.iter().any(|v|v==p)){return Err(Error("VNDB token requires listread and listwrite permissions".into()));}Ok(user)}
 pub async fn subject(&self,id:i64)->Result<Subject>{let page:Page<Subject>=self.json("/vn",Some(json!({"filters":["id","=",format!("v{id}")],"fields":"title,description,image.url,released"}))).await?;page.results.into_iter().next().ok_or_else(||Error("VNDB visual novel was not found".into()))}
 async fn page(&self,user:&str,filter:Value)->Result<Page<Collection>>{self.json("/ulist",Some(json!({"user":user,"filters":filter,"fields":"vote,labels.id,vn{id,title,description,image.url,released}","sort":"id","results":100}))).await}
 pub async fn collections(&self,user:&str)->Result<Vec<Collection>>{
  let mut rows=Vec::new();let mut filter=json!([]);
  loop{let page=self.page(user,filter).await?;if page.results.is_empty(){if page.more{return Err(Error("VNDB list changed during pagination".into()));}break;}filter=json!(["id",">",page.results.last().unwrap().id]);rows.extend(page.results);if !page.more{break;}tokio::time::sleep(std::time::Duration::from_millis(1600)).await;}
  Ok(rows)
 }
 pub async fn collection(&self,user:&str,id:i64)->Result<Option<Collection>>{Ok(self.page(user,json!(["id","=",format!("v{id}")])).await?.results.into_iter().next())}
 pub async fn set_value(&self,id:i64,field:&str,value:i64)->Result<()>{
  let body=patch(field,value)?;self.send(reqwest::Method::PATCH,&format!("/ulist/v{id}"),Some(body)).await?;Ok(())
 }
}
pub(super) fn patch(field:&str,value:i64)->Result<Value>{
 match field {
  "rating" if value==0=>Ok(json!({"vote":null})),
  "rating" if (10..=100).contains(&value)=>Ok(json!({"vote":value})),
  "status" if value==0=>Ok(json!({"labels_unset":[1,2,3,4,5]})),
  "status" if (1..=5).contains(&value)=>Ok(json!({"labels_set":[value],"labels_unset":(1..=4).filter(|id|*id!=value).collect::<Vec<_>>()})),
  _=>Err(Error("Invalid VNDB sync value".into()))
 }
}

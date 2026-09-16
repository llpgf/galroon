use crate::db::Db;
use rusqlite::params;
use serde_json::{json,Value};
type R<T>=Result<T,String>;
pub const FIELDS:[&str;8]=["title","original_title","description","cover","developer","released","tags","aliases"];
pub fn snapshot(db:&Db,id:&str)->R<Value>{let c=db.lock().map_err(|e|e.to_string())?;let(s,o,r):(String,String,i64)=c.query_row("SELECT source_json,overrides,revision FROM works WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;Ok(json!({"source":serde_json::from_str::<Value>(&s).map_err(|e|e.to_string())?,"overrides":serde_json::from_str::<Value>(&o).map_err(|e|e.to_string())?,"revision":r}))}
fn validate(field:&str,v:&Value)->R<()> {
    if ["tags","aliases"].contains(&field){if !v.as_array().is_some_and(|a|a.iter().all(Value::is_string)){return Err(format!("{field} must be a list of text values"));}}
    else if !v.is_string(){return Err(format!("{field} must be text"));}
    if ["title","original_title"].contains(&field)&&v.as_str().unwrap_or("").trim().is_empty(){return Err("Titles cannot be empty".into());}Ok(())
}
pub fn apply(db:&Db,id:&str,revision:i64,patch:&Value,reset:&[String],source:Option<Value>)->R<i64>{
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    let(old,raw,over):(i64,String,String)=tx.query_row("SELECT revision,source_json,overrides FROM works WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
    if old!=revision{return Err("This work changed. Reload before editing.".into());}
    let mut baseline:Value=serde_json::from_str(&raw).map_err(|e|e.to_string())?;let mut overrides:Value=serde_json::from_str(&over).map_err(|e|e.to_string())?;
    if let Some(source)=source{for f in FIELDS{if let Some(v)=source.get(f){validate(f,v)?;baseline[f]=v.clone();}}baseline["refreshed_at"]=json!(crate::db::now());}
    for(key,v)in patch.as_object().ok_or("Metadata patch must be an object")?{if !FIELDS.contains(&key.as_str()){return Err(format!("Unknown metadata field: {key}"));}validate(key,v)?;overrides[key]=v.clone();}
    for f in reset{if !FIELDS.contains(&f.as_str()){return Err("Unknown reset field".into());}if baseline.get(f).is_none(){return Err(format!("No source value exists for {f}"));}overrides.as_object_mut().ok_or("Invalid overrides")?.remove(f);}
    for f in FIELDS{if let Some(v)=overrides.get(f).or_else(||baseline.get(f)){validate(f,v)?;let value=if v.is_array(){v.to_string()}else{v.as_str().unwrap().into()};tx.execute(&format!("UPDATE works SET {f}=?2 WHERE id=?1"),params![id,value]).map_err(|e|e.to_string())?;}}
    tx.execute("UPDATE works SET source_json=?2,overrides=?3,revision=revision+1 WHERE id=?1",params![id,baseline.to_string(),overrides.to_string()]).map_err(|e|e.to_string())?;tx.commit().map_err(|e|e.to_string())?;Ok(old+1)
}
pub async fn fetch(id:&str)->R<Value>{
    if !id.strip_prefix('v').is_some_and(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_digit())){return Err("Invalid VNDB work identifier".into());}
    let response=reqwest::Client::new().post("https://api.vndb.org/kana/vn").timeout(std::time::Duration::from_secs(25)).json(&json!({"filters":["id","=",id],"fields":"title,alttitle,aliases,description,image.url,released,developers.name,tags.name","results":1})).send().await.map_err(|e|e.to_string())?;
    if !response.status().is_success(){return Err(format!("VNDB returned {}; try again later",response.status()));}
    let response:Value=response.json().await.map_err(|e|e.to_string())?;let v=response["results"].as_array().and_then(|v|v.first()).ok_or("VNDB work not found")?;
    let title=v["title"].as_str().ok_or("VNDB response has no title")?;
    Ok(json!({"title":title,"original_title":v["alttitle"].as_str().unwrap_or(title),"aliases":v["aliases"].as_array().cloned().unwrap_or_default(),"description":v["description"].as_str().unwrap_or(""),"cover":v["image"]["url"].as_str().unwrap_or(""),"released":v["released"].as_str().unwrap_or(""),"developer":v["developers"].as_array().map(|a|a.iter().filter_map(|x|x["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),"tags":v["tags"].as_array().map(|a|a.iter().filter_map(|x|x["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default()}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn refresh_preserves_override_and_reset_uses_latest_source(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("db")).unwrap();db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,source_json) VALUES('w','First','First',?1)",[json!({"title":"First","original_title":"First"}).to_string()]).unwrap();assert_eq!(apply(&db,"w",1,&json!({"title":"My title"}),&[],None).unwrap(),2);apply(&db,"w",2,&json!({}),&[],Some(json!({"title":"Updated source"}))).unwrap();let title=||db.lock().unwrap().query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap();assert_eq!(title(),"My title");apply(&db,"w",3,&json!({}),&["title".into()],None).unwrap();assert_eq!(title(),"Updated source");assert!(apply(&db,"w",3,&json!({"title":"Stale edit"}),&[],None).is_err());assert_eq!(title(),"Updated source");assert!(apply(&db,"w",4,&json!({"title":""}),&[],None).is_err());assert_eq!(snapshot(&db,"w").unwrap()["revision"],4);}
}

use crate::db::Db;
use rusqlite::{params,OptionalExtension};
use serde_json::{json,Value};
type R<T>=Result<T,String>;
pub const FIELDS:[&str;8]=["title","original_title","description","cover","developer","released","tags","aliases"];
const PROVIDERS:[&str;3]=["vndb","bangumi","dlsite"];
const VALID_PREFS:[&str;5]=["auto","vndb","bangumi","dlsite","manual"];

fn non_empty(v:&Value)->bool{
    match v{
        Value::String(s)=>!s.trim().is_empty(),
        Value::Array(a)=>!a.is_empty(),
        _=>false,
    }
}
fn subject_to_provider(subject:&Value)->Value{
    let name=subject["name"].as_str().unwrap_or("");
    let name_cn=subject["name_cn"].as_str().unwrap_or("");
    let title=if name_cn.is_empty(){name}else{name_cn};
    let cover=subject["images"]["large"].as_str().or_else(||subject["images"]["common"].as_str()).filter(|u|crate::artwork::allowed(u)).unwrap_or("");
    let released=subject["date"].as_str().unwrap_or("");
    let description=subject["summary"].as_str().or_else(||subject["short_summary"].as_str()).unwrap_or("");
    let mut obj=serde_json::Map::new();
    if let Some(id)=subject["id"].as_i64(){obj.insert("subject_id".into(),json!(id));}
    if !title.is_empty(){obj.insert("title".into(),json!(title));}
    if !name.is_empty(){obj.insert("original_title".into(),json!(name));}
    if !cover.is_empty(){obj.insert("cover".into(),json!(cover));}
    if !released.is_empty(){obj.insert("released".into(),json!(released));}
    if !description.is_empty(){obj.insert("description".into(),json!(description));}
    Value::Object(obj)
}
fn dlsite_to_provider(p:&crate::dlsite::Product)->Value{
    let mut obj=serde_json::Map::new();
    obj.insert("code".into(),json!(p.code));
    obj.insert("title".into(),json!(p.title));
    obj.insert("original_title".into(),json!(p.title));
    if let Some(m)=&p.maker{obj.insert("developer".into(),json!(m));}
    if let Some(c)=&p.cover{obj.insert("cover".into(),json!(c));}
    if let Some(r)=&p.released{obj.insert("released".into(),json!(r));}
    if let Some(d)=&p.description{obj.insert("description".into(),json!(d));}
    if !p.tags.is_empty(){obj.insert("tags".into(),json!(p.tags));}
    Value::Object(obj)
}
fn detect_dlsite_code(c:&rusqlite::Connection,id:&str,baseline:&Value,title:&str,orig:&str,desc:&str)->Option<String>{
    if let Some(code)=baseline["_providers"]["dlsite"]["code"].as_str().filter(|s|crate::dlsite::valid_code(s)){
        return Some(code.to_owned());
    }
    for text in [desc,title,orig,baseline["description"].as_str().unwrap_or(""),baseline["title"].as_str().unwrap_or("")]{
        if let Some(code)=crate::dlsite::extract_codes(text).into_iter().next(){return Some(code);}
    }
    if let Ok(mut stmt)=c.prepare("SELECT r.relative_path, r.title FROM resources r LEFT JOIN resource_bindings rb ON rb.resource_id=r.id WHERE r.work_id=?1 OR rb.work_id=?1 LIMIT 20"){
        if let Ok(rows)=stmt.query_map([id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))){
            for (path,rtitle) in rows.flatten(){
                for text in [path.as_str(),rtitle.as_str()]{
                    if let Some(code)=crate::dlsite::extract_codes(text).into_iter().next(){return Some(code);}
                }
            }
        }
    }
    None
}
fn synthesize_providers(c:&rusqlite::Connection,id:&str,vndb_id:Option<&str>,baseline:&Value)->(Value,Option<i64>){
    let mut providers=baseline.get("_providers").and_then(Value::as_object).cloned().unwrap_or_default();
    if !providers.contains_key("vndb")&&vndb_id.is_some(){
        let mut vndb=serde_json::Map::new();
        for f in FIELDS{if let Some(v)=baseline.get(f){vndb.insert(f.into(),v.clone());}}
        if !vndb.is_empty(){providers.insert("vndb".into(),Value::Object(vndb));}
    }
    let bgm_link:Option<i64>=c.query_row("SELECT subject_id FROM bangumi_links WHERE work_id=?1",[id],|r|r.get(0)).optional().ok().flatten();
    if let Some(sid)=bgm_link{
        if !providers.contains_key("bangumi"){
            if let Ok(Some(raw))=c.query_row("SELECT subject FROM bangumi_remote WHERE subject_id=?1",[sid],|r|r.get::<_,String>(0)).optional(){
                if let Ok(subj)=serde_json::from_str::<Value>(&raw){
                    providers.insert("bangumi".into(),subject_to_provider(&subj));
                }
            }
        }
    }
    (Value::Object(providers),bgm_link)
}
pub fn snapshot(db:&Db,id:&str)->R<Value>{
    let c=db.lock().map_err(|e|e.to_string())?;
    let(s,o,r,vndb_id,vndb_release,title,orig,desc):(String,String,i64,Option<String>,Option<String>,String,String,String)=c.query_row(
        "SELECT source_json,overrides,revision,vndb_id,vndb_release,title,original_title,description FROM works WHERE id=?1",
        [id],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)),
    ).map_err(|e|e.to_string())?;
    let baseline:Value=serde_json::from_str(&s).map_err(|e|e.to_string())?;
    let overrides:Value=serde_json::from_str(&o).map_err(|e|e.to_string())?;
    let (providers,bangumi_id)=synthesize_providers(&c,id,vndb_id.as_deref(),&baseline);
    let dlsite_code=detect_dlsite_code(&c,id,&baseline,&title,&orig,&desc);
    let preferences=baseline.get("_preferences").cloned().unwrap_or_else(||json!({}));
    Ok(json!({
        "source":baseline,
        "overrides":overrides,
        "revision":r,
        "providers":providers,
        "preferences":preferences,
        "ids":{"vndb_id":vndb_id,"vndb_release":vndb_release,"bangumi_id":bangumi_id,"dlsite_code":dlsite_code}
    }))
}
fn validate(field:&str,v:&Value)->R<()> {
    if ["tags","aliases"].contains(&field){if !v.as_array().is_some_and(|a|a.iter().all(Value::is_string)){return Err(format!("{field} must be a list of text values"));}}
    else if !v.is_string(){return Err(format!("{field} must be text"));}
    if ["title","original_title"].contains(&field)&&v.as_str().unwrap_or("").trim().is_empty(){return Err("Titles cannot be empty".into());}
    if field=="cover"{
        let url=v.as_str().unwrap_or("").trim();
        if !url.is_empty()&&!crate::artwork::allowed(url){return Err("Cover URL must come from VNDB, Bangumi, DLsite, or an uploaded local image".into());}
    }
    Ok(())
}
fn resolve_field<'a>(field:&str,pref:&str,providers:&'a Value,baseline:&'a Value,overrides:&'a Value)->Option<&'a Value>{
    let auto_pick=||{
        for p in PROVIDERS{
            if let Some(v)=providers.get(p).and_then(|o|o.get(field)).filter(|v|non_empty(v)){return Some(v);}
        }
        baseline.get(field)
    };
    match pref{
        "manual"=>overrides.get(field).or_else(auto_pick),
        "vndb"|"bangumi"|"dlsite"=>providers.get(pref).and_then(|o|o.get(field)).filter(|v|non_empty(v)).or_else(auto_pick),
        _=>overrides.get(field).or_else(auto_pick),
    }
}
pub fn apply(db:&Db,id:&str,revision:i64,patch:&Value,reset:&[String],source:Option<Value>)->R<i64>{
    apply_with_preferences(db,id,revision,patch,reset,None,source,None)
}
pub fn apply_with_preferences(
    db:&Db,
    id:&str,
    revision:i64,
    patch:&Value,
    reset:&[String],
    preferences:Option<&Value>,
    source:Option<Value>,
    provider_update:Option<(&str,Value,bool)>,
)->R<i64>{
    let mut c=db.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
    let(old,raw,over,vndb_id):(i64,String,String,Option<String>)=tx.query_row("SELECT revision,source_json,overrides,vndb_id FROM works WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
    if old!=revision{return Err("This work changed. Reload before editing.".into());}
    let mut baseline:Value=serde_json::from_str(&raw).map_err(|e|e.to_string())?;
    let mut overrides:Value=serde_json::from_str(&over).map_err(|e|e.to_string())?;
    if !baseline.is_object(){baseline=json!({});}
    if !overrides.is_object(){overrides=json!({});}
    if let Some(source)=source{
        let mut vndb_prov=serde_json::Map::new();
        for f in FIELDS{if let Some(v)=source.get(f){validate(f,v)?;baseline[f]=v.clone();vndb_prov.insert(f.into(),v.clone());}}
        baseline["refreshed_at"]=json!(crate::db::now());
        if !baseline["_providers"].is_object(){baseline["_providers"]=json!({});}
        baseline["_providers"]["vndb"]=Value::Object(vndb_prov);
    }
    if let Some((pname,pdata,apply_empty))=provider_update{
        if !PROVIDERS.contains(&pname){return Err("Unknown metadata provider".into());}
        if !baseline["_providers"].is_object(){baseline["_providers"]=json!({});}
        for f in FIELDS{if let Some(v)=pdata.get(f){validate(f,v)?;}}
        baseline["_providers"][pname]=pdata.clone();
        if pname=="vndb"{
            for f in FIELDS{if let Some(v)=pdata.get(f){baseline[f]=v.clone();}}
            baseline["refreshed_at"]=json!(crate::db::now());
        }
        if apply_empty{
            for f in FIELDS{
                if let Some(v)=pdata.get(f).filter(|v|non_empty(v)){
                    let cur_empty=baseline.get(f).is_none_or(|cur|!non_empty(cur));
                    if cur_empty{baseline[f]=v.clone();}
                }
            }
        }
    }
    if let Some(prefs)=preferences{
        let map=prefs.as_object().ok_or("Preferences must be an object")?;
        if !baseline["_preferences"].is_object(){baseline["_preferences"]=json!({});}
        for (field,pref) in map{
            if !FIELDS.contains(&field.as_str()){return Err(format!("Unknown preference field: {field}"));}
            let pref_str=pref.as_str().ok_or("Preference value must be text")?;
            if !VALID_PREFS.contains(&pref_str){return Err(format!("Unsupported preference: {pref_str}"));}
            if pref_str=="auto"{
                baseline["_preferences"].as_object_mut().unwrap().remove(field);
                overrides.as_object_mut().unwrap().remove(field);
            }else{
                baseline["_preferences"][field]=json!(pref_str);
                if pref_str!="manual"{overrides.as_object_mut().unwrap().remove(field);}
            }
        }
    }
    for(key,v)in patch.as_object().ok_or("Metadata patch must be an object")?{
        if !FIELDS.contains(&key.as_str()){return Err(format!("Unknown metadata field: {key}"));}
        validate(key,v)?;
        overrides[key]=v.clone();
        if baseline["_preferences"].is_object(){baseline["_preferences"][key]=json!("manual");}
    }
    for f in reset{
        if !FIELDS.contains(&f.as_str()){return Err("Unknown reset field".into());}
        let (providers,_)=synthesize_providers(&tx,id,vndb_id.as_deref(),&baseline);
        let has_any=baseline.get(f).is_some()||PROVIDERS.iter().any(|p|providers.get(p).and_then(|o|o.get(f)).is_some());
        if !has_any{return Err(format!("No source value exists for {f}"));}
        overrides.as_object_mut().ok_or("Invalid overrides")?.remove(f);
        if let Some(prefs)=baseline.get_mut("_preferences").and_then(Value::as_object_mut){prefs.remove(f);}
    }
    let (providers,_)=synthesize_providers(&tx,id,vndb_id.as_deref(),&baseline);
    for f in FIELDS{
        let pref=baseline["_preferences"][f].as_str().unwrap_or("auto");
        if let Some(v)=resolve_field(f,pref,&providers,&baseline,&overrides){
            validate(f,v)?;
            let value=if v.is_array(){v.to_string()}else{v.as_str().unwrap().into()};
            tx.execute(&format!("UPDATE works SET {f}=?2 WHERE id=?1"),params![id,value]).map_err(|e|e.to_string())?;
        }
    }
    tx.execute("UPDATE works SET source_json=?2,overrides=?3,revision=revision+1 WHERE id=?1",params![id,baseline.to_string(),overrides.to_string()]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;Ok(old+1)
}
pub async fn fetch_source(app:&crate::App,id:&str,source_kind:&str,raw_id:&str,revision:i64,apply_empty:bool)->R<Value>{
    let raw_id=raw_id.trim();
    if raw_id.is_empty(){return Err("Source identifier is required".into());}
    match source_kind{
        "vndb"=>{
            let fetched=fetch(raw_id,None).await?;
            if let Some(cover)=fetched["cover"].as_str().filter(|u|crate::artwork::allowed(u)){
                let _=app.db.lock().unwrap().execute("INSERT OR IGNORE INTO artwork_references(url,seen) VALUES(?1,?2)",params![cover,crate::db::now()]);
            }
            {
                let c=app.db.lock().map_err(|e|e.to_string())?;
                c.execute("UPDATE works SET vndb_id=?2 WHERE id=?1 AND merged_into IS NULL",params![id,raw_id]).map_err(|e|e.to_string())?;
            }
            apply_with_preferences(&app.db,id,revision,&json!({}),&[],None,None,Some(("vndb",fetched,apply_empty)))?;
            snapshot(&app.db,id)
        }
        "bangumi"=>{
            let subject=crate::bangumi::fetch_subject_for_app(app,raw_id).await.map_err(|e|e.0)?;
            let subj_val=serde_json::to_value(&subject).map_err(|e|e.to_string())?;
            let pdata=subject_to_provider(&subj_val);
            if let Some(cover)=pdata["cover"].as_str().filter(|u|crate::artwork::allowed(u)){
                let _=app.db.lock().unwrap().execute("INSERT OR IGNORE INTO artwork_references(url,seen) VALUES(?1,?2)",params![cover,crate::db::now()]);
            }
            {
                let mut c=app.db.lock().map_err(|e|e.to_string())?;
                let tx=c.transaction().map_err(|e|e.to_string())?;
                let other:Option<String>=tx.query_row("SELECT work_id FROM bangumi_links WHERE subject_id=?1",[subject.id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
                if other.as_ref().is_some_and(|wid|wid!=id){
                    return Err("This Bangumi subject is already linked to another work".into());
                }
                tx.execute("INSERT INTO bangumi_links(work_id,subject_id) VALUES(?1,?2) ON CONFLICT(work_id) DO UPDATE SET subject_id=excluded.subject_id,base_status=NULL,hidden=0",params![id,subject.id]).map_err(|e|e.to_string())?;
                tx.execute("INSERT INTO bangumi_remote(subject_id,status,subject) VALUES(?1,0,?2) ON CONFLICT(subject_id) DO UPDATE SET subject=excluded.subject",params![subject.id,subj_val.to_string()]).map_err(|e|e.to_string())?;
                tx.execute("DELETE FROM bangumi_ignored WHERE subject_id=?1",[subject.id]).map_err(|e|e.to_string())?;
                tx.commit().map_err(|e|e.to_string())?;
            }
            apply_with_preferences(&app.db,id,revision,&json!({}),&[],None,None,Some(("bangumi",pdata,apply_empty)))?;
            snapshot(&app.db,id)
        }
        "dlsite"=>{
            let code=crate::dlsite::extract_codes(raw_id).into_iter().next().unwrap_or_else(||raw_id.to_ascii_uppercase());
            let product=crate::dlsite::lookup(&code).await?;
            let pdata=dlsite_to_provider(&product);
            if let Some(cover)=pdata["cover"].as_str().filter(|u|crate::artwork::allowed(u)){
                let _=app.db.lock().unwrap().execute("INSERT OR IGNORE INTO artwork_references(url,seen) VALUES(?1,?2)",params![cover,crate::db::now()]);
            }
            apply_with_preferences(&app.db,id,revision,&json!({}),&[],None,None,Some(("dlsite",pdata,apply_empty)))?;
            snapshot(&app.db,id)
        }
        _=>Err("Unsupported metadata source".into()),
    }
}
/// Provider metadata for a work. A remake card (`release`) keeps its own release title, date, producers and art.
/// Fields where a manual value now differs from the freshly fetched provider value. Refresh keeps the manual value;
/// the person decides per field whether to keep it or reset it to the provider value.
pub fn conflicts(db:&Db,id:&str)->R<Vec<Value>>{
    let snap=snapshot(db,id)?;let mut out=Vec::new();
    for f in FIELDS{if let (Some(mine),Some(source))=(snap["overrides"].get(f),snap["source"].get(f)){if mine!=source{out.push(json!({"field":f,"mine":mine,"provider":source}));}}}
    Ok(out)
}
pub async fn fetch(id:&str,release:Option<&str>)->R<Value>{
    let mut work=fetch_work(id).await?;
    if let Some(release)=release{
        if !release.strip_prefix('r').is_some_and(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_digit())){return Err("Invalid VNDB release identifier".into());}
        crate::vndb_rate::wait(std::time::Duration::from_millis(1600)).await;
        let response=reqwest::Client::new().post(format!("{}/release",crate::matching::vndb_api())).timeout(std::time::Duration::from_secs(25)).json(&json!({"filters":["id","=",release],"fields":"title,alttitle,released,producers.name,images.url,images.type","results":1})).send().await.map_err(|e|e.to_string())?;
        if !response.status().is_success(){return Err(format!("VNDB returned {}; try again later",response.status()));}
        let response:Value=response.json().await.map_err(|e|e.to_string())?;let mut r=response["results"].as_array().and_then(|v|v.first()).cloned().ok_or("VNDB release not found")?;
        r["release_title"]=r["title"].clone();
        let card=crate::matching::release_card(&json!({"image":{"url":work["cover"]}}),&r);
        for k in ["title","original_title","released","developer","cover"]{work[k]=card[k].clone();}
    }
    Ok(work)
}
async fn fetch_work(id:&str)->R<Value>{
    if !id.strip_prefix('v').is_some_and(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_digit())){return Err("Invalid VNDB work identifier".into());}
    crate::vndb_rate::wait(std::time::Duration::from_millis(1600)).await;
    let response=reqwest::Client::new().post(format!("{}/vn",crate::matching::vndb_api())).timeout(std::time::Duration::from_secs(25)).json(&json!({"filters":["id","=",id],"fields":"title,alttitle,aliases,description,image.url,released,developers.name,tags.name","results":1})).send().await.map_err(|e|e.to_string())?;
    if !response.status().is_success(){return Err(format!("VNDB returned {}; try again later",response.status()));}
    let response:Value=response.json().await.map_err(|e|e.to_string())?;let v=response["results"].as_array().and_then(|v|v.first()).ok_or("VNDB work not found")?;
    let title=v["title"].as_str().ok_or("VNDB response has no title")?;
    Ok(json!({"title":title,"original_title":v["alttitle"].as_str().unwrap_or(title),"aliases":v["aliases"].as_array().cloned().unwrap_or_default(),"description":v["description"].as_str().unwrap_or(""),"cover":v["image"]["url"].as_str().unwrap_or(""),"released":v["released"].as_str().unwrap_or(""),"developer":v["developers"].as_array().map(|a|a.iter().filter_map(|x|x["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),"tags":v["tags"].as_array().map(|a|a.iter().filter_map(|x|x["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default()}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn conflicts_list_only_manual_values_that_differ_from_the_provider(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("db")).unwrap();db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,source_json) VALUES('w','First','First',?1)",[json!({"title":"First","original_title":"First","released":"2020"}).to_string()]).unwrap();
  apply(&db,"w",1,&json!({"title":"My title","released":"2021"}),&[],None).unwrap();apply(&db,"w",2,&json!({}),&[],Some(json!({"title":"Updated","original_title":"First","released":"2021"}))).unwrap();
  let c=conflicts(&db,"w").unwrap();assert_eq!(c,vec![json!({"field":"title","mine":"My title","provider":"Updated"})]);}
 #[test]fn refresh_preserves_override_and_reset_uses_latest_source(){let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("db")).unwrap();db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,source_json) VALUES('w','First','First',?1)",[json!({"title":"First","original_title":"First"}).to_string()]).unwrap();assert_eq!(apply(&db,"w",1,&json!({"title":"My title"}),&[],None).unwrap(),2);apply(&db,"w",2,&json!({}),&[],Some(json!({"title":"Updated source"}))).unwrap();let title=||db.lock().unwrap().query_row("SELECT title FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap();assert_eq!(title(),"My title");apply(&db,"w",3,&json!({}),&["title".into()],None).unwrap();assert_eq!(title(),"Updated source");assert!(apply(&db,"w",3,&json!({"title":"Stale edit"}),&[],None).is_err());assert_eq!(title(),"Updated source");assert!(apply(&db,"w",4,&json!({"title":""}),&[],None).is_err());assert_eq!(snapshot(&db,"w").unwrap()["revision"],4);}
 #[test]fn roon_style_provider_preferences_and_fallback_resolve_cover_and_fields(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("db")).unwrap();
  db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,cover,description,source_json) VALUES('w','Manual','Manual','','DLsite RJ351325',?1)",[json!({"title":"Manual","original_title":"Manual","cover":""}).to_string()]).unwrap();
  let snap=snapshot(&db,"w").unwrap();
  assert_eq!(snap["ids"]["dlsite_code"],"RJ351325");
  let dl_cover="https://img.dlsite.jp/modpub/images2/work/doujin/RJ352000/RJ351325_img_main.jpg";
  let bgm_cover="https://lain.bgm.tv/pic/cover/l/98/48/61169_4u8lj.jpg";
  let local_cover="local://artwork/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.jpg";
  // Store DLsite provider -> auto fills empty cover
  let rev=apply_with_preferences(&db,"w",1,&json!({}),&[],None,None,Some(("dlsite",json!({"code":"RJ351325","title":"DLsite Title","cover":dl_cover}),true))).unwrap();
  assert_eq!(db.lock().unwrap().query_row("SELECT cover FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),dl_cover);
  // Store Bangumi provider and explicitly prefer Bangumi cover
  let rev=apply_with_preferences(&db,"w",rev,&json!({}),&[],Some(&json!({"cover":"bangumi"})),None,Some(("bangumi",json!({"subject_id":61169,"title":"BGM Title","cover":bgm_cover}),false))).unwrap();
  assert_eq!(db.lock().unwrap().query_row("SELECT cover FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),bgm_cover);
  // Manual custom uploaded cover overrides provider
  let rev=apply_with_preferences(&db,"w",rev,&json!({"cover":local_cover}),&[],None,None,None).unwrap();
  assert_eq!(db.lock().unwrap().query_row("SELECT cover FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),local_cover);
  // Reject arbitrary untrusted cover URL
  assert!(apply_with_preferences(&db,"w",rev,&json!({"cover":"https://evil.example/x.jpg"}),&[],None,None,None).is_err());
  // Switch preference back to dlsite clears manual override
  apply_with_preferences(&db,"w",rev,&json!({}),&[],Some(&json!({"cover":"dlsite"})),None,None).unwrap();
  assert_eq!(db.lock().unwrap().query_row("SELECT cover FROM works WHERE id='w'",[],|r|r.get::<_,String>(0)).unwrap(),dl_cover);
 }
}

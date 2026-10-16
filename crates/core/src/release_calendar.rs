//! Release calendar: VNDB releases for this month and the six months either side.
//! Results live in a separate cache file for a week and are purged on every read; nothing is written into the collection.
use std::{collections::BTreeMap,path::Path,sync::OnceLock,time::{Duration,SystemTime,UNIX_EPOCH}};
use axum::{extract::{Query,State},http::HeaderMap,Json};
use rusqlite::{Connection,OptionalExtension,params};
use serde::Deserialize;
use serde_json::{Value,json};
use crate::{ApiError,App};

const FILE:&str="release-calendar.sqlite";
const TTL_SECONDS:i64=7*24*3600;
pub const WINDOW:i64=6;
const PAGE_SIZE:usize=100;
const MAX_PAGES:usize=3;
const FIELDS:&str="id,title,alttitle,released,producers.name,producers.developer,vns.id,vns.rtype,vns.title,vns.alttitle,vns.image.url";
static FETCH:OnceLock<tokio::sync::Mutex<()>>=OnceLock::new();

/// Months are counted as `year * 12 + month - 1` so the window is plain integer arithmetic.
fn month_index(year:i64,month:i64)->i64{year*12+month-1}
pub(crate) fn label(index:i64)->String{format!("{:04}-{:02}",index.div_euclid(12),index.rem_euclid(12)+1)}
fn first_day(index:i64)->String{format!("{}-01",label(index))}
pub fn parse_month(value:&str)->Option<i64>{
 let (year,month)=value.split_once('-')?;
 if year.len()!=4||month.len()!=2||!year.bytes().chain(month.bytes()).all(|b|b.is_ascii_digit()){return None;}
 let (year,month)=(year.parse::<i64>().ok()?,month.parse::<i64>().ok()?);
 (1..=12).contains(&month).then(||month_index(year,month))
}
/// The UTC month containing `now` (civil-from-days, valid for any date after 1970).
pub fn current_month(now:SystemTime)->i64{
 let days=now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64/86_400;
 let z=days+719_468;let era=z.div_euclid(146_097);let doe=z-era*146_097;
 let yoe=(doe-doe/1_460+doe/36_524-doe/146_096)/365;let doy=doe-(365*yoe+yoe/4-yoe/100);
 let mp=(5*doy+2)/153;let month=if mp<10{mp+3}else{mp-9};let year=yoe+era*400+i64::from(month<=2);
 month_index(year,month)
}
fn now_seconds()->i64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64}

fn open(dir:&Path)->Result<Connection,String>{
 let c=Connection::open(dir.join(FILE)).map_err(|e|e.to_string())?;
 c.execute_batch("PRAGMA journal_mode=WAL;CREATE TABLE IF NOT EXISTS months(month INTEGER PRIMARY KEY,fetched INTEGER NOT NULL,body TEXT NOT NULL);").map_err(|e|e.to_string())?;
 Ok(c)
}
/// Drops expired months and months that have left the window around `today`.
fn purge(c:&Connection,today:i64,now:i64)->Result<usize,String>{
 c.execute("DELETE FROM months WHERE fetched<=?1 OR month<?2 OR month>?3",params![now-TTL_SECONDS,today-WINDOW,today+WINDOW]).map_err(|e|e.to_string())
}
fn cached(dir:&Path,month:i64,today:i64,now:i64)->Result<Option<(i64,Value)>,String>{
 let c=open(dir)?;purge(&c,today,now)?;
 let row=c.query_row("SELECT fetched,body FROM months WHERE month=?1",[month],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|e.to_string())?;
 Ok(row.and_then(|(fetched,body)|serde_json::from_str(&body).ok().map(|value|(fetched,value))))
}
fn store(dir:&Path,month:i64,now:i64,releases:&Value)->Result<(),String>{
 open(dir)?.execute("INSERT INTO months(month,fetched,body) VALUES(?1,?2,?3) ON CONFLICT(month) DO UPDATE SET fetched=excluded.fetched,body=excluded.body",params![month,now,releases.to_string()]).map(|_|()).map_err(|e|e.to_string())
}
/// Artwork for a cached release may be shown while its month is cached.
pub fn references(dir:&Path,url:&str)->Result<bool,String>{
 if !dir.join(FILE).exists(){return Ok(false);}
 open(dir)?.query_row("SELECT EXISTS(SELECT 1 FROM months WHERE fetched>?1 AND instr(body,?2)>0)",params![now_seconds()-TTL_SECONDS,format!("\"{url}\"")],|r|r.get(0)).map_err(|e|e.to_string())
}

/// One row per visual novel: its earliest complete, official, non-patch Japanese Windows release in the month.
fn condense(results:&[Value])->Value{
 let mut by_vn:BTreeMap<String,Value>=BTreeMap::new();
 for release in results{
  let Some(vn)=release["vns"].as_array().and_then(|vns|vns.iter().find(|vn|vn["rtype"]=="complete")) else {continue};
  let Some(id)=vn["id"].as_str() else {continue};
  let released=release["released"].as_str().unwrap_or_default();
  if by_vn.get(id).is_some_and(|row|row["released"].as_str().unwrap_or_default()<=released){continue;}
  let producers=release["producers"].as_array().map(|all|{let developers:Vec<&Value>=all.iter().filter(|p|p["developer"]==true).collect();(if developers.is_empty(){all.iter().collect()}else{developers}).into_iter().filter_map(|p|p["name"].as_str()).map(str::to_owned).collect::<Vec<_>>()}).unwrap_or_default();
  by_vn.insert(id.to_owned(),json!({
   "release_id":release["id"],"vn_id":id,"released":released,
   "title":vn["title"].as_str().or(release["title"].as_str()).unwrap_or_default(),
   "original_title":vn["alttitle"].as_str().or(release["alttitle"].as_str()).unwrap_or_default(),
   "producers":producers,
   "image":vn["image"]["url"].as_str().filter(|url|crate::artwork::allowed(url)).map(|url|json!({"url":url})).unwrap_or(Value::Null)
  }));
 }
 let mut rows:Vec<Value>=by_vn.into_values().collect();
 // Month-only dates ("2026-10") have no day yet; list them after the dated releases.
 let undated=|v:&Value|v["released"].as_str().map_or(true,|d|d.len()<10);
 rows.sort_by(|a,b|undated(a).cmp(&undated(b)).then(a["released"].as_str().cmp(&b["released"].as_str())).then(a["title"].as_str().cmp(&b["title"].as_str())));
 Value::Array(rows)
}
async fn fetch(api:&str,month:i64)->Result<Value,String>{
 let client=reqwest::Client::new();let mut results=Vec::new();
 let filters=json!(["and",["released",">=",first_day(month)],["released","<",first_day(month+1)],["official","=",1],["patch","!=",1],["lang","=","ja"],["platform","=","win"]]);
 for page in 1..=MAX_PAGES{
  crate::vndb_rate::wait(Duration::from_millis(1600)).await;
  let body=json!({"filters":filters,"fields":FIELDS,"sort":"released","results":PAGE_SIZE,"page":page});
  let response=client.post(format!("{api}/release")).timeout(Duration::from_secs(8)).json(&body).send().await.map_err(|_|"VNDB could not be reached within 8 seconds. Try again later.".to_string())?;
  if !response.status().is_success(){return Err(format!("VNDB returned {}. Try again later.",response.status()));}
  let data=response.json::<Value>().await.map_err(|_|"VNDB returned an unreadable response".to_string())?;
  let Some(rows)=data["results"].as_array() else {return Err("VNDB returned no results field".into())};
  results.extend(rows.iter().cloned());
  if data["more"]!=true{break;}
 }
 Ok(condense(&results))
}
async fn releases(dir:&Path,api:&str,month:i64,today:i64)->Result<(i64,Value),ApiError>{
 let lookup=|dir:std::path::PathBuf|async move{tokio::task::spawn_blocking(move||cached(&dir,month,today,now_seconds())).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)};
 if let Some(hit)=lookup(dir.to_path_buf()).await?{return Ok(hit);}
 // One fetch at a time; a second caller for the same month finds the first caller's rows.
 let _turn=FETCH.get_or_init(||tokio::sync::Mutex::new(())).lock().await;
 if let Some(hit)=lookup(dir.to_path_buf()).await?{return Ok(hit);}
 let rows=fetch(api,month).await.map_err(ApiError)?;let now=now_seconds();
 let (base,value)=(dir.to_path_buf(),rows.clone());
 tokio::task::spawn_blocking(move||store(&base,month,now,&value)).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)?;
 Ok((now,rows))
}
/// Marks rows the collection already holds, read live so status changes show at once.
fn annotate(c:&Connection,rows:&mut Value)->rusqlite::Result<()>{
 let Some(rows)=rows.as_array_mut() else {return Ok(())};
 let mut statement=c.prepare("SELECT id,status FROM works WHERE vndb_id=?1 AND merged_into IS NULL ORDER BY rowid LIMIT 1")?;
 for row in rows{
  let Some(vn)=row["vn_id"].as_str().map(str::to_owned) else {continue};
  if let Some((id,status))=statement.query_row([&vn],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{row["work_id"]=json!(id);row["status"]=json!(status);}
 }
 Ok(())
}

#[derive(Deserialize)]#[serde(deny_unknown_fields)]pub struct Request{month:String}
pub async fn get(State(a):State<App>,h:HeaderMap,Query(q):Query<Request>)->crate::Result<Value>{
 crate::auth(&a,&h)?;
 let today=current_month(SystemTime::now());
 let month=parse_month(&q.month).ok_or_else(||ApiError("Invalid month".into()))?;
 if (month-today).abs()>WINDOW{return Err(ApiError("Month is outside the release calendar".into()));}
 let (fetched,mut rows)=releases(&a.state_dir,&crate::matching::vndb_api(),month,today).await?;
 {let c=a.db.lock().unwrap();annotate(&c,&mut rows)?;}
 Ok(Json(json!({"month":label(month),"current_month":label(today),"window":WINDOW,"fetched_at":fetched,"expires_at":fetched+TTL_SECONDS,"releases":rows})))
}

/// Route-wide tests read the calendar without reaching VNDB.
#[cfg(test)]pub(crate) fn seed(dir:&Path,month:i64){store(dir,month,now_seconds(),&json!([])).unwrap();}

#[cfg(test)]mod tests{
 use super::*;
 use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
 #[test]fn months_parse_and_follow_the_utc_calendar(){
  assert_eq!(parse_month("2026-10"),Some(month_index(2026,10)));assert_eq!(parse_month("2026-13"),None);assert_eq!(parse_month("26-10"),None);assert_eq!(parse_month("2026-1"),None);
  assert_eq!(label(month_index(2026,12)+1),"2027-01");assert_eq!(first_day(month_index(2027,1)),"2027-01-01");
  let at=|s:u64|current_month(UNIX_EPOCH+Duration::from_secs(s));
  assert_eq!(label(at(0)),"1970-01");assert_eq!(label(at(1_792_108_800)),"2026-10");assert_eq!(label(at(1_709_208_000)),"2024-02");assert_eq!(label(at(1_709_251_199)),"2024-02");assert_eq!(label(at(1_709_251_200)),"2024-03");
 }
 #[test]fn condense_keeps_the_earliest_complete_release_per_work(){
  let rows=condense(&[
   json!({"id":"r3","released":"2026-10-24","producers":[{"name":"Publisher","developer":false},{"name":"Studio","developer":true}],"vns":[{"id":"v1","rtype":"complete","title":"Alpha","alttitle":"アルファ","image":{"url":"https://t.vndb.org/cv/01/1.jpg"}}]}),
   json!({"id":"r2","released":"2026-10-10","producers":[{"name":"Publisher","developer":false}],"vns":[{"id":"v1","rtype":"complete","title":"Alpha","image":{"url":"https://t.vndb.org/cv/01/1.jpg"}}]}),
   json!({"id":"r4","released":"2026-10-02","vns":[{"id":"v2","rtype":"trial","title":"Demo"}]}),
   json!({"id":"r6","released":"2026-10","vns":[{"id":"v4","rtype":"complete","title":"Undated"}]}),
   json!({"id":"r5","released":"2026-10-05","vns":[{"id":"v3","rtype":"complete","title":"Beta","image":{"url":"https://example.com/x.jpg"}}]})
  ]);
  let rows=rows.as_array().unwrap();assert_eq!(rows.len(),3);assert_eq!(rows[2]["vn_id"],"v4");
  assert_eq!(rows[0]["vn_id"],"v3");assert!(rows[0]["image"].is_null());
  assert_eq!(rows[1]["release_id"],"r2");assert_eq!(rows[1]["producers"],json!(["Publisher"]));assert_eq!(rows[1]["image"]["url"],"https://t.vndb.org/cv/01/1.jpg");
 }
 #[tokio::test]async fn cache_serves_repeat_reads_and_purges_old_or_distant_months(){
  use axum::{routing::post,Router};
  let calls=Arc::new(AtomicUsize::new(0));let seen=calls.clone();
  let app=Router::new().route("/release",post(move|Json(body):Json<Value>|{let seen=seen.clone();async move{
   seen.fetch_add(1,Ordering::SeqCst);assert_eq!(body["filters"][1],json!(["released",">=","2026-10-01"]));assert_eq!(body["filters"][2],json!(["released","<","2026-11-01"]));
   let page=body["page"].as_i64().unwrap();
   Json(json!({"more":page==1,"results":[{"id":format!("r{page}"),"released":format!("2026-10-0{page}"),"vns":[{"id":format!("v{page}"),"rtype":"complete","title":format!("Work {page}"),"image":{"url":format!("https://t.vndb.org/cv/0{page}/{page}.jpg")}}]}]}))
  }}));
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=format!("http://{}",listener.local_addr().unwrap());let server=tokio::spawn(async move{axum::serve(listener,app).await.unwrap();});
  let t=tempfile::tempdir().unwrap();let today=month_index(2026,10);
  let (_,first)=releases(t.path(),&api,today,today).await.map_err(|e|e.0).unwrap();assert_eq!(first.as_array().unwrap().len(),2);assert_eq!(calls.load(Ordering::SeqCst),2);
  let (_,again)=releases(t.path(),&api,today,today).await.map_err(|e|e.0).unwrap();assert_eq!(again,first);assert_eq!(calls.load(Ordering::SeqCst),2);
  assert!(references(t.path(),"https://t.vndb.org/cv/02/2.jpg").unwrap());assert!(!references(t.path(),"https://t.vndb.org/cv/09/9.jpg").unwrap());
  let c=open(t.path()).unwrap();c.execute("INSERT INTO months(month,fetched,body) VALUES(?1,?2,'[]'),(?3,?2,'[]')",params![today+1,now_seconds(),today-7]).unwrap();
  assert_eq!(purge(&c,today,now_seconds()).unwrap(),1);assert_eq!(purge(&c,today,now_seconds()+TTL_SECONDS).unwrap(),2);
  assert!(!references(t.path(),"https://t.vndb.org/cv/02/2.jpg").unwrap());
  server.abort();
 }
 #[test]fn annotation_links_owned_works_and_their_status(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let c=db.lock().unwrap();
  c.execute("INSERT INTO works(id,title,original_title,vndb_id,status) VALUES('w1','Alpha','アルファ','v1','playing')",[]).unwrap();
  let mut rows=json!([{"vn_id":"v1"},{"vn_id":"v2"}]);annotate(&c,&mut rows).unwrap();
  assert_eq!(rows[0]["work_id"],"w1");assert_eq!(rows[0]["status"],"playing");assert!(rows[1].get("work_id").is_none());
 }
}

use axum::{extract::{Query,State},http::{HeaderMap,header},response::{IntoResponse,Response}};
use serde::Deserialize;
use crate::{ApiError,App,artwork_cache::{Cache,Image,MAX_IMAGE_BYTES}};
static DOWNLOAD:std::sync::OnceLock<crate::artwork_queue::Queue>=std::sync::OnceLock::new();
#[derive(Deserialize)]pub struct Request{url:String}
fn allowed(value:&str)->bool{
 // Only immutable raster paths on the provider's artwork host. Never follow redirects.
 regex::Regex::new(r"^https://t\.vndb\.org/(cv|ch|sf)(\.t)?/[0-9]{2}/[0-9]+\.(jpg|png|webp)$").unwrap().is_match(value)
}
pub(crate) fn referenced(value:&serde_json::Value,url:&str)->bool{
 fn walk(value:&serde_json::Value,url:&str,depth:usize)->bool{if depth>32{return false;}match value{
  serde_json::Value::Object(object)=>object.get("image").and_then(|image|image.get("url")).and_then(|v|v.as_str())==Some(url)||object.values().any(|v|walk(v,url,depth+1)),
  serde_json::Value::Array(values)=>values.iter().any(|v|walk(v,url,depth+1)),_=>false
 }}walk(value,url,0)
}
async fn known(a:&App,url:&str)->Result<bool,ApiError>{
 let db=a.db.clone();let target=url.to_owned();let stored=tokio::task::spawn_blocking(move||->Result<bool,String>{
  let c=db.lock().map_err(|e|e.to_string())?;
  if c.query_row("SELECT EXISTS(SELECT 1 FROM works WHERE cover=?1 AND merged_into IS NULL)",[&target],|r|r.get::<_,bool>(0)).map_err(|e|e.to_string())?{return Ok(true);}
  c.query_row("SELECT EXISTS(SELECT 1 FROM artwork_references WHERE url=?1)",[&target],|r|r.get::<_,bool>(0)).map_err(|e|e.to_string())
 }).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)?;
 Ok(stored||crate::matching::known_artwork(url).await)
}
async fn cached(base:std::path::PathBuf,url:String)->Result<Option<Image>,ApiError>{
 tokio::task::spawn_blocking(move||Cache::open(&base)?.get(&url)).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)
}
fn response(image:Image)->Response{
 ([(header::CONTENT_TYPE,image.mime),(header::CACHE_CONTROL,"private, no-store".into()),(header::X_CONTENT_TYPE_OPTIONS,"nosniff".into())],image.bytes).into_response()
}
pub async fn stats(State(a):State<App>,h:HeaderMap)->Result<axum::Json<crate::artwork_cache::Stats>,ApiError>{manage(a,h,false).await}
pub async fn clear(State(a):State<App>,h:HeaderMap)->Result<axum::Json<crate::artwork_cache::Stats>,ApiError>{manage(a,h,true).await}
async fn manage(a:App,h:HeaderMap,clear:bool)->Result<axum::Json<crate::artwork_cache::Stats>,ApiError>{
 if crate::access::authenticate(&a,&h)?.role!="owner"{return Err(ApiError("Local owner access required".into()));}
 let result=tokio::task::spawn_blocking(move||{let mut cache=Cache::open(&a.state_dir)?;if clear{cache.clear()}else{cache.stats()}}).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)?;
 Ok(axum::Json(result))
}
async fn download(url:&str)->Result<Vec<u8>,ApiError>{
 let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(20)).build().map_err(|e|ApiError(e.to_string()))?;
 let mut remote=client.get(url).send().await.map_err(|_|ApiError("Artwork download unavailable".into()))?;
 if !remote.status().is_success(){return Err(ApiError("Artwork download failed".into()));}
 if remote.content_length().is_some_and(|n|n>MAX_IMAGE_BYTES as u64){return Err(ApiError("Artwork exceeds size limit".into()));}
 let mut bytes=Vec::new();while let Some(chunk)=remote.chunk().await.map_err(|_|ApiError("Artwork download interrupted".into()))?{if chunk.len()>MAX_IMAGE_BYTES-bytes.len(){return Err(ApiError("Artwork exceeds size limit".into()));}bytes.extend_from_slice(&chunk);}
 Ok(bytes)
}
pub async fn read(State(a):State<App>,h:HeaderMap,Query(q):Query<Request>)->Result<Response,ApiError>{
 crate::access::authenticate(&a,&h)?;
 if !allowed(&q.url){return Err(ApiError("Unsupported artwork URL".into()));}
 if !known(&a,&q.url).await?{return Err(ApiError("Artwork is not referenced by this Core".into()));}
 if let Some(image)=cached(a.state_dir.clone(),q.url.clone()).await?{return Ok(response(image));}
 let key=format!("{}:{}",a.state_dir.display(),Cache::key(&q.url));
 let _permit=DOWNLOAD.get_or_init(||crate::artwork_queue::Queue::new(4,64)).enter(key).await.map_err(ApiError)?;
 if let Some(image)=cached(a.state_dir.clone(),q.url.clone()).await?{return Ok(response(image));}
 let started=std::time::Instant::now();crate::diagnostics::record("info","artwork.download.started","Artwork download started",None);
 let downloaded=download(&q.url).await;
 crate::diagnostics::record(if downloaded.is_ok(){"info"}else{"error"},if downloaded.is_ok(){"artwork.download.completed"}else{"artwork.download.failed"},&format!("Artwork download ended after {} ms",started.elapsed().as_millis()),None);
 let bytes=downloaded?;
 let base=a.state_dir;let url=q.url;
 let image=tokio::task::spawn_blocking(move||{let mut cache=Cache::open(&base)?;cache.put(&url,&bytes)?;cache.get(&url)?.ok_or_else(||"Artwork cache write unavailable".to_string())}).await.map_err(|e|ApiError(e.to_string()))?.map_err(ApiError)?;
 Ok(response(image))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn references_only_accept_typed_image_fields(){
  let url="https://t.vndb.org/cv/01/1.jpg";assert!(referenced(&serde_json::json!({"results":[{"image":{"url":url}}]}),url));assert!(!referenced(&serde_json::json!({"description":url,"extlinks":[{"url":url}]}),url));
 }
 async fn serve(bytes:Vec<u8>)->(String,tokio::task::JoinHandle<()>){
  use tokio::io::{AsyncReadExt,AsyncWriteExt};
  let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/image",listener.local_addr().unwrap());
  let task=tokio::spawn(async move{let(mut socket,_)=listener.accept().await.unwrap();let mut request=[0u8;4096];let _=socket.read(&mut request).await;let _=socket.write_all(&bytes).await;});(url,task)
 }
 #[tokio::test]async fn downloader_rejects_redirect_status_and_truncated_body(){
  for(raw,expected)in [
   ("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: 0\r\n\r\n","Artwork download failed"),
   ("HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n","Artwork download failed"),
   ("HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort","Artwork download interrupted")]{
   let(url,task)=serve(raw.as_bytes().to_vec()).await;let result=download(&url).await.map_err(|e|e.0);assert_eq!(result.unwrap_err(),expected);task.await.unwrap();
  }
 }
 #[tokio::test]async fn downloader_bounds_declared_and_chunked_sizes_and_accepts_exact_payload(){
  let raw=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",MAX_IMAGE_BYTES+1);let(url,task)=serve(raw.into_bytes()).await;assert_eq!(download(&url).await.map_err(|e|e.0).unwrap_err(),"Artwork exceeds size limit");task.await.unwrap();
  let mut raw=format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",MAX_IMAGE_BYTES+1).into_bytes();raw.extend(vec![1;MAX_IMAGE_BYTES+1]);raw.extend_from_slice(b"\r\n0\r\n\r\n");let(url,task)=serve(raw).await;assert_eq!(download(&url).await.map_err(|e|e.0).unwrap_err(),"Artwork exceeds size limit");task.await.unwrap();
  let(url,task)=serve(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc".to_vec()).await;assert_eq!(download(&url).await.map_err(|e|e.0).unwrap(),b"abc");task.await.unwrap();
 }
 #[test]fn provider_raster_paths_only(){
  for url in ["https://t.vndb.org/cv/01/123.jpg","https://t.vndb.org/ch.t/12/345.png","https://t.vndb.org/sf/00/99.webp"]{assert!(allowed(url));}
  for url in ["http://t.vndb.org/cv/01/1.jpg","https://t.vndb.org.evil/cv/01/1.jpg","https://user@t.vndb.org/cv/01/1.jpg","https://t.vndb.org:443/cv/01/1.jpg","https://t.vndb.org/cv/01/1.jpg?token=secret","https://t.vndb.org/cv/01/1.svg","https://127.0.0.1/a.png","file:///C:/secret","https://t.vndb.org/cv/01/../1.jpg"]{assert!(!allowed(url),"{url}");}
 }
 #[tokio::test]async fn authenticated_cached_image_needs_no_network(){
  use tower::ServiceExt;
  let t=tempfile::tempdir().unwrap();let a=crate::initialize(t.path().to_path_buf()).unwrap();let token=a.token.clone();let url="https://t.vndb.org/cv/01/123.jpg";
  let bytes=b"\xff\xd8\xfffixture";Cache::open(t.path()).unwrap().put(url,bytes).unwrap();
  a.db.lock().unwrap().execute("INSERT INTO works(id,title,original_title,cover) VALUES('w','Fixture','Fixture',?1)",[url]).unwrap();
  let app=axum::Router::new().route("/art",axum::routing::get(read)).with_state(a);
  let path=format!("/art?url={url}");
  let denied=app.clone().oneshot(axum::http::Request::builder().uri(&path).body(axum::body::Body::empty()).unwrap()).await.unwrap();assert_eq!(denied.status(),401);
  let ok=app.oneshot(axum::http::Request::builder().uri(path).header("Authorization",format!("Bearer {token}")).body(axum::body::Body::empty()).unwrap()).await.unwrap();assert_eq!(ok.status(),200);assert_eq!(ok.headers()[header::CONTENT_TYPE],"image/jpeg");assert_eq!(ok.headers()[header::CACHE_CONTROL],"private, no-store");assert_eq!(&axum::body::to_bytes(ok.into_body(),MAX_IMAGE_BYTES).await.unwrap()[..],bytes);
 }
}


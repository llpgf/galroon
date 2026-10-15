//! Background workers owned once by Core, independently of its HTTP listeners.
use crate::{App,automatch,smart_updates,watching};

pub struct Services{vndb_sync:crate::vndb_sync::Worker,bangumi:crate::bangumi::Worker,downloads:crate::collection_download::Worker,updates:crate::updates::Worker,smart:smart_updates::Worker,matching:automatch::Worker,watching:watching::Service}
impl Services{
 pub fn start(app:App)->Result<Self,String>{
  let watching=watching::start(app.clone())?;let matching=automatch::start(app.db.clone());let smart=smart_updates::start(app.db.clone());
  let vndb_sync=crate::vndb_sync::start(app.clone());let bangumi=crate::bangumi::start(app.clone());let updates=crate::updates::start(app.updates.clone());let downloads=crate::collection_download::start(app);Ok(Self{vndb_sync,bangumi,downloads,watching,matching,smart,updates})
 }
 pub fn request_stop(&self){self.vndb_sync.request_stop();self.bangumi.request_stop();self.downloads.request_stop();self.updates.request_stop();self.watching.request_stop();self.matching.request_stop();self.smart.request_stop();}
 pub(crate) fn stop_smart_updates(&self){self.smart.request_stop();}
 pub(crate) fn restart_smart_updates(&mut self,db:crate::db::Db){self.smart=smart_updates::start(db);}
}
impl Drop for Services{fn drop(&mut self){self.request_stop();}}

#[cfg(test)]mod tests{
 use super::*;
 use tokio::io::{AsyncReadExt,AsyncWriteExt};
 #[tokio::test]async fn one_services_owns_workers_for_two_listeners_and_shared_shutdown(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();
  let first=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let second=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let addresses=[first.local_addr().unwrap(),second.local_addr().unwrap()];
  // One owner outside the listener list; serve_listeners never constructs Services.
  let services=Services::start(app.clone()).unwrap();let (stop,stopped)=tokio::sync::oneshot::channel();
  let server=tokio::spawn(crate::serve_listeners(app.clone(),vec![(first,crate::Transport::Loopback),(second,crate::Transport::Loopback)],async{let _=stopped.await;}));
  let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(3)).build().unwrap();
  for address in addresses{
   let health=client.get(format!("http://{address}/api/health")).send().await.unwrap();assert_eq!(health.status(),reqwest::StatusCode::OK);assert_eq!(health.json::<serde_json::Value>().await.unwrap()["name"],"Galroon");
   let who=client.get(format!("http://{address}/api/access/me")).bearer_auth(&app.token).send().await.unwrap().error_for_status().unwrap().json::<serde_json::Value>().await.unwrap();assert_eq!(who["role"],"owner");
  }
  // Hold a request in each listener's JSON extractor, then complete both after shutdown.
  let body=r#"{"title":"Drain fixture"}"#;let mut pending=Vec::new();
  for address in addresses{
   let mut stream=tokio::net::TcpStream::connect(address).await.unwrap();let headers=format!("POST /api/works HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",app.token,body.len());stream.write_all(headers.as_bytes()).await.unwrap();
   let mut ready=[0;25];tokio::time::timeout(std::time::Duration::from_secs(3),stream.read_exact(&mut ready)).await.unwrap().unwrap();assert_eq!(&ready,b"HTTP/1.1 100 Continue\r\n\r\n");pending.push(stream);
  }
  stop.send(()).unwrap();
  // Windows retries a refused connect for about two seconds, so allow several attempts.
  tokio::time::timeout(std::time::Duration::from_secs(20),async{for address in addresses{while tokio::net::TcpStream::connect(address).await.is_ok(){tokio::time::sleep(std::time::Duration::from_millis(10)).await;}}}).await.unwrap();
  for mut stream in pending{
   assert!(!server.is_finished(),"Server returned before all requests drained");stream.write_all(body.as_bytes()).await.unwrap();let mut response=String::new();tokio::time::timeout(std::time::Duration::from_secs(3),stream.read_to_string(&mut response)).await.unwrap().unwrap();assert!(response.starts_with("HTTP/1.1 200 OK\r\n"),"{response}");
  }
  tokio::time::timeout(std::time::Duration::from_secs(3),server).await.unwrap().unwrap().unwrap();
  assert_eq!(app.db.lock().unwrap().query_row("SELECT count(*) FROM works WHERE title='Drain fixture'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
  for address in addresses{assert!(tokio::net::TcpStream::connect(address).await.is_err(),"Listener {address} still accepts connections");}
  services.request_stop();drop(services);
 }
}

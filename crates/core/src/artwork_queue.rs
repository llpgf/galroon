//! Bounded downloads with per-cache-key serialization and cancellation by drop.
use std::{collections::HashMap,sync::{Arc,Mutex,Weak},time::Duration};
use tokio::sync::{Semaphore,OwnedSemaphorePermit,OwnedMutexGuard};
pub struct Queue{slots:Arc<Semaphore>,admission:Arc<Semaphore>,keys:Mutex<HashMap<String,Weak<tokio::sync::Mutex<()>>>>}
pub struct Permit{_download:OwnedSemaphorePermit,_key:OwnedMutexGuard<()>,_admission:OwnedSemaphorePermit}
impl Queue{
 pub fn new(downloads:usize,pending:usize)->Self{Self{slots:Arc::new(Semaphore::new(downloads)),admission:Arc::new(Semaphore::new(pending)),keys:Mutex::new(HashMap::new())}}
 pub async fn enter(&self,key:String)->Result<Permit,String>{
  let admission=self.admission.clone().try_acquire_owned().map_err(|_|"Artwork queue is full; retry")?;
  let lock={let mut keys=self.keys.lock().map_err(|_|"Artwork queue unavailable")?;keys.retain(|_,v|v.strong_count()>0);let lock=keys.get(&key).and_then(Weak::upgrade).unwrap_or_else(||Arc::new(tokio::sync::Mutex::new(())));keys.insert(key,Arc::downgrade(&lock));lock};
  tokio::time::timeout(Duration::from_secs(30),async{
   let key=lock.lock_owned().await;
   let download=self.slots.clone().acquire_owned().await.map_err(|_|"Artwork queue unavailable".to_string())?;
   Ok(Permit{_download:download,_key:key,_admission:admission})
  }).await.map_err(|_|"Artwork download queue is busy; retry".to_string())?
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[tokio::test]async fn distinct_downloads_are_bounded_and_same_key_does_not_consume_extra_slot(){
  let q=Arc::new(Queue::new(2,4));let first=q.enter("a".into()).await.unwrap();let other=q.clone();let waiting=tokio::spawn(async move{other.enter("a".into()).await});tokio::task::yield_now().await;
  let second=q.enter("b".into()).await.unwrap();assert_eq!(q.slots.available_permits(),0);assert!(!waiting.is_finished());drop(first);let duplicate=waiting.await.unwrap().unwrap();assert_eq!(q.slots.available_permits(),0);drop(duplicate);drop(second);assert_eq!(q.slots.available_permits(),2);assert_eq!(q.admission.available_permits(),4);
 }
 #[tokio::test]async fn cancellation_and_full_queue_release_capacity(){
  let q=Arc::new(Queue::new(1,2));let first=q.enter("a".into()).await.unwrap();let other=q.clone();let waiting=tokio::spawn(async move{other.enter("b".into()).await});tokio::task::yield_now().await;assert_eq!(q.admission.available_permits(),0);
  assert!(q.enter("c".into()).await.is_err());waiting.abort();assert!(matches!(waiting.await,Err(e) if e.is_cancelled()));assert_eq!(q.admission.available_permits(),1);drop(first);let third=q.enter("c".into()).await.unwrap();drop(third);assert_eq!(q.admission.available_permits(),2);assert_eq!(q.slots.available_permits(),1);
 }
}


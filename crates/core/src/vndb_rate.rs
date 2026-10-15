//! One request budget for matching, browsing and background collection downloads.
use std::{sync::OnceLock,time::Duration};
use tokio::{sync::Mutex,time::Instant};
static LAST:OnceLock<Mutex<Option<Instant>>>=OnceLock::new();
async fn acquire(last:&Mutex<Option<Instant>>,spacing:Duration)->Instant{
 let mut last=last.lock().await;
 if let Some(at)=*last{tokio::time::sleep_until(at+spacing).await;}
 let now=Instant::now();*last=Some(now);now
}
pub(crate) async fn wait(spacing:Duration){
 // Provider fixtures explicitly use zero spacing; they do not consume the live budget.
 if !spacing.is_zero(){acquire(LAST.get_or_init(||Mutex::new(None)),spacing).await;}
}
#[cfg(test)]mod tests{
 use super::*;
 #[tokio::test]async fn concurrent_callers_share_spacing_and_cancelled_waits_release_the_gate(){
  let gate=std::sync::Arc::new(Mutex::new(None));let spacing=Duration::from_millis(30);
  acquire(&gate,spacing).await;
  let waiting=gate.clone();let cancelled=tokio::spawn(async move{acquire(&waiting,Duration::from_secs(60)).await;});
  tokio::task::yield_now().await;cancelled.abort();let _=cancelled.await;
  let mut tasks=Vec::new();for _ in 0..3{let gate=gate.clone();tasks.push(tokio::spawn(async move{acquire(&gate,spacing).await}));}
  let mut starts=Vec::new();for task in tasks{starts.push(tokio::time::timeout(Duration::from_secs(3),task).await.unwrap().unwrap());}starts.sort();
  for times in starts.windows(2){assert!(times[1].duration_since(times[0])>=spacing);}
 }
}

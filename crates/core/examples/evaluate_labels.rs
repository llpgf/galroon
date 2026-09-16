//! Independent labels are read only for evaluation, never used in candidate retrieval.
use serde_json::{json,Value};
fn resume(bytes: Option<&[u8]>, samples: &[Value], labels_sha256: &str, evaluator_sha256: &str) -> Result<Vec<Value>, String> {
 let mut ids=std::collections::HashSet::new();
 for sample in samples {
  let id=sample.get("case").filter(|v|v.is_string()||v.is_number()).ok_or("Every sample needs a case ID")?;
  if !ids.insert(id.to_string()) {return Err("Duplicate sample case ID".into());}
 }
 let rows:Vec<Value>=match bytes {Some(b)=>serde_json::from_slice(b).map_err(|e|format!("Existing output is invalid; preserve it and choose a fresh output: {e}"))?,None=>vec![]};
 let mut seen=std::collections::HashSet::new();
 for row in &rows {
  if row["labels_sha256"]!=labels_sha256||row["evaluator_sha256"]!=evaluator_sha256 {return Err("Output provenance differs or is missing; use a fresh output for these labels and this evaluator".into());}
  if !ids.contains(&row["case"].to_string())||!seen.insert(row["case"].to_string()) {return Err("Unknown or duplicate output case ID".into());}
  let sample=samples.iter().find(|s|s["case"]==row["case"]).unwrap();
  if row["expected"]!=sample["expected"]||row["category"]!=sample["category"] {return Err("Output label fields differ from the supplied samples".into());}
 }
 Ok(rows)
}
#[tokio::main]async fn main(){
 let args:Vec<_>=std::env::args().collect();let samples:Vec<Value>=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();assert!(!samples.is_empty()&&samples.len()<=40);
 let labels_sha256=galroon_core::plans::hash(std::path::Path::new(&args[1])).unwrap().1;
 let evaluator_sha256=galroon_core::plans::hash(&std::env::current_exe().unwrap()).unwrap().1;
 let existing=match std::fs::read(&args[2]){Ok(bytes)=>Some(bytes),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>None,Err(e)=>panic!("Cannot read existing output: {e}")};
 let mut report=resume(existing.as_deref(),&samples,&labels_sha256,&evaluator_sha256).expect("Unsafe evaluation resume rejected before querying VNDB");
 for s in samples{if report.iter().any(|r|r["case"]==s["case"]){continue;}let hints:Vec<String>=match &s["hints"]{Value::Null=>vec![],Value::String(v)=>vec![v.clone()],v=>serde_json::from_value(v.clone()).expect("hints must be strings")};let input=galroon_core::matching::prepare(s["raw"].as_str().unwrap(),&hints);
 let result=match input{Ok(i)=>galroon_core::matching::search(i).await,Err(e)=>Err(e)};
 let mut row=match result{Ok(response)=>{let reason=galroon_core::automatch::review_reason(&response);let proposed=if reason.is_none(){response["results"][0]["id"].clone()}else{Value::Null};let wrong=!proposed.is_null()&&proposed!=s["expected"];json!({"case":s["case"],"category":s["category"],"expected":s["expected"],"proposed":proposed,"wrong_auto":wrong,"top":response["results"][0]["id"],"reason":reason,"response":response})},Err(e)=>json!({"case":s["case"],"category":s["category"],"expected":s["expected"],"error":e})};
 row["labels_sha256"]=json!(labels_sha256);row["evaluator_sha256"]=json!(evaluator_sha256);
 println!("{}",json!({"case":row["case"],"proposed":row["proposed"],"expected":row["expected"],"wrong_auto":row["wrong_auto"],"error":row["error"]}));report.push(row);std::fs::write(&args[2],serde_json::to_vec_pretty(&report).unwrap()).unwrap();
 }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn changed_labels_or_evaluator_cannot_reuse_results(){
  let samples=vec![json!({"case":1,"expected":"v1","category":"title"})];
  let row=json!({"case":1,"expected":"v1","category":"title","labels_sha256":"labels-a","evaluator_sha256":"exe-a"});
  let bytes=serde_json::to_vec(&vec![row]).unwrap();
  assert_eq!(resume(Some(&bytes),&samples,"labels-a","exe-a").unwrap().len(),1);
  assert!(resume(Some(&bytes),&samples,"labels-b","exe-a").is_err());
  assert!(resume(Some(&bytes),&samples,"labels-a","exe-b").is_err());
 }
 #[test]fn invalid_or_unattributed_existing_output_is_not_discarded(){
  let samples=vec![json!({"case":1})];
  assert!(resume(Some(b"truncated"),&samples,"l","e").is_err());
  assert!(resume(Some(br#"[{"case":1}]"#),&samples,"l","e").is_err());
  assert!(resume(None,&samples,"l","e").unwrap().is_empty());
 }
 #[test]fn duplicate_or_unknown_cases_cannot_count_as_completed(){
  let sample=json!({"case":1});let row=json!({"case":1,"labels_sha256":"l","evaluator_sha256":"e"});
  assert!(resume(None,&[sample.clone(),sample.clone()],"l","e").is_err());
  let duplicated=serde_json::to_vec(&vec![row.clone(),row]).unwrap();
  assert!(resume(Some(&duplicated),std::slice::from_ref(&sample),"l","e").is_err());
  let unknown=serde_json::to_vec(&vec![json!({"case":2,"labels_sha256":"l","evaluator_sha256":"e"})]).unwrap();
  assert!(resume(Some(&unknown),&[sample],"l","e").is_err());
 }
}



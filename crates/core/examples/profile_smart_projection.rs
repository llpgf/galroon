//! Read-only diagnostic sampling; not acceptance or a changed performance deadline.
use rusqlite::{Connection,OpenFlags};
fn main(){
 let path=std::env::args().nth(1).expect("Generated fixture database required");
 for category in ["actual","status","organizing","relations","edition"]{let capacity=16;
  let c=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();c.set_prepared_statement_cache_capacity(capacity);
  c.execute_batch("CREATE TEMP TABLE sampled_ids(id TEXT PRIMARY KEY); INSERT INTO sampled_ids SELECT id FROM main.works ORDER BY id LIMIT 2000; CREATE TEMP VIEW works AS SELECT * FROM main.works WHERE id IN (SELECT id FROM sampled_ids);").unwrap();
  let raw:String=c.query_row("SELECT definition FROM smart_lists WHERE id='fixture'",[],|r|r.get(0)).unwrap();let mut definition:galroon_core::smart_lists::Definition=serde_json::from_str(&raw).unwrap();
  let predicate=|field:&str,value:&str|serde_json::json!({"kind":"field","predicate":{"field":field,"mode":"any","values":[value]}});
  let rule=match category{"status"=>Some(predicate("status","backlog")),"organizing"=>Some(predicate("organizing_state","unorganized")),"relations"=>Some(predicate("brands","p1")),"edition"=>Some(serde_json::json!({"kind":"edition","join":"all","conditions":[{"field":"confirmed_languages","mode":"any","values":["ja"]}]})),_=>None};if let Some(rule)=rule{definition.rule=serde_json::from_value(rule).unwrap();}

  let started=std::time::Instant::now();let result=galroon_core::smart_snapshots::preview_definition(&c,&definition,&std::collections::BTreeMap::from([("fixture-root".into(),false)])).ok().unwrap();
  println!("{}",serde_json::json!({"category":category,"diagnostic_sample":2000,"statement_cache_capacity":capacity,"elapsed_ms":started.elapsed().as_millis(),"matches":result["total"]}));
 }
}

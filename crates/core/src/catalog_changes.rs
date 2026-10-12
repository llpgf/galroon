//! Public invalidation sequence, independent of caches, authentication and paging revisions.
use rusqlite::Connection;
use axum::{extract::State,http::HeaderMap,Json};
use serde_json::{Value,json};
// Editions are stored as releases. Join-only changes are visible catalog changes too.
pub const TABLES:&[&str]=&["works","releases","resources","files","resource_files","work_releases","resource_bindings","work_preferences","custom_tags","custom_tag_works","custom_tag_entities","curated_lists","list_entries","list_entry_variants","smart_lists","relation_overrides","relation_corrections","entity_overrides","roots","issues","plans","plan_locations","plan_reversals","operations","quarantine","match_items","match_suggestions","jobs","job_summary_entries","library_identity"];
pub fn migrate(c:&Connection)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS catalog_change_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),seq INTEGER NOT NULL); INSERT OR IGNORE INTO catalog_change_state VALUES(1,0);")?;
 for table in TABLES{
  let mut q=c.prepare(&format!("PRAGMA table_info({table})"))?;let columns=q.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
  let changed=columns.iter().map(|col|format!("OLD.{col} IS NOT NEW.{col}")).collect::<Vec<_>>().join(" OR ");
  for action in ["INSERT","UPDATE","DELETE"]{let when=if action=="UPDATE"{format!("WHEN {changed}")}else{String::new()};c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS catalog_changed_{table}_{action} AFTER {action} ON {table} {when} BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END;"))?;}
 }
 // Watch settings and visible watcher state, excluding event queues and bookkeeping.
 for action in ["INSERT","UPDATE","DELETE"]{let when=if action=="UPDATE"{"WHEN OLD.enabled IS NOT NEW.enabled OR OLD.exclude IS NOT NEW.exclude OR OLD.revision IS NOT NEW.revision OR OLD.status IS NOT NEW.status OR OLD.message IS NOT NEW.message OR OLD.needs_scan IS NOT NEW.needs_scan OR OLD.job_id IS NOT NEW.job_id OR OLD.pending IS NOT NEW.pending"}else{""};c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS catalog_changed_source_watch_{action} AFTER {action} ON source_watch {when} BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END;"))?;}
 for action in ["INSERT","UPDATE","DELETE"]{let row=if action=="DELETE"{"OLD"}else{"NEW"};let changed=if action=="UPDATE"{" AND OLD.value IS NOT NEW.value"}else{""};c.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS catalog_changed_settings_{action} AFTER {action} ON settings WHEN {row}.key IN ('organization.managed_root','auto_match_enabled','auto_match_requested'){changed} BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END;"))?;}
 // Failed background computations are visible, but retry timestamps and leases are not.
 c.execute_batch("CREATE TRIGGER IF NOT EXISTS catalog_changed_smart_error_insert AFTER INSERT ON smart_compute_state WHEN NEW.error<>'' BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END; CREATE TRIGGER IF NOT EXISTS catalog_changed_smart_error_update AFTER UPDATE OF error ON smart_compute_state WHEN OLD.error IS NOT NEW.error BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END;")?;
 // Only physical availability changes count; smart_catalog_state also tracks cache fills.
 c.execute_batch("CREATE TRIGGER IF NOT EXISTS catalog_changed_online AFTER UPDATE OF root_digest ON smart_catalog_state WHEN OLD.root_digest IS NOT NEW.root_digest BEGIN UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1; END;")?;
 Ok(())
}
pub fn bump(c:&Connection)->rusqlite::Result<()>{c.execute("UPDATE catalog_change_state SET seq=seq+1 WHERE singleton=1",[])?;Ok(())}
pub fn seq(c:&Connection)->rusqlite::Result<i64>{c.query_row("SELECT seq FROM catalog_change_state WHERE singleton=1",[],|r|r.get(0))}
pub async fn read(State(a):State<crate::App>,h:HeaderMap)->crate::Result<Value>{crate::auth(&a,&h)?;let c=a.db.lock().unwrap();Ok(Json(json!({"seq":seq(&c)?,"library_id":crate::context::library(&c)?.id})))}
#[cfg(test)]mod tests;

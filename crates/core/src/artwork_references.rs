use rusqlite::Connection;
pub fn migrate(c:&Connection,backfill:bool)->rusqlite::Result<()>{
 c.execute_batch("CREATE TABLE IF NOT EXISTS artwork_references(url TEXT NOT NULL,source TEXT NOT NULL,PRIMARY KEY(url,source));CREATE INDEX IF NOT EXISTS artwork_reference_source ON artwork_references(source);CREATE INDEX IF NOT EXISTS works_active_cover ON works(cover) WHERE merged_into IS NULL;")?;
 let insert="INSERT OR IGNORE INTO artwork_references(url,source) SELECT leaf.value,NEW.key FROM json_tree(CASE WHEN (NEW.key LIKE 'exploration.%' OR NEW.key LIKE 'discovery.v1.%') AND json_valid(NEW.value) THEN NEW.value ELSE '{}' END) leaf JOIN json_tree(CASE WHEN (NEW.key LIKE 'exploration.%' OR NEW.key LIKE 'discovery.v1.%') AND json_valid(NEW.value) THEN NEW.value ELSE '{}' END) parent ON leaf.parent=parent.id WHERE leaf.key='url' AND leaf.type='text' AND parent.key='image' AND parent.type='object';";
 c.execute_batch(&format!("DROP TRIGGER IF EXISTS artwork_ref_insert;CREATE TRIGGER artwork_ref_insert AFTER INSERT ON settings BEGIN DELETE FROM artwork_references WHERE source=NEW.key;{insert} END;CREATE TRIGGER IF NOT EXISTS artwork_ref_update AFTER UPDATE ON settings BEGIN DELETE FROM artwork_references WHERE source=OLD.key;{insert} END;CREATE TRIGGER IF NOT EXISTS artwork_ref_delete AFTER DELETE ON settings BEGIN DELETE FROM artwork_references WHERE source=OLD.key;END;"))?;
 if backfill{c.execute_batch("DELETE FROM artwork_references;INSERT OR IGNORE INTO artwork_references(url,source) SELECT leaf.value,s.key FROM settings s,json_tree(CASE WHEN (s.key LIKE 'exploration.%' OR s.key LIKE 'discovery.v1.%') AND json_valid(s.value) THEN s.value ELSE '{}' END) leaf JOIN json_tree(CASE WHEN (s.key LIKE 'exploration.%' OR s.key LIKE 'discovery.v1.%') AND json_valid(s.value) THEN s.value ELSE '{}' END) parent ON leaf.parent=parent.id WHERE leaf.key='url' AND leaf.type='text' AND parent.key='image' AND parent.type='object';")?;}Ok(())
}
#[cfg(test)]mod tests{
 #[test]fn schema34_rebuild_removes_stale_replaced_urls(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");{let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();c.execute("INSERT INTO settings(key,value) VALUES('exploration.x',?1)",[r#"{"image":{"url":"current"}}"#]).unwrap();c.execute_batch("INSERT INTO artwork_references(url,source) VALUES('stale','exploration.x');PRAGMA user_version=34;").unwrap();}
  let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM artwork_references",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT url FROM artwork_references",[],|r|r.get::<_,String>(0)).unwrap(),"current");
 }

 #[test]fn references_track_replace_delete_and_rollback(){
  let t=tempfile::tempdir().unwrap();let db=crate::db::open(&t.path().join("library.sqlite")).unwrap();let mut c=db.lock().unwrap();
  c.execute("INSERT INTO settings(key,value) VALUES('exploration.fixture',?1)",[r#"{"results":[{"image":{"url":"a"}},{"image":{"url":"a"}}],"extlinks":[{"url":"ignored"}]}"#]).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM artwork_references",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  {let tx=c.transaction().unwrap();tx.execute("UPDATE settings SET value=?1 WHERE key='exploration.fixture'",[r#"{"image":{"url":"b"}}"#]).unwrap();assert_eq!(tx.query_row("SELECT url FROM artwork_references",[],|r|r.get::<_,String>(0)).unwrap(),"b");tx.rollback().unwrap();}
  assert_eq!(c.query_row("SELECT url FROM artwork_references",[],|r|r.get::<_,String>(0)).unwrap(),"a");c.execute("INSERT OR REPLACE INTO settings(key,value) VALUES('exploration.fixture',?1)",[r#"{"image":{"url":"replacement"}}"#]).unwrap();assert_eq!(c.query_row("SELECT url FROM artwork_references",[],|r|r.get::<_,String>(0)).unwrap(),"replacement");c.execute("UPDATE settings SET value='invalid' WHERE key='exploration.fixture'",[]).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM artwork_references",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn schema33_backfills_only_typed_cache_references(){
  let t=tempfile::tempdir().unwrap();let path=t.path().join("library.sqlite");{let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();c.execute_batch("DROP TRIGGER artwork_ref_insert;DROP TRIGGER artwork_ref_update;DROP TRIGGER artwork_ref_delete;DROP TABLE artwork_references;PRAGMA user_version=33;").unwrap();for key in ["exploration.x","discovery.v1.x","private.notes"]{c.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![key,r#"{"image":{"url":"same"}}"#]).unwrap();}}
  let db=crate::db::open(&path).unwrap();let c=db.lock().unwrap();assert_eq!(c.query_row("SELECT count(*) FROM artwork_references",[],|r|r.get::<_,i64>(0)).unwrap(),2);c.execute("DELETE FROM settings WHERE key='exploration.x'",[]).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM artwork_references",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
}



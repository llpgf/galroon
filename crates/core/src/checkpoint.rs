//! Move large edition checkpoints off the response path, without changing FULL durability.
//! WAL remains SQLite-owned. A partial PASSIVE checkpoint is safe and never removes it.
use rusqlite::{Connection, OpenFlags};
use std::{collections::BTreeSet, path::{Path, PathBuf}, sync::{Condvar, Mutex, OnceLock}};

#[derive(Default)]
struct State { pending: BTreeSet<PathBuf>, active: Option<PathBuf>, running: bool }
fn queue() -> &'static (Mutex<State>, Condvar) {
    static QUEUE: OnceLock<(Mutex<State>, Condvar)> = OnceLock::new();
    QUEUE.get_or_init(|| (Mutex::new(State::default()), Condvar::new()))
}
fn request(path: PathBuf) {
    let (lock, changed) = queue();
    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
    state.pending.insert(path);
    if state.running { return; }
    state.running = true;
    if let Err(error) = std::thread::Builder::new().name("galroon-checkpoint".into()).spawn(worker) {
        // The writer's normal autocheckpoint is already restored; SQLite also checkpoints on close.
        eprintln!("Could not start background checkpoint: {error}");
        state.pending.clear(); state.running = false; changed.notify_all();
    }
}
fn worker() {
    let (lock, changed) = queue();
    loop {
        let path = {
            let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
            let Some(path) = state.pending.pop_first() else {
                state.running = false; changed.notify_all(); return;
            };
            state.active = Some(path.clone()); path
        };
        if let Err(error) = passive(&path) { eprintln!("Background checkpoint deferred: {error}"); }
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.active = None; changed.notify_all();
    }
}
fn passive(path: &Path) -> rusqlite::Result<(i64, i64, i64)> {
    // Never recreate a removed collection. PASSIVE does not wait for readers or writers.
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    c.busy_timeout(std::time::Duration::ZERO)?;
    c.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
}
/// Call after writers/HTTP have drained and before releasing the collection process lock.
pub fn drain(path: &Path) {
    let (lock, changed) = queue();
    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
    while state.pending.contains(path) || state.active.as_deref() == Some(path) {
        state = changed.wait(state).unwrap_or_else(|e| e.into_inner());
    }
}

pub(crate) struct LargeEdition<'a> {
    c: &'a Connection, original: Option<i64>, path: Option<PathBuf>, committed: bool,
}
impl<'a> LargeEdition<'a> {
    pub fn new(c: &'a Connection, edition: Option<&str>, incoming: usize) -> rusqlite::Result<Self> {
        let old: i64 = if let Some(id) = edition {
            c.query_row("SELECT count(*) FROM work_releases WHERE release_id=?1", [id], |r| r.get(0))?
        } else { 0 };
        let mut guard = Self { c, original: None, path: None, committed: false };
        if old.max(incoming as i64) < 1000 { return Ok(guard); }
        // Only production file-backed WAL/FULL connections qualify. Other modes retain their policy.
        let mode: String = c.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        let sync: i64 = c.pragma_query_value(None, "synchronous", |r| r.get(0))?;
        if mode != "wal" || sync != 2 { return Ok(guard); }
        if let Some(path) = c.path().filter(|p| !p.is_empty()) {
            let original = c.pragma_query_value(None, "wal_autocheckpoint", |r| r.get::<_,i64>(0))?;
            c.pragma_update(None, "wal_autocheckpoint", 0)?;
            guard.original = Some(original); guard.path = Some(PathBuf::from(path));
        }
        Ok(guard)
    }
    pub fn committed(&mut self) { self.committed = true; }
}
impl Drop for LargeEdition<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original {
            if let Err(error) = self.c.pragma_update(None, "wal_autocheckpoint", original) {
                // Do not turn an already committed edit into an apparent failed save.
                eprintln!("Could not restore automatic checkpoint: {error}");
            }
            if self.committed { if let Some(path) = self.path.take() { request(path); } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Connection) {
        let temp = tempfile::tempdir().unwrap();
        let c = Connection::open(temp.path().join("test.sqlite")).unwrap();
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE work_releases(release_id TEXT); CREATE TABLE sample(value INTEGER);").unwrap();
        (temp, c)
    }
    fn policy(c: &Connection) -> (i64, i64) {
        (c.pragma_query_value(None,"wal_autocheckpoint",|r|r.get(0)).unwrap(),c.pragma_query_value(None,"synchronous",|r|r.get(0)).unwrap())
    }
    #[test]
    fn rollback_restores_custom_policy_and_keeps_full_sync() {
        let (_temp,c) = fixture();
        c.pragma_update(None,"wal_autocheckpoint",321).unwrap();
        {
            let _guard = LargeEdition::new(&c,None,1000).unwrap();
            assert_eq!(policy(&c),(0,2));
            let tx=c.unchecked_transaction().unwrap();
            tx.execute("INSERT INTO sample VALUES(1)",[]).unwrap();
        }
        assert_eq!(policy(&c),(321,2));
        assert_eq!(c.query_row("SELECT count(*) FROM sample",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        let _small=LargeEdition::new(&c,None,999).unwrap();
        assert_eq!(policy(&c),(321,2));
    }
    #[test]
    fn committed_writes_survive_reader_and_background_drain() {
        let (_temp,c) = fixture();
        let path=PathBuf::from(c.path().unwrap());
        let reader=Connection::open(&path).unwrap();
        reader.execute_batch("BEGIN; SELECT * FROM sample;").unwrap();
        for n in 0..12 {
            let mut guard=LargeEdition::new(&c,None,1000).unwrap();
            let tx=c.unchecked_transaction().unwrap();
            tx.execute("INSERT INTO sample VALUES(?1)",[n]).unwrap();
            tx.commit().unwrap(); guard.committed(); drop(guard);
            assert_eq!(policy(&c),(1000,2));
        }
        drain(&path); // A long reader must not prevent worker shutdown.
        assert_eq!(reader.query_row("SELECT count(*) FROM sample",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        reader.execute_batch("COMMIT").unwrap();
        request(path.clone()); drain(&path);
        let check=passive(&path).unwrap(); assert_eq!(check.1,check.2);
        assert_eq!(c.query_row("SELECT sum(value) FROM sample",[],|r|r.get::<_,i64>(0)).unwrap(),66);
        assert_eq!(c.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
    }
}

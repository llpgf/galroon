//! One active organization destination per catalog. Historical root rows stay intact.
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
type R<T> = Result<T, String>;
pub const KEY: &str = "organization.managed_root";
fn sql<T>(v: rusqlite::Result<T>) -> R<T> { v.map_err(|e| e.to_string()) }
fn comparable(p: &Path) -> PathBuf {
    let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    #[cfg(windows)] {
        let s = p.to_string_lossy().replace('/', "\\").to_lowercase();
        // Offline roots cannot be canonicalized; compare them with Win32/UNC
        // spelling of live roots instead of mismatching the verbatim prefix.
        PathBuf::from(if let Some(rest)=s.strip_prefix("\\\\?\\unc\\") {format!("\\\\{rest}")}
            else {s.strip_prefix("\\\\?\\").unwrap_or(&s).to_owned()})
    }
    #[cfg(not(windows))] { p }
}
pub fn validate(c: &Connection, destination: &Path) -> R<()> {
    crate::plans::validate_chain(destination)?;
    if !destination.is_dir() { return Err("Reconnect the managed folder before organizing files.".into()); }
    let selected: Option<String> = sql(c.query_row("SELECT value FROM settings WHERE key=?1", [KEY], |r| r.get(0)).optional())?;
    let dest = comparable(destination);
    if let Some(selected) = selected {
        if comparable(Path::new(&selected)) != dest {
            return Err(format!("This collection uses one managed folder: {selected}. Select that folder. To relink it after an external move, use Change source location."));
        }
    }
    let mut s = sql(c.prepare("SELECT path,role FROM roots"))?;
    for row in sql(s.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))))? {
        let (path, role) = sql(row)?;
        let other = comparable(Path::new(&path));
        if role == "managed" && other == dest { continue; }
        if dest.starts_with(&other) || other.starts_with(&dest) {
            return Err("The managed folder must be separate from source, quarantine and other managed folders.".into());
        }
    }
    Ok(())
}
/// Called inside the approval/execution transaction; previews never select a root.
pub fn select_plan(c: &Connection, pid: &str) -> R<()> {
    let kind: String = sql(c.query_row("SELECT kind FROM plans WHERE id=?1", [pid], |r| r.get(0)))?;
    if kind != "organize" { return Ok(()); }
    let path: String = sql(c.query_row("SELECT root_path FROM plan_locations WHERE plan_id=?1", [pid], |r| r.get(0)))?;
    validate(c, Path::new(&path))?;
    sql(c.execute("INSERT OR IGNORE INTO settings(key,value) VALUES(?1,?2)", params![KEY,path]))?;
    sql(c.execute("INSERT OR IGNORE INTO roots(id,path,label,role) VALUES(?1,?2,'Managed library','managed')", params![crate::db::id(),path]))?;
    Ok(())
}

//! Endpoint discovery and the paired-device credential file.
//! The owner control-channel credential is never kept: discovery only reads the URL and library identity.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type R<T> = Result<T, String>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Credential {
    pub token: String,
    pub library_id: String,
    pub device_name: String,
    pub expires: i64,
}

pub struct Endpoint {
    pub url: String,
    pub library_id: Option<String>,
}

/// Only plain loopback HTTP origins are accepted, mirroring the desktop pairing boundary.
pub fn normalize_url(raw: &str) -> R<String> {
    let rest = raw.trim().trim_end_matches('/').strip_prefix("http://").ok_or("Core address must start with http://")?;
    let (host, port) = rest.rsplit_once(':').ok_or("Core address needs a port, e.g. http://127.0.0.1:51234")?;
    if !matches!(host, "127.0.0.1" | "localhost") {
        return Err("Only loopback Core addresses (127.0.0.1) are allowed".into());
    }
    let port: u16 = port.parse().map_err(|_| "Invalid Core port")?;
    if port == 0 {
        return Err("Invalid Core port".into());
    }
    Ok(format!("http://127.0.0.1:{port}"))
}

/// Tauri's app_local_data_dir for identifier app.galroon.desktop, or GALROON_DATA like the desktop app.
pub fn desktop_base() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("GALROON_DATA") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("app.galroon.desktop"))
}

/// Same pointer rules as the desktop Controller: the active collection must stay inside its base.
pub fn active_collection(base: &Path) -> R<PathBuf> {
    let pointer = base.join("active-library.json");
    if !pointer.exists() {
        return Ok(base.to_path_buf());
    }
    let bytes = std::fs::read(&pointer).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Collection pointer is too large".into());
    }
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let dir = PathBuf::from(v["current"].as_str().ok_or("Invalid collection pointer")?).canonicalize().map_err(|e| e.to_string())?;
    if !dir.starts_with(base.canonicalize().map_err(|e| e.to_string())?) {
        return Err("Collection pointer is outside its local data directory".into());
    }
    Ok(dir)
}

pub fn core_exe() -> R<PathBuf> {
    if let Some(exe) = std::env::var_os("GALROON_CORE_EXE") {
        return Ok(PathBuf::from(exe));
    }
    let own = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(own.parent().ok_or("No application directory")?.join("galroon-core.exe"))
}

/// Explicit address first; otherwise ask the running local Core through its identity-checked control channel.
pub async fn endpoint(explicit: Option<&str>) -> R<Endpoint> {
    if let Some(url) = explicit.map(str::to_owned).or_else(|| std::env::var("GALROON_URL").ok()).filter(|u| !u.trim().is_empty()) {
        return Ok(Endpoint { url: normalize_url(&url)?, library_id: None });
    }
    discover().await
}

#[cfg(windows)]
async fn discover() -> R<Endpoint> {
    let base = desktop_base().ok_or("LOCALAPPDATA is not set")?;
    let dir = active_collection(&base)?;
    let exe = core_exe()?.canonicalize().map_err(|_| "galroon-core.exe was not found next to galroon-mcp.exe; set GALROON_CORE_EXE".to_string())?;
    // Connect only; never launch Core from an MCP client.
    let session = galroon_core::local_core::request(&dir, &exe, "session")
        .await
        .map_err(|e| format!("Galroon Core is not running for this collection. Open Galroon first. ({e})"))?;
    let url = normalize_url(session["url"].as_str().ok_or("Core session has no address")?)?;
    let library_id = session["library_id"].as_str().map(str::to_owned);
    Ok(Endpoint { url, library_id })
}

#[cfg(not(windows))]
async fn discover() -> R<Endpoint> {
    Err("Automatic Core discovery is Windows-only. Pass --url http://127.0.0.1:<port> or set GALROON_URL".into())
}

pub fn credential_path() -> R<PathBuf> {
    if let Some(path) = std::env::var_os("GALROON_MCP_TOKEN_FILE") {
        return Ok(PathBuf::from(path));
    }
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or("No per-user configuration directory; set GALROON_MCP_TOKEN_FILE")?;
    Ok(root.join("galroon-mcp").join("credential.json"))
}

pub fn load_credential() -> R<Credential> {
    if let Ok(token) = std::env::var("GALROON_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(Credential { token: token.trim().to_owned(), library_id: String::new(), device_name: String::new(), expires: 0 });
        }
    }
    let path = credential_path()?;
    let bytes = std::fs::read(&path).map_err(|_| format!("Not paired yet. Create a pairing code in Galroon (Settings > Access) and run: galroon-mcp pair <CODE>  [{}]", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid credential file {}: {e}", path.display()))
}

pub fn save_credential(credential: &Credential) -> R<PathBuf> {
    let path = credential_path()?;
    let dir = path.parent().ok_or("Invalid credential path")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!(".credential-{}.tmp", uuid::Uuid::new_v4().simple()));
    let bytes = serde_json::to_vec_pretty(credential).map_err(|e| e.to_string())?;
    write_private(&temp, &bytes)?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })?;
    Ok(path)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> R<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
}

// %LOCALAPPDATA% is already restricted to the current Windows user.
#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> R<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_addresses_are_accepted() {
        assert_eq!(normalize_url("http://localhost:4000/").unwrap(), "http://127.0.0.1:4000");
        assert_eq!(normalize_url(" http://127.0.0.1:51234 ").unwrap(), "http://127.0.0.1:51234");
        for bad in ["https://127.0.0.1:1", "http://192.168.1.2:80", "http://127.0.0.1", "http://127.0.0.1:0", "http://user@127.0.0.1:80", "http://127.0.0.1:80/api"] {
            assert!(normalize_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn collection_pointer_cannot_escape_base() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().join("base");
        std::fs::create_dir_all(base.join("collections/a")).unwrap();
        assert_eq!(active_collection(&base).unwrap(), base);
        let inside = base.join("collections/a").canonicalize().unwrap();
        std::fs::write(base.join("active-library.json"), serde_json::json!({"current": inside}).to_string()).unwrap();
        assert_eq!(active_collection(&base).unwrap(), inside);
        std::fs::write(base.join("active-library.json"), serde_json::json!({"current": t.path()}).to_string()).unwrap();
        assert!(active_collection(&base).is_err());
    }
}

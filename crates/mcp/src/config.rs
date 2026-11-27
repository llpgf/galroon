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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
}

#[derive(Clone, Default)]
pub struct Options {
    pub url: Option<String>,
    pub fingerprint: Option<String>,
    pub token: Option<String>,
    pub token_env: Option<String>,
    pub library_id: Option<String>,
    pub device_id: Option<String>,
}

pub struct Endpoint {
    pub url: String,
    pub fingerprint: Option<String>,
    pub library_id: Option<String>,
    pub device_id: Option<String>,
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

/// Validate a remote LAN HTTPS origin using Core's private-network and origin rules.
pub fn normalize_lan_url(raw: &str) -> R<String> {
    galroon_core::lan_client::endpoint(raw)
}

/// Validate either a loopback HTTP origin or a pinned LAN HTTPS origin.
/// HTTPS always requires a valid 52-character base32 SPKI fingerprint; HTTP rejects fingerprints.
pub fn normalize_endpoint(raw: &str, fingerprint: Option<&str>) -> R<(String, Option<String>)> {
    let trimmed = raw.trim();
    let fp = fingerprint.map(str::trim).filter(|s| !s.is_empty());
    if trimmed.starts_with("https://") {
        let url = normalize_lan_url(trimmed)?;
        let fp = fp.ok_or("LAN HTTPS Core requires --fingerprint <SPKI> (or GALROON_FINGERPRINT)")?;
        galroon_core::lan_client::decode_fingerprint(fp)?;
        Ok((url, Some(fp.to_owned())))
    } else {
        if fp.is_some() {
            return Err("SPKI --fingerprint is only valid with an HTTPS LAN Core URL".into());
        }
        Ok((normalize_url(trimmed)?, None))
    }
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

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Explicit address first; otherwise ask the running local Core through its identity-checked control channel.
pub async fn endpoint(explicit: Option<&str>) -> R<Endpoint> {
    resolve_endpoint(&Options { url: explicit.map(str::to_owned), ..Default::default() }, None).await
}

/// Resolve either an explicit/env/saved LAN or loopback endpoint, or discover the local Core on Windows.
pub async fn resolve_endpoint(options: &Options, saved: Option<&Credential>) -> R<Endpoint> {
    let explicit_url = options.url.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).or_else(|| non_empty_env("GALROON_URL"));
    let explicit_fp = options.fingerprint.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).or_else(|| non_empty_env("GALROON_FINGERPRINT"));
    let raw_url = explicit_url.or_else(|| saved.and_then(|c| c.url.clone()).filter(|s| !s.trim().is_empty()));
    if let Some(raw_url) = raw_url {
        // Only inherit a saved SPKI pin when the URL matches the saved endpoint.
        let fp = explicit_fp.or_else(|| {
            let saved = saved?;
            let saved_url = saved.url.as_deref().and_then(|u| normalize_lan_url(u).ok())?;
            let target_url = normalize_lan_url(&raw_url).ok()?;
            (saved_url == target_url).then(|| saved.fingerprint.clone()).flatten()
        });
        let (url, fingerprint) = normalize_endpoint(&raw_url, fp.as_deref())?;
        return Ok(Endpoint { url, fingerprint, library_id: None, device_id: None });
    }
    if explicit_fp.is_some() {
        return Err("SPKI --fingerprint requires an HTTPS --url (or GALROON_URL)".into());
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
    let device_id = session["device_id"].as_str().map(str::to_owned);
    Ok(Endpoint { url, fingerprint: None, library_id, device_id })
}

#[cfg(not(windows))]
async fn discover() -> R<Endpoint> {
    Err("Automatic Core discovery is Windows-only. Pass --url http://127.0.0.1:<port> (or --url https://<lan-host>:<port> --fingerprint <SPKI>) or set GALROON_URL".into())
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
    load_credential_with(&Options::default())
}

pub fn load_credential_with(options: &Options) -> R<Credential> {
    let library_override = options.library_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).or_else(|| non_empty_env("GALROON_LIBRARY"));
    let device_override = options.device_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).or_else(|| non_empty_env("GALROON_DEVICE"));
    if let Some(token) = options.token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Credential {
            token: token.to_owned(),
            library_id: library_override.unwrap_or_default(),
            device_name: String::new(),
            expires: 0,
            url: None,
            fingerprint: None,
            device_id: device_override,
        });
    }
    if let Some(var) = options.token_env.as_deref() {
        let token = std::env::var(var).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).ok_or_else(|| format!("Token environment variable {var} is not set or empty"))?;
        return Ok(Credential {
            token,
            library_id: library_override.unwrap_or_default(),
            device_name: String::new(),
            expires: 0,
            url: None,
            fingerprint: None,
            device_id: device_override,
        });
    }
    if let Some(token) = non_empty_env("GALROON_TOKEN") {
        return Ok(Credential {
            token,
            library_id: library_override.unwrap_or_default(),
            device_name: String::new(),
            expires: 0,
            url: None,
            fingerprint: None,
            device_id: device_override,
        });
    }
    let path = credential_path()?;
    let bytes = std::fs::read(&path).map_err(|_| format!("Not paired yet. Create a pairing code in Galroon (Settings > Access) and run: galroon-mcp pair <CODE>  [{}]", path.display()))?;
    let mut credential: Credential = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid credential file {}: {e}", path.display()))?;
    if let Some(lib) = library_override {
        credential.library_id = lib;
    }
    if let Some(dev) = device_override {
        credential.device_id = Some(dev);
    }
    Ok(credential)
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
    fn lan_endpoints_require_https_private_host_and_valid_spki_pin() {
        let fp = galroon_core::lan_tls::format_spki_fingerprint(&[7u8; 32]);
        assert_eq!(normalize_endpoint("https://192.168.50.154:14831/", Some(&fp)).unwrap(), ("https://192.168.50.154:14831".into(), Some(fp.clone())));
        assert_eq!(normalize_endpoint("https://nas.local:14830", Some(&fp)).unwrap(), ("https://nas.local:14830".into(), Some(fp.clone())));
        assert_eq!(normalize_endpoint("https://[fd00::1]:14830", Some(&fp)).unwrap(), ("https://[fd00::1]:14830".into(), Some(fp.clone())));
        // HTTPS without pin or with invalid pin is rejected.
        assert!(normalize_endpoint("https://192.168.50.154:14831", None).is_err());
        assert!(normalize_endpoint("https://192.168.50.154:14831", Some("invalid")).is_err());
        // Public/loopback IP literals and paths/credentials are rejected for LAN HTTPS.
        for bad in ["https://8.8.8.8:14830", "https://127.0.0.1:14830", "https://user@192.168.50.154:14830", "https://192.168.50.154:14830/api"] {
            assert!(normalize_endpoint(bad, Some(&fp)).is_err(), "{bad}");
        }
        // Loopback HTTP must reject SPKI pin and accept plain loopback without pin.
        assert!(normalize_endpoint("http://127.0.0.1:51234", Some(&fp)).is_err());
        assert_eq!(normalize_endpoint("http://127.0.0.1:51234", None).unwrap(), ("http://127.0.0.1:51234".into(), None));
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

use std::path::Path;
use super::{Error,Result};
// Credential IDs are random per connection and never included in catalog backups.
#[cfg(any(windows,target_os="macos"))]
fn entry(id:&str)->Result<keyring::Entry>{keyring::Entry::new("Galroon.VNDB",id).map_err(|_|Error("System credential storage is unavailable".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn write(_:&Path,id:&str,token:&str)->Result<()>{entry(id)?.set_password(token).map_err(|_|Error("Cannot save VNDB token in system credential storage".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn read(_:&Path,id:&str)->Result<String>{entry(id)?.get_password().map_err(|_|Error("VNDB token is unavailable. Reconnect your account.".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn delete(_:&Path,id:&str)->Result<()>{match entry(id)?.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(Error("Cannot remove VNDB token from system credential storage".into()))}}
// Without a system store (Linux/NAS Core), tokens live in owner-only files under device_dir.
#[cfg(not(any(windows,target_os="macos")))]
pub fn write(dir:&Path,id:&str,token:&str)->Result<()>{crate::secret_files::write(dir,"vndb",id,token).map_err(|_|Error("Cannot save VNDB token in Core credential storage".into()))}
#[cfg(not(any(windows,target_os="macos")))]
pub fn read(dir:&Path,id:&str)->Result<String>{crate::secret_files::read(dir,"vndb",id).map_err(|_|Error("VNDB token is unavailable. Reconnect your account.".into()))}
#[cfg(not(any(windows,target_os="macos")))]
pub fn delete(dir:&Path,id:&str)->Result<()>{crate::secret_files::delete(dir,"vndb",id).map_err(|_|Error("Cannot remove VNDB token from Core credential storage".into()))}

use super::{Error,Result};
// Credential IDs are random per connection and never included in catalog backups.
#[cfg(any(windows,target_os="macos"))]
fn entry(id:&str)->Result<keyring::Entry>{keyring::Entry::new("Galroon.VNDB",id).map_err(|_|Error("System credential storage is unavailable".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn write(id:&str,token:&str)->Result<()>{entry(id)?.set_password(token).map_err(|_|Error("Cannot save VNDB token in system credential storage".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn read(id:&str)->Result<String>{entry(id)?.get_password().map_err(|_|Error("VNDB token is unavailable. Reconnect your account.".into()))}
#[cfg(any(windows,target_os="macos"))]
pub fn delete(id:&str)->Result<()>{match entry(id)?.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(Error("Cannot remove VNDB token from system credential storage".into()))}}
#[cfg(not(any(windows,target_os="macos")))]
pub fn write(_: &str,_:&str)->Result<()>{Err(Error("VNDB credential storage currently requires Windows or macOS".into()))}
#[cfg(not(any(windows,target_os="macos")))]
pub fn read(_: &str)->Result<String>{Err(Error("VNDB credential storage currently requires Windows or macOS".into()))}
#[cfg(not(any(windows,target_os="macos")))]
pub fn delete(_: &str)->Result<()>{Ok(())}

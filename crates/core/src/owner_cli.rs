//! Unix entry arguments and owner operations. Passwords only enter through stdin.
use std::{ffi::OsString,path::PathBuf,io::{BufRead,Read}};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use axum::{extract::{State,Path},http::{HeaderMap,header},Json};
type R<T>=Result<T,String>;
#[derive(Serialize,Deserialize)]#[serde(tag="command",rename_all="lowercase",deny_unknown_fields)]
pub(crate) enum OwnerCommand{Setup{password:String},Pair{name:String,role:String},Devices,Revoke{id:String},PairingCodes,RevokePairingCode{id:String},NetworkShow,NetworkSet{settings:crate::network::NetworkEdit},Session}
pub enum Command{Serve{dir:PathBuf,device_dir:PathBuf,control_dir:PathBuf,port:u16},Owner{dir:PathBuf,control_dir:PathBuf,action:OwnerAction}}
pub struct OwnerAction(OwnerCommand);
const USAGE:&str="Usage: galroon-core serve --data-dir <dir> [--device-dir <dir>] [--control-dir <dir> (default <data-dir>/control)] [--loopback-port <1-65535>] | galroon-core owner <setup|pair --name <name> [--role desktop|web]|pairing-codes [revoke <id>]|network show|network set --address <IP> [--port 14830] [--hostname <name>]... [--enable|--disable]|devices|revoke <id>|session> [--data-dir <dir> (default /data)] [--control-dir <dir> (default <data-dir>/control)]";
pub fn parse(args:impl IntoIterator<Item=OsString>)->R<Command>{
 let mut args=args.into_iter();let mode=args.next().ok_or(USAGE)?;let mut rest:Vec<OsString>=args.collect();
 let mut dir=None;let mut device_dir=None;let mut control_dir=None;let mut port=None;let mut name=None;let mut role=None;let mut address=None;let mut network_port=None;let mut hostnames=Vec::new();let mut enabled=None;let mut positional=Vec::new();let mut i=0;
 while i<rest.len(){
  if mode=="owner"{match rest[i].to_str(){
   Some("--hostname")=>{i+=1;hostnames.push(rest.get(i).and_then(|v|v.to_str()).ok_or(USAGE)?.to_owned());i+=1;continue;},
   Some(flag @ ("--enable"|"--disable"))=>{if enabled.replace(flag=="--enable").is_some(){return Err(USAGE.into());}i+=1;continue;},_=>{}
  }}
  let slot=match rest[i].to_str(){Some("--data-dir")=>Some(&mut dir),Some("--control-dir")=>Some(&mut control_dir),Some("--device-dir")if mode=="serve"=>Some(&mut device_dir),Some("--loopback-port")if mode=="serve"=>Some(&mut port),Some("--address")if mode=="owner"=>Some(&mut address),Some("--port")if mode=="owner"=>Some(&mut network_port),Some("--name")if mode=="owner"=>Some(&mut name),Some("--role")if mode=="owner"=>Some(&mut role),_=>None};
  if let Some(slot)=slot{if slot.is_some()||i+1==rest.len(){return Err(USAGE.into());}*slot=Some(std::mem::take(&mut rest[i+1]));i+=2;}else{if rest[i].to_str().is_some_and(|v|v.starts_with('-')){return Err(USAGE.into());}positional.push(std::mem::take(&mut rest[i]));i+=1;}
 }
 if mode=="serve"{
  if !positional.is_empty(){return Err(USAGE.into());}let dir=PathBuf::from(dir.ok_or(USAGE)?);let device_dir=device_dir.map(PathBuf::from).unwrap_or_else(||dir.clone());
  let port=match port{Some(v)=>v.to_str().and_then(|s|s.parse::<u16>().ok()).filter(|p|*p!=0).ok_or("Loopback port must be between 1 and 65535")?,None=>14800};
  let control_dir=control_dir.map(PathBuf::from).unwrap_or_else(||dir.join("control"));
  return Ok(Command::Serve{dir,device_dir,control_dir,port});
 }
 if mode!="owner"{return Err(USAGE.into());}
 let verb=positional.first().and_then(|v|v.to_str()).ok_or(USAGE)?;
 if verb!="pair"&&(name.is_some()||role.is_some()){return Err(USAGE.into());}
 if !(verb=="network"&&positional.get(1).is_some_and(|v|v=="set"))&&(address.is_some()||network_port.is_some()||!hostnames.is_empty()||enabled.is_some()){return Err(USAGE.into());}
 let action=match (verb,positional.len()){
  ("setup",1)=>OwnerCommand::Setup{password:String::new()},("devices",1)=>OwnerCommand::Devices,("session",1)=>OwnerCommand::Session,
  ("revoke",2)=>OwnerCommand::Revoke{id:positional[1].to_str().ok_or("Invalid device id")?.into()},
  ("pair",1)=>{let name=name.and_then(|n|n.into_string().ok()).ok_or("Pair requires --name")?;let role=match role{Some(r)=>r.into_string().map_err(|_|"Invalid role")?,None=>"desktop".into()};if !matches!(role.as_str(),"desktop"|"web"){return Err("Role must be desktop or web".into());}OwnerCommand::Pair{name,role}},
  ("pairing-codes",1)=>OwnerCommand::PairingCodes,
  ("pairing-codes",3)if positional[1]=="revoke"=>OwnerCommand::RevokePairingCode{id:positional[2].to_str().ok_or("Invalid pairing code id")?.into()},
  ("network",2)if positional[1]=="show"=>OwnerCommand::NetworkShow,
  ("network",2)if positional[1]=="set"=>{let address=address.and_then(|v|v.into_string().ok()).ok_or("Network set requires --address")?.parse().map_err(|_|"Invalid IP address")?;let port=match network_port{Some(v)=>v.to_str().and_then(|v|v.parse::<u16>().ok()).filter(|v|*v!=0).ok_or("Invalid network port")?,None=>14830};OwnerCommand::NetworkSet{settings:crate::network::NetworkEdit{address,port,hostnames,enabled}}},
  _=>return Err(USAGE.into())
 };
 let dir=dir.map(PathBuf::from).unwrap_or_else(||PathBuf::from("/data"));let control_dir=control_dir.map(PathBuf::from).unwrap_or_else(||dir.join("control"));
 Ok(Command::Owner{dir,control_dir,action:OwnerAction(action)})
}
fn action_line(action:OwnerAction,input:&mut impl BufRead)->R<String>{
 let mut command=action.0;
 if let OwnerCommand::Setup{password}= &mut command{
  let mut bytes=Vec::new();input.take(1027).read_until(b'\n',&mut bytes).map_err(|_|"Could not read password from stdin")?;
  if bytes.last()==Some(&b'\n'){bytes.pop();if bytes.last()==Some(&b'\r'){bytes.pop();}}
  if bytes.len()>1024{return Err("Password exceeds 1024 bytes".into());}*password=String::from_utf8(bytes).map_err(|_|"Password must be UTF-8")?;
 }
 if matches!(command,OwnerCommand::Session){return Ok("session".into());}
 serde_json::to_string(&command).map_err(|_|"Could not encode owner command".into())
}
pub async fn run(command:Command)->R<()>{
 match command{
  Command::Serve{dir,device_dir,control_dir,port}=>crate::unix_core::run(dir,device_dir,control_dir,port).await,
  Command::Owner{control_dir,action,..}=>{
   let line=action_line(action,&mut std::io::stdin().lock())?;
   let value=crate::unix_core::request(&control_dir,&line).await?;
   // Owner results (including session tokens and pairing codes) go only to stdout.
   println!("{value}");Ok(())
  }
 }
}
pub(crate) async fn execute(app:&crate::App,command:OwnerCommand)->R<Value>{
 let mut headers=HeaderMap::new();headers.insert(header::AUTHORIZATION,format!("Bearer {}",app.token).parse().map_err(|_|"Invalid owner identity")?);
 // Reuse the existing owner handlers, without sending secrets through HTTP or diagnostics.
 match command{
  OwnerCommand::Setup{password}=>{let db=app.db.clone();tokio::task::spawn_blocking(move||crate::access::setup(&db,&password,false)).await.map_err(|_|"Owner setup failed")??;Ok(json!({"configured":true}))},
  OwnerCommand::Pair{name,role}=>{if !matches!(role.as_str(),"desktop"|"web"){return Err("Role must be desktop or web".into());}crate::access::pairing(State(app.clone()),headers,Json(json!({"name":name,"role":role}))).await.map(|Json(v)|v).map_err(|e|e.0)},
  OwnerCommand::Devices=>crate::access::devices(State(app.clone()),headers).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::Revoke{id}=>crate::access::revoke(State(app.clone()),headers,Path(id)).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::PairingCodes=>crate::access::pairing_codes(State(app.clone()),headers).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::RevokePairingCode{id}=>crate::access::revoke_pairing_code(State(app.clone()),headers,Path(id)).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::NetworkShow=>crate::access::network_show(State(app.clone()),headers).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::NetworkSet{settings}=>crate::access::network_set(State(app.clone()),headers,Json(settings)).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::Session=>Err("Use the session control command".into())
 }
}

#[cfg(test)]mod tests{
 use super::*;
 fn parsed(args:&[&str])->R<Command>{parse(args.iter().map(OsString::from))}
 fn line(args:&[&str],stdin:&str)->String{let Command::Owner{action,..}=parsed(args).unwrap()else{panic!("owner expected")};action_line(action,&mut std::io::Cursor::new(stdin)).unwrap()}
 #[test]fn serve_arguments_and_owner_scope(){
  let Command::Serve{dir,device_dir,control_dir,port}=parsed(&["serve","--data-dir","collection"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("collection"));assert_eq!(device_dir,dir);assert_eq!(control_dir,dir.join("control"));assert_eq!(port,14800);
  let Command::Serve{device_dir,port,..}=parsed(&["serve","--data-dir","collection","--device-dir","device","--loopback-port","14801"]).unwrap()else{panic!()};assert_eq!(device_dir,PathBuf::from("device"));assert_eq!(port,14801);
  for args in [vec!["serve","--data-dir","d","--loopback-port","0"],vec!["serve","--data-dir","d","--loopback-port","65536"],vec!["owner","pair","--name","n","--role","owner"],vec!["owner","setup","password-in-argv"],vec!["owner","setup","--password","secret"],vec!["owner","session","--name","n"],vec!["restore"],vec!["serve"]]{assert!(parsed(&args).is_err(),"{args:?}");}
  let Command::Owner{dir,control_dir,..}=parsed(&["owner","session"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("/data"));assert_eq!(control_dir,dir.join("control"));
  let Command::Owner{dir,control_dir,..}=parsed(&["owner","--data-dir","custom","session"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("custom"));assert_eq!(control_dir,dir.join("control"));
 }
 #[test]fn control_directory_arguments_for_serve_and_every_owner_command(){
  let Command::Serve{dir,control_dir,..}=parsed(&["serve","--control-dir","runtime","--data-dir","collection"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("collection"));assert_eq!(control_dir,PathBuf::from("runtime"));
  for verb in [vec!["setup"],vec!["pair","--name","desktop"],vec!["devices"],vec!["revoke","device-id"],vec!["session"]]{
   let mut args=vec!["owner","--control-dir","runtime"];args.extend(verb);args.extend(["--data-dir","collection"]);
   let Command::Owner{dir,control_dir,..}=parsed(&args).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("collection"));assert_eq!(control_dir,PathBuf::from("runtime"));
  }
  for args in [vec!["serve","--data-dir","d","--control-dir"],vec!["owner","session","--control-dir"],vec!["serve","--data-dir","d","--control-dir","a","--control-dir","b"],vec!["owner","session","--control-dir","a","--control-dir","b"]]{assert!(parsed(&args).is_err(),"{args:?}");}
 }
 #[test]fn setup_reads_password_only_from_bounded_stdin(){
  let value:Value=serde_json::from_str(&line(&["owner","setup"]," leading spaces stay \r\n")).unwrap();assert_eq!(value["password"]," leading spaces stay ");
  assert_eq!(line(&["owner","session"],"ignored"),"session");
  let Command::Owner{action,..}=parsed(&["owner","setup"]).unwrap()else{panic!()};assert!(action_line(action,&mut std::io::Cursor::new("p".repeat(1025))).is_err());
 }
 #[tokio::test]async fn setup_pair_redeem_devices_revoke_use_existing_access_logic(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("data")).unwrap();
  let setup:OwnerCommand=serde_json::from_str(&line(&["owner","setup"],"fixture owner password\n")).unwrap();assert_eq!(execute(&app,setup).await.unwrap()["configured"],true);
  let pairing:OwnerCommand=serde_json::from_str(&line(&["owner","pair","--name","Desktop fixture","--role","desktop"],"")).unwrap();let code=execute(&app,pairing).await.unwrap();
  let response=crate::access::pair(State(app.clone()),Json(json!({"code":code["code"]}))).await.map_err(|e|e.0).unwrap();let bytes=axum::body::to_bytes(response.into_body(),4096).await.unwrap();let paired:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(paired["role"],"desktop");
  assert!(crate::access::pair(State(app.clone()),Json(json!({"code":code["code"]}))).await.is_err());
  let devices=execute(&app,OwnerCommand::Devices).await.unwrap();assert_eq!(devices[0]["id"],paired["id"]);assert_eq!(devices[0]["name"],"Desktop fixture");
  let mut headers=HeaderMap::new();headers.insert(header::AUTHORIZATION,format!("Bearer {}",paired["token"].as_str().unwrap()).parse().unwrap());assert!(crate::access::authenticate(&app,&headers).is_ok());
  execute(&app,OwnerCommand::Revoke{id:paired["id"].as_str().unwrap().into()}).await.unwrap();assert!(crate::access::authenticate(&app,&headers).is_err());
 }
}

#[cfg(test)]mod pairing_network_tests{
 use super::*;
 async fn command(app:&crate::App,args:&[&str])->R<Value>{let Command::Owner{action,..}=parse(args.iter().map(OsString::from))?else{panic!()};let line=action_line(action,&mut std::io::Cursor::new(""))?;execute(app,serde_json::from_str(&line).unwrap()).await}
 #[tokio::test]async fn web_pair_network_settings_and_pending_code_revoke(){
  let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("data")).unwrap();crate::access::setup(&app.db,"Owner CLI fixture password",false).unwrap();
  let shown=command(&app,&["owner","network","show"]).await.unwrap();assert_eq!(shown["status"],"not configured");assert!(shown.get("spki_fingerprint").is_none());
  let path=app.device_dir.join("network.json");assert!(command(&app,&["owner","network","set","--address","8.8.8.8","--enable"]).await.is_err());assert!(!path.exists());
  let result=command(&app,&["owner","network","set","--address","192.168.1.20","--hostname","nas.local","--hostname","nas","--enable"]).await.unwrap();assert_eq!(result["restart_required"],true);assert!(result["message"].as_str().unwrap().contains("Core restart"));assert!(app.network.is_none());let before=std::fs::read(&path).unwrap();
  assert!(command(&app,&["owner","network","set","--address","127.0.0.1"]).await.is_err());assert_eq!(std::fs::read(&path).unwrap(),before);
  // An open file continues to read the old inode after the replacement, proving rename rather than truncation.
  let mut old=std::fs::File::open(&path).unwrap();command(&app,&["owner","network","set","--address","192.168.1.21","--port","14831"]).await.unwrap();let mut old_bytes=Vec::new();old.read_to_end(&mut old_bytes).unwrap();assert_eq!(old_bytes,before);assert_ne!(std::fs::read(&path).unwrap(),before);
  assert!(!std::fs::read_dir(&app.device_dir).unwrap().any(|e|e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
  for role in ["web","desktop"]{let code=command(&app,&["owner","pair","--name","CLI fixture","--role",role]).await.unwrap();assert_eq!(code["role"],role);let pairing=crate::pairing::parse_pairing_string(code["pairing_string"].as_str().unwrap()).unwrap();assert_eq!(pairing.host,"192.168.1.21");assert_eq!(pairing.port,14831);let shown=command(&app,&["owner","network","show"]).await.unwrap();assert_eq!(shown["spki_fingerprint"],pairing.fingerprint);assert_eq!(shown["certificate_sha256"],code["certificate_sha256"]);assert_eq!(shown["settings"]["enabled"],true);
   let list=command(&app,&["owner","pairing-codes"]).await.unwrap();assert_eq!(list[0]["id"],code["id"]);assert!(list[0].get("code").is_none());assert!(list[0].get("hash").is_none());command(&app,&["owner","pairing-codes","revoke",code["id"].as_str().unwrap()]).await.unwrap();assert_eq!(command(&app,&["owner","pairing-codes"]).await.unwrap(),json!([]));
  }
  command(&app,&["owner","network","set","--address","192.168.1.21","--disable"]).await.unwrap();let code=command(&app,&["owner","pair","--name","Disabled LAN","--role","web"]).await.unwrap();assert!(code.get("pairing_string").is_none());assert!(code.get("certificate_sha256").is_none());let shown=command(&app,&["owner","network","show"]).await.unwrap();assert_eq!(shown["settings"]["enabled"],false);assert!(shown["spki_fingerprint"].is_string());
 }
 #[test]fn invalid_network_cli_shapes_are_rejected(){
  for args in [vec!["owner","network","set"],vec!["owner","network","set","--address","not-ip"],vec!["owner","network","set","--address","192.168.1.1","--enable","--disable"],vec!["owner","network","show","--enable"],vec!["owner","pairing-codes","revoke"],vec!["owner","pairing-codes","--hostname","nas"]]{assert!(parse(args.iter().map(OsString::from)).is_err(),"{args:?}");}
 }
}

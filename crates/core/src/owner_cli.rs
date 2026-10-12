//! Unix entry arguments and owner operations. Passwords only enter through stdin.
use std::{ffi::OsString,path::PathBuf,io::{BufRead,Read}};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use axum::{extract::{State,Path},http::{HeaderMap,header},Json};
type R<T>=Result<T,String>;
#[derive(Serialize,Deserialize)]#[serde(tag="command",rename_all="lowercase",deny_unknown_fields)]
pub(crate) enum OwnerCommand{Setup{password:String},Pair{name:String,role:String},Devices,Revoke{id:String},Session}
pub enum Command{Serve{dir:PathBuf,device_dir:PathBuf,port:u16},Owner{dir:PathBuf,action:OwnerAction}}
pub struct OwnerAction(OwnerCommand);
const USAGE:&str="Usage: galroon-core serve --data-dir <dir> [--device-dir <dir>] [--loopback-port <1-65535>] | galroon-core owner <setup|pair --name <name> [--role desktop]|devices|revoke <id>|session> [--data-dir <dir> (default /data)]";
pub fn parse(args:impl IntoIterator<Item=OsString>)->R<Command>{
 let mut args=args.into_iter();let mode=args.next().ok_or(USAGE)?;let mut rest:Vec<OsString>=args.collect();
 let mut dir=None;let mut device_dir=None;let mut port=None;let mut name=None;let mut role=None;let mut positional=Vec::new();let mut i=0;
 while i<rest.len(){
  let slot=match rest[i].to_str(){Some("--data-dir")=>Some(&mut dir),Some("--device-dir")if mode=="serve"=>Some(&mut device_dir),Some("--loopback-port")if mode=="serve"=>Some(&mut port),Some("--name")if mode=="owner"=>Some(&mut name),Some("--role")if mode=="owner"=>Some(&mut role),_=>None};
  if let Some(slot)=slot{if slot.is_some()||i+1==rest.len(){return Err(USAGE.into());}*slot=Some(std::mem::take(&mut rest[i+1]));i+=2;}else{if rest[i].to_str().is_some_and(|v|v.starts_with('-')){return Err(USAGE.into());}positional.push(std::mem::take(&mut rest[i]));i+=1;}
 }
 if mode=="serve"{
  if !positional.is_empty(){return Err(USAGE.into());}let dir=PathBuf::from(dir.ok_or(USAGE)?);let device_dir=device_dir.map(PathBuf::from).unwrap_or_else(||dir.clone());
  let port=match port{Some(v)=>v.to_str().and_then(|s|s.parse::<u16>().ok()).filter(|p|*p!=0).ok_or("Loopback port must be between 1 and 65535")?,None=>14800};
  return Ok(Command::Serve{dir,device_dir,port});
 }
 if mode!="owner"{return Err(USAGE.into());}
 let verb=positional.first().and_then(|v|v.to_str()).ok_or(USAGE)?;
 if verb!="pair"&&(name.is_some()||role.is_some()){return Err(USAGE.into());}
 let action=match (verb,positional.len()){
  ("setup",1)=>OwnerCommand::Setup{password:String::new()},("devices",1)=>OwnerCommand::Devices,("session",1)=>OwnerCommand::Session,
  ("revoke",2)=>OwnerCommand::Revoke{id:positional[1].to_str().ok_or("Invalid device id")?.into()},
  ("pair",1)=>{let name=name.and_then(|n|n.into_string().ok()).ok_or("Pair requires --name")?;let role=match role{Some(r)=>r.into_string().map_err(|_|"Invalid role")?,None=>"desktop".into()};if role!="desktop"{return Err("Only the desktop role is supported".into());}OwnerCommand::Pair{name,role}},
  _=>return Err(USAGE.into())
 };
 Ok(Command::Owner{dir:dir.map(PathBuf::from).unwrap_or_else(||PathBuf::from("/data")),action:OwnerAction(action)})
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
  Command::Serve{dir,device_dir,port}=>crate::unix_core::run(dir,device_dir,port).await,
  Command::Owner{dir,action}=>{
   let line=action_line(action,&mut std::io::stdin().lock())?;
   let value=crate::unix_core::request(&dir,&line).await?;
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
  OwnerCommand::Pair{name,role}=>{if role!="desktop"{return Err("Only the desktop role is supported".into());}crate::access::pairing(State(app.clone()),headers,Json(json!({"name":name}))).await.map(|Json(v)|v).map_err(|e|e.0)},
  OwnerCommand::Devices=>crate::access::devices(State(app.clone()),headers).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::Revoke{id}=>crate::access::revoke(State(app.clone()),headers,Path(id)).await.map(|Json(v)|v).map_err(|e|e.0),
  OwnerCommand::Session=>Err("Use the session control command".into())
 }
}

#[cfg(test)]mod tests{
 use super::*;
 fn parsed(args:&[&str])->R<Command>{parse(args.iter().map(OsString::from))}
 fn line(args:&[&str],stdin:&str)->String{let Command::Owner{action,..}=parsed(args).unwrap()else{panic!("owner expected")};action_line(action,&mut std::io::Cursor::new(stdin)).unwrap()}
 #[test]fn serve_arguments_and_owner_scope(){
  let Command::Serve{dir,device_dir,port}=parsed(&["serve","--data-dir","collection"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("collection"));assert_eq!(device_dir,dir);assert_eq!(port,14800);
  let Command::Serve{device_dir,port,..}=parsed(&["serve","--data-dir","collection","--device-dir","device","--loopback-port","14801"]).unwrap()else{panic!()};assert_eq!(device_dir,PathBuf::from("device"));assert_eq!(port,14801);
  for args in [vec!["serve","--data-dir","d","--loopback-port","0"],vec!["serve","--data-dir","d","--loopback-port","65536"],vec!["owner","pair","--name","n","--role","web"],vec!["owner","setup","password-in-argv"],vec!["owner","setup","--password","secret"],vec!["owner","session","--name","n"],vec!["restore"],vec!["serve"]]{assert!(parsed(&args).is_err(),"{args:?}");}
  let Command::Owner{dir,..}=parsed(&["owner","session"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("/data"));
  let Command::Owner{dir,..}=parsed(&["owner","--data-dir","custom","session"]).unwrap()else{panic!()};assert_eq!(dir,PathBuf::from("custom"));
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

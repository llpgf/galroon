//! Validate helper listings before any extraction writes occur.
use std::collections::BTreeMap;
type R<T>=Result<T,String>;
pub fn safe_path(path:&str)->R<()>{
 if path.is_empty()||path.chars().any(char::is_control)||path.starts_with(['\\','/'])||path.contains([':', '<','>','"','|','?','*']){return Err("Archive has an unsafe path".into());}
 for part in path.split(['\\','/']){
  if part.is_empty()||matches!(part,"."|"..")||part.trim_end_matches(['.',' '])!=part{return Err("Archive has an unsafe path".into());}
  let stem=part.split('.').next().unwrap_or("").to_uppercase();
  let device=matches!(stem.as_str(),"CON"|"PRN"|"AUX"|"NUL"|"CONIN$"|"CONOUT$")||["COM","LPT"].iter().any(|prefix|stem.strip_prefix(prefix).is_some_and(|s|matches!(s,"1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"|"¹"|"²"|"³")));
  if device{return Err("Archive uses a reserved Windows filename".into());}
 }Ok(())
}
pub fn total_bytes(values:impl IntoIterator<Item=u64>)->R<u64>{values.into_iter().try_fold(0u64,|sum,n|sum.checked_add(n).ok_or("Archive sizes exceed the supported range".into()))}
pub fn require_space(needed:u64,available:u64)->R<()>{if needed>available{return Err(format!("Insufficient destination space: need {needed} more bytes; {available} bytes are available."));}Ok(())}
pub fn parse(text:&str)->R<Vec<(String,u64)>>{
 let mut members=vec![];let mut paths=BTreeMap::<String,bool>::new();
 for block in text.split("\n\n"){
  let mut path=None;let mut size=None;let mut folder=false;
  for line in block.lines(){
   if let Some(v)=line.strip_prefix("Path = "){if path.replace(v).is_some(){return Err("Ambiguous archive path metadata".into());}}
   if let Some(v)=line.strip_prefix("Size = "){if size.replace(v).is_some(){return Err("Ambiguous archive size metadata".into());}}
   if line=="Folder = +"||line.starts_with("Attributes = D"){folder=true;}
   if ["Symbolic Link = ","Hard Link = ","Reparse = ","Reparse Point = "].iter().any(|prefix|line.strip_prefix(prefix).is_some_and(|v|!v.trim().is_empty()))||line.strip_prefix("Mode = ").is_some_and(|v|v.starts_with('l'))||line.strip_prefix("Attributes = ").is_some_and(|v|v.split_whitespace().any(|part|part.starts_with('l'))){return Err("Archive contains links; extraction requires manual review".into());}
  }
  if let Some(path)=path{
   safe_path(path)?;let key=path.replace('\\',"/").to_lowercase();
   if paths.contains_key(&key){return Err("Archive contains duplicate or case-conflicting paths".into());}
   let mut parent=key.as_str();while let Some((prefix,_))=parent.rsplit_once('/') {if paths.get(prefix)==Some(&false){return Err("Archive file conflicts with a directory".into());}parent=prefix;}
   if !folder&&paths.range(format!("{key}/")..).next().is_some_and(|(p,_)|p.starts_with(&format!("{key}/"))){return Err("Archive file conflicts with a directory".into());}
   paths.insert(key,folder);if !folder{let size=size.ok_or("Archive file size is missing")?;if size.is_empty()||!size.bytes().all(|b|b.is_ascii_digit()){return Err("Archive has an invalid file size".into());}let size=size.parse::<u64>().map_err(|_|"Archive file size exceeds the supported range")?;members.push((path.to_owned(),size));}
  }
 }
 if members.is_empty(){return Err("No extractable files were found".into());}total_bytes(members.iter().map(|(_,s)|*s))?;Ok(members)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn rejects_windows_devices_and_ambiguous_paths(){for p in ["CON","aux.txt","a/LPT9.bin","COM¹.txt","x/../y","x/./y","x//y","x\\","x ","x.","a:stream","a?b","C:\\x","\\\\server\\share"]{assert!(safe_path(p).is_err(),"{p}");}for p in ["日本語/音楽.bin","folder/data","COM10.txt","auxiliary.txt"]{assert!(safe_path(p).is_ok(),"{p}");}}
 #[test]fn rejects_duplicate_case_and_file_directory_collisions_in_both_orders(){for (a,b) in [("Data/File.bin","data/file.BIN"),("a","a/b"),("a/b","a"),("a\\b","a/b")]{assert!(parse(&format!("Path = {a}\nSize = 1\n\nPath = {b}\nSize = 2\n")).is_err());}assert!(parse("Path = a\nFolder = +\n\nPath = a/b\nSize = 2\n").is_ok());}
 #[test]fn iso_empty_link_and_directory_size_fields_are_not_links(){assert_eq!(parse("Path = folder\nFolder = +\nSize = \nSymbolic Link = \n\nPath = folder/file\nFolder = -\nSize = 0\nSymbolic Link = \n").unwrap(),vec![("folder/file".into(),0)]);for field in ["Symbolic Link = ../outside","Hard Link = target","Mode = lrwxrwxrwx","Attributes =  lrwxrwxrwx","Reparse Point = target"]{assert!(parse(&format!("Path = link\nSize = 0\n{field}\n")).is_err());}}
 #[test]fn sizes_are_required_checked_and_capacity_failures_are_explicit(){for size in ["", "bad", "-1", "18446744073709551616"]{assert!(parse(&format!("Path = file\nSize = {size}\n")).is_err());}assert!(parse("Path = file\n").is_err());assert!(parse("Path = file\nSize = 1\nSize = 2\n").is_err());assert!(parse(&format!("Path = a\nSize = {}\n\nPath = b\nSize = 1\n",u64::MAX)).is_err());assert_eq!(total_bytes([0,3,7]).unwrap(),10);assert!(require_space(11,10).unwrap_err().contains("need 11"));assert!(require_space(10,10).is_ok());assert_eq!(parse("Path = empty.txt\nSize = 0\n").unwrap(),vec![("empty.txt".into(),0)]);}
}

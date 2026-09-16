//! Filename evidence identifies candidate volume groups; only an archive test proves readability.
use serde::Serialize;
use std::{path::Path,collections::BTreeMap,sync::OnceLock};
use regex::Regex;
#[derive(Clone,Debug)]pub struct Part{pub key:String,pub entry:String,pub index:u32,pub numbered:bool}
pub fn part(name:&str)->Option<Part>{
 static SPLIT:OnceLock<Regex>=OnceLock::new();static RAR:OnceLock<Regex>=OnceLock::new();static OLD:OnceLock<Regex>=OnceLock::new();
 let lower=name.to_lowercase();
 if let Some(c)=SPLIT.get_or_init(||Regex::new(r"(?i)^(.*\.(?:7z|zip))\.(\d{3,6})$").unwrap()).captures(name){let base=c.get(1)?.as_str();let n=c.get(2)?.as_str();return Some(Part{key:format!("split:{}:{}",base.to_lowercase(),n.len()),entry:format!("{base}.{:0width$}",1,width=n.len()),index:n.parse().ok()?,numbered:true});}
 if let Some(c)=RAR.get_or_init(||Regex::new(r"(?i)^(.*)\.part(\d{1,6})\.rar$").unwrap()).captures(name){let base=c.get(1)?.as_str();let n=c.get(2)?.as_str();return Some(Part{key:format!("rar:{}:{}",base.to_lowercase(),n.len()),entry:format!("{base}.part{:0width$}.rar",1,width=n.len()),index:n.parse().ok()?,numbered:true});}
 if let Some(c)=OLD.get_or_init(||Regex::new(r"(?i)^(.*)\.r(\d{2})$").unwrap()).captures(name){let base=c.get(1)?.as_str();return Some(Part{key:format!("oldrar:{}",base.to_lowercase()),entry:format!("{base}.rar"),index:c.get(2)?.as_str().parse::<u32>().ok()?+2,numbered:true});}
 if lower.ends_with(".rar"){return Some(Part{key:format!("oldrar:{}",&lower[..lower.len()-4]),entry:name.into(),index:1,numbered:false});}None
}
#[derive(Serialize,Debug)]pub struct Group{pub entry:String,pub members:Vec<String>,pub missing:Vec<u32>,pub duplicate_indices:bool,pub requires_verification:bool}
pub fn groups(paths:&[String])->Vec<Group>{
 let mut groups:BTreeMap<(String,String),Vec<(String,Part)>>=BTreeMap::new();
 for p in paths{let path=Path::new(p);if let Some(part)=path.file_name().and_then(|s|s.to_str()).and_then(part){let parent=path.parent().unwrap_or(Path::new(""));groups.entry((parent.to_string_lossy().to_lowercase(),part.key.clone())).or_default().push((p.clone(),part));}}
 groups.into_values().filter(|g|g.iter().any(|(_,p)|p.numbered)).map(|mut group|{group.sort_by_key(|(_,p)|p.index);let first=&group[0];let entry=Path::new(&first.0).parent().unwrap_or(Path::new("")).join(&first.1.entry).to_string_lossy().into_owned();let indices:std::collections::BTreeSet<_>=group.iter().map(|(_,p)|p.index).collect();let max=indices.last().copied().unwrap_or(1);let missing=(1..=max).filter(|n|!indices.contains(n)).collect();Group{entry,members:group.iter().map(|(s,_)|s.clone()).collect(),missing,duplicate_indices:indices.len()!=group.len()||indices.contains(&0),requires_verification:true}}).collect()
}
pub fn check_known(paths:&[String])->Result<Vec<Group>,String>{let groups=groups(paths);for g in &groups{if !g.missing.is_empty(){return Err(format!("Missing archive volumes for {}: {:?}",g.entry,g.missing));}if g.duplicate_indices{return Err(format!("Ambiguous archive volumes for {}",g.entry));}}Ok(groups)}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn grouping_requires_first_volume_and_no_gaps(){let paths=vec!["game.7z.001".into(),"game.7z.003".into()];assert!(check_known(&paths).is_err());let paths=vec!["game.part02.rar".into()];assert!(check_known(&paths).is_err());assert_eq!(check_known(&["a.rar".into(),"a.r00".into()]).unwrap()[0].members.len(),2);assert_eq!(groups(&["a.rar".into()]).len(),0);assert_eq!(part("game.part002.rar").unwrap().entry,"game.part001.rar");}
 #[test]fn same_names_in_other_folders_are_not_combined(){let groups=groups(&["a/game.7z.001".into(),"b/game.7z.002".into()]);assert_eq!(groups.len(),2);assert_eq!(groups[1].missing,vec![1]);}
}

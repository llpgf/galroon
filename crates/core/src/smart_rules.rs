//! Typed smart-list predicates. Unknown and incomplete metadata never imply absence.
use serde::{Deserialize,Serialize};
use std::collections::{BTreeMap,BTreeSet};
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
#[serde(rename_all="snake_case")]
pub enum Field{PersonalTags,RelatedPersonalTags,SourceTags,Brands,People,PersonRoles,Characters,Status,Favorite,SourceLocations,MatchingState,OrganizingState}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
#[serde(rename_all="snake_case")]
pub enum EditionField{ConfirmedLanguages,Platforms,Availability}
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]#[serde(rename_all="snake_case")]
pub enum Join{All,Any}
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]#[serde(rename_all="snake_case")]
pub enum SetMode{Any,All,None}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Predicate<F>{pub field:F,pub mode:SetMode,pub values:Vec<String>,#[serde(default)]pub exclude:bool,#[serde(default)]pub include_unknown:bool}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Rule{
 Group{join:Join,children:Vec<Rule>},
 Field{predicate:Predicate<Field>},
 Year{from:u16,to:u16,#[serde(default)]exclude:bool,#[serde(default)]include_unknown:bool},
 Edition{join:Join,conditions:Vec<Predicate<EditionField>>,#[serde(default)]exclude:bool,#[serde(default)]include_unknown:bool},
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Truth{Yes,No,Unknown}
impl Truth{fn inverted(self)->Self{match self{Self::Yes=>Self::No,Self::No=>Self::Yes,Self::Unknown=>Self::Unknown}}}
#[derive(Default,Serialize)]pub struct KnownSet{pub values:BTreeSet<String>,pub complete:bool}
#[derive(Default,Serialize)]pub struct EditionFacts{pub fields:BTreeMap<EditionField,KnownSet>}
#[derive(Default,Serialize)]pub struct WorkFacts{pub valid_until:Option<i64>,pub fields:BTreeMap<Field,KnownSet>,pub year:Option<u16>,pub editions:Vec<EditionFacts>,pub editions_complete:bool}
fn combine(join:Join,values:impl IntoIterator<Item=Truth>)->Truth{
 let mut unknown=false;for value in values{match (join,value){(Join::All,Truth::No)=>return Truth::No,(Join::Any,Truth::Yes)=>return Truth::Yes,(_,Truth::Unknown)=>unknown=true,_=>{}}}
 if unknown{Truth::Unknown}else{match join{Join::All=>Truth::Yes,Join::Any=>Truth::No}}
}
fn policy(value:Truth,exclude:bool,include_unknown:bool)->Truth{let value=if exclude{value.inverted()}else{value};if value==Truth::Unknown&&include_unknown{Truth::Yes}else{value}}
fn predicate<F>(condition:&Predicate<F>,known:Option<&KnownSet>)->Truth{
 let value=match known{None=>Truth::Unknown,Some(known)=>{
 let any=condition.values.iter().any(|v|known.values.contains(v));let all=condition.values.iter().all(|v|known.values.contains(v));
 match condition.mode{
 SetMode::Any=>if any{Truth::Yes}else if known.complete{Truth::No}else{Truth::Unknown},
 SetMode::All=>if all{Truth::Yes}else if known.complete{Truth::No}else{Truth::Unknown},
 SetMode::None=>if any{Truth::No}else if known.complete{Truth::Yes}else{Truth::Unknown},
 }}};policy(value,condition.exclude,condition.include_unknown)
}
pub fn evaluate_checked(rule:&Rule,work:&WorkFacts)->Result<Truth,String>{validate(rule)?;Ok(evaluate(rule,work))}
fn evaluate(rule:&Rule,work:&WorkFacts)->Truth{match rule{
 Rule::Group{join,children}=>combine(*join,children.iter().map(|rule|evaluate(rule,work))),
 Rule::Field{predicate:p}=>predicate(p,work.fields.get(&p.field)),
 Rule::Year{from,to,exclude,include_unknown}=>policy(work.year.map(|y|if y>=*from&&y<=*to{Truth::Yes}else{Truth::No}).unwrap_or(Truth::Unknown),*exclude,*include_unknown),
 Rule::Edition{join,conditions,exclude,include_unknown}=>{
 // Existential conditions share one edition; never combine language from A with availability from B.
 let mut values=work.editions.iter().map(|edition|combine(*join,conditions.iter().map(|p|predicate(p,edition.fields.get(&p.field))))).collect::<Vec<_>>();
 if !work.editions_complete{values.push(Truth::Unknown);}
 policy(combine(Join::Any,values),*exclude,*include_unknown)
 }
}}
fn validate_predicate<F>(p:&Predicate<F>)->Result<(),String>{
 if p.values.is_empty()||p.values.len()>100||p.values.iter().any(|v|v.is_empty()||v.len()>256||v.chars().any(char::is_control)){return Err("Select 1–100 bounded values per condition".into());}
 if p.values.iter().collect::<BTreeSet<_>>().len()!=p.values.len(){return Err("Duplicate condition values".into());}Ok(())
}
pub fn validate(rule:&Rule)->Result<(),String>{
 fn visit(rule:&Rule,depth:usize,count:&mut usize)->Result<(),String>{*count+=1;if *count>64{return Err("A rule can contain at most 64 conditions and groups".into());}
 match rule{
 Rule::Group{children,..}=>{if depth>=2||children.is_empty(){return Err("Use nonempty groups, at most two levels".into());}for child in children{visit(child,depth+1,count)?;}},
 Rule::Field{predicate}=>{validate_predicate(predicate)?;if predicate.field==Field::Favorite&&predicate.values.iter().any(|v|!matches!(v.as_str(),"true"|"false")){return Err("Favorite accepts only true or false".into());}if predicate.field==Field::OrganizingState&&predicate.values.iter().any(|v|!matches!(v.as_str(),"organized"|"unorganized"|"pending"|"partial")){return Err("Select a supported organizing state".into());}},
 Rule::Year{from,to,..}=>if *from<1||from>to||*to>9999{return Err("Invalid year range".into());},
 Rule::Edition{conditions,..}=>{if conditions.is_empty(){return Err("Edition conditions required".into());}for p in conditions{*count+=1;if *count>64{return Err("Too many conditions".into());}validate_predicate(p)?;}},
 }Ok(())}
 visit(rule,0,&mut 0)
}
#[cfg(test)]mod tests{
 use super::*;
 fn set(values:&[&str],complete:bool)->KnownSet{KnownSet{values:values.iter().map(|v|v.to_string()).collect(),complete}}
 fn p<F>(field:F,mode:SetMode,values:&[&str])->Predicate<F>{Predicate{field,mode,values:values.iter().map(|v|v.to_string()).collect(),exclude:false,include_unknown:false}}
 #[test]fn partial_sets_do_not_prove_negative_but_can_prove_positive(){
 let mut w=WorkFacts::default();w.fields.insert(Field::Characters,set(&["c1"],false));
 for (mode,values,expected) in [(SetMode::Any,vec!["c1","c2"],Truth::Yes),(SetMode::All,vec!["c1","c2"],Truth::Unknown),(SetMode::None,vec!["c2"],Truth::Unknown),(SetMode::None,vec!["c1"],Truth::No)]{assert_eq!(evaluate(&Rule::Field{predicate:p(Field::Characters,mode,&values)},&w),expected);}
 let mut condition=p(Field::People,SetMode::Any,&["s1"]);condition.exclude=true;assert_eq!(evaluate(&Rule::Field{predicate:condition.clone()},&w),Truth::Unknown);condition.include_unknown=true;assert_eq!(evaluate(&Rule::Field{predicate:condition},&w),Truth::Yes);
 }
 #[test]fn same_edition_language_and_available_cannot_be_spliced(){
 let rule=Rule::Edition{join:Join::All,conditions:vec![p(EditionField::ConfirmedLanguages,SetMode::Any,&["zh"]),p(EditionField::Availability,SetMode::Any,&["available"])],exclude:false,include_unknown:false};
 let edition=|lang:&str,availability:&str|EditionFacts{fields:BTreeMap::from([(EditionField::ConfirmedLanguages,set(&[lang],true)),(EditionField::Availability,set(&[availability],true))])};
 let mut work=WorkFacts{editions:vec![edition("zh","missing"),edition("en","available")],editions_complete:true,..Default::default()};assert_eq!(evaluate(&rule,&work),Truth::No);
 work.editions.push(edition("zh","available"));assert_eq!(evaluate(&rule,&work),Truth::Yes);
 work.editions.clear();work.editions_complete=false;assert_eq!(evaluate(&rule,&work),Truth::Unknown);
 work.editions_complete=true;assert_eq!(evaluate(&rule,&work),Truth::No);
 }
 #[test]fn known_empty_is_different_from_unfetched_and_groups_use_three_values(){
 let mut work=WorkFacts::default();let absent=Rule::Field{predicate:p(Field::SourceTags,SetMode::None,&["g1"])};assert_eq!(evaluate(&absent,&work),Truth::Unknown);work.fields.insert(Field::SourceTags,set(&[],true));assert_eq!(evaluate(&absent,&work),Truth::Yes);
 let unknown=Rule::Year{from:2000,to:2020,exclude:false,include_unknown:false};assert_eq!(evaluate(&Rule::Group{join:Join::Any,children:vec![absent.clone(),unknown.clone()]},&work),Truth::Yes);assert_eq!(evaluate(&Rule::Group{join:Join::All,children:vec![absent,unknown]},&work),Truth::Unknown);
 }
 #[test]fn validation_rejects_unbounded_invalid_and_untyped_rules(){
 let field=Rule::Field{predicate:p(Field::Status,SetMode::Any,&["backlog"])};let valid=Rule::Group{join:Join::All,children:vec![Rule::Group{join:Join::Any,children:vec![field.clone()]}]};assert!(validate(&valid).is_ok());assert!(validate(&Rule::Group{join:Join::All,children:vec![valid]}).is_err());assert!(validate(&Rule::Group{join:Join::All,children:vec![field;64]}).is_err());assert!(validate(&Rule::Group{join:Join::Any,children:vec![]}).is_err());assert!(serde_json::from_str::<Rule>(r#"{"kind":"sql","query":"SELECT *"}"#).is_err());assert!(serde_json::from_str::<Rule>(r#"{"kind":"field","predicate":{"field":"other_list","mode":"any","values":["l"]}}"#).is_err());
 }
}

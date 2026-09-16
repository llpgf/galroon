//! Collection filtering shared by bounded catalog reads. Keep parity with src/library.ts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Filters {
    pub studio: String, pub year: String, pub language: String, pub tag: String,
    pub availability: String, pub tags: Vec<String>, pub excluded_tags: Vec<String>,
    pub favorites: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Query {
    pub filters: Filters, pub query: String, pub status: String,
    pub sort: String, pub title_mode: String, pub locale: String,
}
impl Default for Query {
    fn default() -> Self { Self { filters: Filters::default(), query: String::new(), status: "all".into(), sort: "added".into(), title_mode: "display".into(), locale: "en".into() } }
}
impl Query {
    pub fn validate(&self) -> Result<(), String> {
        if self.query.encode_utf16().count()>200 || self.locale.len()>64 { return Err("Collection search is too long".into()); }
        if !["all","favorite","backlog","playing","completed","on_hold","dropped"].contains(&self.status.as_str()) || !["added","title","newest","oldest"].contains(&self.sort.as_str()) || !["display","original"].contains(&self.title_mode.as_str()) { return Err("Invalid collection view".into()); }
        let f=&self.filters;
        if !["","available","missing","unverified","offline","unknown","no_resources"].contains(&f.availability.as_str()) { return Err("Invalid availability filter".into()); }
        if f.tags.len()>100 || f.excluded_tags.len()>100 || [&f.studio,&f.year,&f.language,&f.tag].into_iter().chain(f.tags.iter()).chain(f.excluded_tags.iter()).any(|s| s.len()>1024) { return Err("Too many or oversized collection filters".into()); }
        Ok(())
    }
}
#[derive(Default)]
pub struct Entry {
    pub id: String, pub title: String, pub original: String, pub studio: String,
    pub year: String, pub tags: Vec<String>, pub custom_tags: BTreeSet<String>,
    pub languages: BTreeSet<String>, pub states: BTreeSet<String>,
    pub search: String, pub status: String, pub favorite: bool, pub added: i64,
}
impl Entry {
    pub fn title(&self, mode: &str) -> &str {
        let (first, fallback)=if mode=="original" { (&self.original,&self.title) } else { (&self.title,&self.original) };
        if first.is_empty() { fallback } else { first }
    }
}
pub fn normalized(s: &str) -> String { s.nfkc().collect::<String>().trim().to_lowercase() }
pub fn tag_names(value: &serde_json::Value) -> Vec<String> {
    let mut out=Vec::new();
    if let Some(tags)=value.as_array() { for tag in tags { if let Some(s)=tag.as_str().or_else(||tag.get("name").and_then(|v|v.as_str())) { let s=s.trim(); if !s.is_empty()&&!out.iter().any(|v|v==s) { out.push(s.to_owned()); } } } }
    out
}
pub fn fold(s: &str) -> String {
    static MARKS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static OTHER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let decomposed=s.nfkd().collect::<String>();
    let lower=MARKS.get_or_init(||regex::Regex::new(r"\p{M}").unwrap()).replace_all(&decomposed, "").to_lowercase();
    OTHER.get_or_init(||regex::Regex::new(r"[^\p{L}\p{N}]+").unwrap()).replace_all(&lower," ").trim().to_owned()
}
fn close_word(a: &str, b: &str) -> bool {
    // JS uses UTF-16 units, including the length thresholds and transpositions.
    let a=a.encode_utf16().collect::<Vec<_>>(); let b=b.encode_utf16().collect::<Vec<_>>();
    let limit=if a.len()>=8 {2} else {1};
    if a.len()<4||a.len().abs_diff(b.len())>limit { return false; }
    let mut previous=(0..=b.len()).collect::<Vec<_>>(); let mut older=previous.clone();
    for i in 1..=a.len() {
        let mut row=vec![i; b.len()+1];
        for j in 1..=b.len() {
            row[j]=(previous[j]+1).min(row[j-1]+1).min(previous[j-1]+usize::from(a[i-1]!=b[j-1]));
            if i>1&&j>1&&a[i-1]==b[j-2]&&a[i-2]==b[j-1] { row[j]=row[j].min(older[j-2]+1); }
        }
        if *row.iter().min().unwrap()>limit { return false; }
        older=previous;previous=row;
    }
    previous[b.len()]<=limit
}
pub fn fuzzy_folded(hay: &str, q: &str) -> bool {
    if q.is_empty()||hay.contains(q)||hay.replace(' ', "").contains(&q.replace(' ', "")) { return true; }
    q.split(' ').all(|w|hay.contains(w)||hay.split(' ').any(|candidate|close_word(w,candidate)))
}
fn value_matches<'a>(filter: &str, values: impl IntoIterator<Item=&'a str>) -> bool {
    if filter.is_empty() { return true; }
    let mut empty=true;
    for value in values { empty=false; if filter==format!("value:{}",normalized(value)) { return true; } }
    empty&&filter=="unknown"
}
pub fn matches(e: &Entry, q: &Query, folded_query: &str) -> bool {
    let f=&q.filters;
    if f.favorites&&!e.favorite || q.status!="all"&&if q.status=="favorite" {!e.favorite} else {e.status!=q.status} { return false; }
    let tagged=|tag:&str| if tag.starts_with("custom:") { e.custom_tags.contains(tag) } else { value_matches(tag,e.tags.iter().map(String::as_str)) };
    let excluded=|tag:&String| f.excluded_tags.contains(tag);
    if f.tags.iter().filter(|tag|excluded(tag)).any(|tag|tagged(tag)) { return false; }
    let included=f.tags.iter().filter(|tag|!excluded(tag)).collect::<Vec<_>>();
    if !included.is_empty()&&!included.iter().any(|tag|tagged(tag)) { return false; }
    fuzzy_folded(&e.search,folded_query)
        &&value_matches(&f.studio,(!e.studio.is_empty()).then_some(e.studio.as_str()))
        &&value_matches(&f.year,(!e.year.is_empty()).then_some(e.year.as_str()))
        &&value_matches(&f.language,e.languages.iter().map(String::as_str))
        &&value_matches(&f.tag,e.tags.iter().map(String::as_str))
        &&(f.availability.is_empty()||e.states.contains(&f.availability))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn fuzzy_search_preserves_accents_width_spacing_transposition_and_short_queries() {
        for (text,q,yes) in [("Café Ｓｔｅｉｎｓ Gate","cafe steinsgate",true),("Steins Gate","stiens",true),("Umineko","uminako",true),("A title","tix",false),("日本語作品","日本 語",true),("星空","",true),("Moon","noon",true),("Moon","sun",false)] { assert_eq!(fuzzy_folded(&fold(text),&fold(q)),yes,"{text} / {q}"); }
    }
    #[test] fn tags_union_exclusions_identity_unknown_and_status_intersect() {
        let e=Entry{tags:vec!["Drama".into()],custom_tags:BTreeSet::from(["custom:one".into()]),states:BTreeSet::from(["no_resources".into()]),status:"backlog".into(),..Default::default()};
        let mut q=Query::default();q.filters.tags=vec!["custom:absent".into(),"value:drama".into()];assert!(matches(&e,&q,""));
        q.filters.excluded_tags=vec!["custom:absent".into()];assert!(matches(&e,&q,""));
        q.filters.excluded_tags.push("value:drama".into());assert!(!matches(&e,&q,""));
        q.filters.tags.clear();assert!(matches(&e,&q,"")); // Unselected exclusions do nothing.
        q.filters.language="unknown".into();assert!(matches(&e,&q,""));q.filters.language="value:ja".into();assert!(!matches(&e,&q,""));
        q.filters.language.clear();q.status="favorite".into();assert!(!matches(&e,&q,""));
    }
}


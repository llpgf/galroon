//! Bounded candidate retrieval and explainable ranking. Never binds a resource.
use regex::Regex;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, sync::OnceLock, time::{Duration, Instant}};
use unicode_normalization::UnicodeNormalization;
pub const ALGORITHM_VERSION:i64=3;

fn replace(text: &str, pattern: &str, with: &str) -> String {
    Regex::new(pattern).unwrap().replace_all(text, with).into_owned()
}
pub fn normalized(text: &str) -> String {
    text.nfkc().flat_map(char::to_lowercase).map(|c| {
        // Katakana and hiragana spellings compare equally; retain long vowel marks.
        if ('\u{30a1}'..='\u{30f6}').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap() } else { c }
    }).filter(|c| c.is_alphanumeric()).collect()
}
fn useful(text: &str) -> bool {
    let n = normalized(text);
    !n.is_empty() && !n.chars().all(|c| c.is_ascii_digit()) &&
        !Regex::new(r"(?i)^(?:rj|vj|dmm)\d+$").unwrap().is_match(&n) &&
        !Regex::new(r"^[A-Z]{2,5}[0-9]{3,6}$").unwrap().is_match(text) &&
        !["files","game","data","setup","disc","disk","update","patch","crack","soundtrack","metadata","readme","sofmap","ソフマップ","wave","wav","mp3","ogg","flac","voice","sound","bgm","se","html","images","image","plugin","plugins","system","save","savedata","config","log","logs","sjisext","ysbin","updchk","movie","movies","manual","特典","音声特典","バイノーラルドラマ"].contains(&n.as_str()) &&
        !Regex::new(r"^(?:\d+[. _-]*)?(?:(?:バイノーラル|ボイス|音声)ドラマ|主題歌音源)$").unwrap().is_match(text)
}
fn technical_directory(text:&str)->bool{
    ["html","images","image","plugin","plugins","ysbin","updchk","sjisext","wave","wav","mp3","ogg","flac","savedata","bgm","se"].contains(&normalized(text).as_str())
}
/// Strip distribution labels, not arbitrary title brackets or sequel subtitles.
pub fn clean_name(input: &str) -> String {
    let mut s: String = input.nfkc().collect();
    s = replace(&s, r"(?i)(?:\.part\d+)?\.(?:rar|zip|7z|iso|mdf|mds|bin|cue)(?:\.\d+)?$", "");
    s = replace(&s, r"(?i)^(?:\((?:18禁ゲーム|18禁|一般ゲーム|一般コミック|同人ゲーム)\)\s*)+", "");
    // A run of distribution brackets is metadata. A single meaningful [title] survives.
    let prefix = Regex::new(r"^((?:\[[^\]]+\]\s*)+)").unwrap();
    if let Some(caps) = prefix.captures(&s) {
        let labels = caps[1].matches('[').count();
        let numeric = Regex::new(r"^\[(?:\d{6,8}|(?i:rj|vj)\d+)\]").unwrap().is_match(&caps[1]);
        if labels >= 2 || numeric { s = s[caps[0].len()..].to_string(); }
    }
    s = replace(&s, r"(?i)\s+\+\s*(?:theme song|voice(?: drama| tokuten)?|vocal cd|soundtrack|update|tokuten|wallpaper)\b.*$", "");
    s = replace(&s, r"(?i)\s+(?:(?:豪華|予約|初回|特典付き|通常|限定|パッケージ|ダウンロード|DL)+版|(?:FANZA|Sofmap|ソフマップ|予約|初回|限定)?特典|追加コンテンツ|壁紙セット|Crack(?:\s|$)).*$", "");
    s = replace(&s, r"(?i)\s*\((?:files|mdf|mds|iso|bin|cue|wav|mp3|pdf|jpg|rr\d+)(?:\+(?:files|mdf|mds|iso|bin|cue|wav|mp3|pdf|jpg|rr\d+))*\)\s*$", "");
    s = replace(&s, r"(?i)[\s_]+(?:修正パッチ|修正ファイル|アップデート|patch|update)(?:[\s_]|ver|v?\d|$).*$", "");
    s = replace(&s, r"[\s_]+(?:特典(?:ボイスドラマ|ASMR|音声)|限定版音声特典|主題歌(?:マキシ)?音源|パッケージ\s*&\s*リーフレット).*$", "");
    s = replace(&s, r"\s+", " ").trim().to_string();
    // Some release names accidentally repeat the whole title twice.
    for (i, _) in s.match_indices(' ') {
        if normalized(&s[..i]) == normalized(&s[i + 1..]) { return s[..i].to_string(); }
    }
    s
}
#[derive(Debug, Clone)]
pub struct Input { pub titles: Vec<String>, pub queries: Vec<String>, pub direct_id: Option<String> }
pub fn prepare(query: &str, hints: &[String]) -> Result<Input, String> {
    if query.len() > 4096 || query.trim().is_empty() { return Err("Enter a title or VNDB work ID (up to 4096 bytes)".into()); }
    let direct = Regex::new(r"(?i)^(?:https://vndb\.org/)?(v[0-9]+)/*$").unwrap();
    if let Some(c) = direct.captures(query.trim()) { return Ok(Input { titles: vec![query.into()], queries: vec![], direct_id: Some(c[1].to_lowercase()) }); }
    let mut titles = Vec::new(); let mut seen = HashSet::new();
    for (index,raw) in std::iter::once(query).chain(hints.iter().take(32).map(String::as_str)).enumerate() {
        // Only a relative name is searched; the caller never supplies the source root.
        // A manually entered title can contain a slash (Fate/stay night).
        let manual_title=index==0 && hints.is_empty() && !raw.contains('\\') && !raw.starts_with('/')
            && !Regex::new(r"^\d{6,}/|(?i)\.(?:rar|zip|7z|iso)(?:\.\d+)?$").unwrap().is_match(raw);
        let parts=if manual_title {vec![raw]}else{
            let parts:Vec<_>=raw.split(['/', '\\']).collect();
            // Keep work ancestors, not the implementation folders inside media/runtime trees.
            let end=parts.iter().position(|p|technical_directory(p)).unwrap_or(parts.len());
            parts[..end].iter().rev().take(4).copied().collect()
        };
        for part in parts {
            // Runtime files and media tracks are not competing work identities.
            if Regex::new(r"(?i)\.(?:txt|json|torrent|exe|dll|xp3|dat|ini|png|jpg|jpeg|gif|bmp|webp|wav|mp3|ogg|flac|m4a|mp4|avi|log|ybn|ypf|pck|ks|tjs|html?|css|js|pdf)$").unwrap().is_match(part) { continue; }
            let title = clean_name(part);
            if useful(&title) && seen.insert(normalized(&title)) { titles.push(title); }
        }
    }
    titles.truncate(6);
    let mut queries = titles.iter().take(2).cloned().collect::<Vec<_>>();
    // Retrieve the tail, but preserve the full input as evidence until developer corroboration.
    if let Some((_,tail))=titles.first().and_then(|s|branded(s)){if useful(&tail)&&!queries.contains(&tail){queries.push(tail);}}
    // A base-title fallback retrieves candidates; ranking always uses the full title.
    if let Some(first) = titles.first() {
        let base = Regex::new(r"\s+[～~〜-]").unwrap().split(first).next().unwrap().trim();
        if normalized(base).chars().count() >= 4 && !queries.iter().any(|q| normalized(q) == normalized(base)) { queries.push(base.into()); }
    }
    queries.truncate(3);
    Ok(Input { titles, queries, direct_id: None })
}
fn numbers(text: &str) -> Vec<String> {
    let s: String = text.nfkc().collect();
    static RE: OnceLock<Regex> = OnceLock::new();
    let mut values = RE.get_or_init(||Regex::new(r"\d+|\b(?:II|III|IV|VI|VII|VIII|IX)\b|[弐参]").unwrap()).find_iter(&s).map(|m| match m.as_str() {
        "II"|"弐" => "2".into(), "III"|"参" => "3".into(), "IV"=>"4".into(), "VI"=>"6".into(), "VII"=>"7".into(), "VIII"=>"8".into(), "IX"=>"9".into(), n=>n.into()
    }).collect::<Vec<String>>(); values.sort(); values.dedup(); values
}
/// A compact romanized filename can corroborate an exact full title, but cannot
/// establish identity by itself. Keep this out of ranking/strong-match thresholds.
pub fn corroborates_filename(query:&str,candidate:&Value)->bool {
    if !query.is_ascii()||query.chars().any(char::is_whitespace)||query.len()<6{return false;}
    fn roman(s:&str)->String{
        let s=replace(s,r"(?i)\[Ni\]|\(Ni\)","2");
        normalized(&s).replace("syou","shou").replace("syo","sho").replace("syu","shu").replace("sya","sha").replace("tyou","chou").replace("tyo","cho").replace("tyu","chu").replace("tya","cha")
    }
    let nq=roman(query);let mut names=vec![];
    for key in ["title","alttitle"]{if let Some(s)=candidate[key].as_str(){names.push(s);}}
    if let Some(titles)=candidate["titles"].as_array(){for t in titles{if let Some(s)=t["latin"].as_str(){names.push(s);}}}
    names.into_iter().filter(|s|s.is_ascii()).any(|name|{
        let base=Regex::new(r"\s+[~～〜-]").unwrap().split(name).next().unwrap().trim();nq==roman(name)||nq==roman(base)
    })
}
fn similarity(a: &str, b: &str) -> f64 {
    let ac: Vec<char> = a.chars().collect(); let bc: Vec<char> = b.chars().collect();
    if ac.len() < 4 || bc.len() < 4 { return 0.0; }
    let grams = |c: &[char]| c.windows(2).map(|x| (x[0],x[1])).collect::<HashSet<_>>();
    let aa = grams(&ac); let bb = grams(&bc);
    2.0 * aa.intersection(&bb).count() as f64 / (aa.len() + bb.len()) as f64
}
fn branded(query:&str)->Option<(String,String)>{let re=Regex::new(r"^\[([^\]]+)\]\s+(.+)$").unwrap();let caps=re.captures(query)?;Some((caps[1].to_owned(),caps[2].to_owned()))}
#[test]fn single_brand_requires_developer_and_complete_title_and_preserves_short_guard(){
 let c=json!({"id":"v4","title":"CLANNAD","developers":[{"name":"Key"}]});let input=prepare("[Key] CLANNAD",&[]).unwrap();assert!(input.queries.contains(&"CLANNAD".into()));assert_eq!(input.titles,vec!["[Key] CLANNAD"]);assert_eq!(rank(&input,vec![c.clone()])[0]["match"]["strength"],"strong");assert_ne!(rank(&prepare("[Unknown] CLANNAD",&[]).unwrap(),vec![c.clone()])[0]["match"]["strength"],"strong");assert_ne!(rank(&prepare("[Key] CLANNAD 2",&[]).unwrap(),vec![c])[0]["match"]["strength"],"strong");let short=json!({"id":"v36","title":"AIR","developers":[{"name":"Key"}]});assert_ne!(rank(&prepare("[Key] AIR",&[]).unwrap(),vec![short])[0]["match"]["strength"],"strong");
}
pub fn rank(input: &Input, candidates: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new(); let mut ranked = Vec::new();
    for mut candidate in candidates {
        let Some(id) = candidate["id"].as_str().map(str::to_owned) else { continue; };
        if !seen.insert(id.clone()) { continue; }
        let mut names: Vec<(String, bool)> = ["title", "alttitle"].iter().filter_map(|k| candidate[k].as_str().map(|s| (s.into(), false))).collect();
        if let Some(titles) = candidate["titles"].as_array() { for title in titles { for key in ["title", "latin"] { if let Some(s) = title[key].as_str() { names.push((s.into(), false)); } } } }
        if let Some(aliases) = candidate["aliases"].as_array() { for s in aliases.iter().filter_map(Value::as_str) { names.push((s.into(), true)); } }
        let mut best = 0.0_f64; let mut reason = "provider_result"; let mut matched = String::new(); let mut conflict = false;
        for query in &input.titles { let nq = normalized(query); for (name, alias) in &names {
            let nn = normalized(name); if nn.is_empty() { continue; }
            let brand_exact=branded(query).is_some_and(|(brand,tail)|normalized(&tail)==nn&&candidate["developers"].as_array().is_some_and(|ds|ds.iter().any(|d|d["name"].as_str().is_some_and(|name|normalized(name)==normalized(&brand)))));
            let exact = nq == nn || brand_exact;
            let mut score = if exact { if *alias { 98.0 } else { 100.0 } } else { 85.0 * similarity(&nq, &nn) };
            let mut why = if brand_exact { "exact_brand_title" } else if exact { if *alias { "exact_alias" } else { "exact_title" } } else { "similar_title" };
            if !exact && (nq.contains(&nn) || nn.contains(&nq)) { score = 45.0 + 23.0 * (nq.chars().count().min(nn.chars().count()) as f64 / nq.chars().count().max(nn.chars().count()) as f64); why = "different_subtitle"; }
            let mismatch = !exact && numbers(query) != numbers(name);
            if mismatch { score = score.min(42.0); why = "number_conflict"; }
            if score > best { best = score; reason = why; matched = name.clone(); conflict = mismatch; }
        } }
        if let Some(releases) = candidate["release_matches"].as_array() {
            for release in releases { if let Some(title) = release["title"].as_str() {
                if input.titles.iter().any(|q| normalized(q)==normalized(&clean_name(title))) && best < 99.0 {
                    best=99.0; reason="exact_release"; matched=title.into(); conflict=false;
                }
            } }
        }
        if let Some(titles)=candidate["confirmed_titles"].as_array(){for title in titles.iter().filter_map(|v|v["title"].as_str()){
            if input.titles.iter().any(|q|normalized(q)==normalized(title))&&best<105.0 {best=105.0;reason="confirmed_title";matched=title.into();conflict=false;}
        }}
        if input.direct_id.as_deref() == Some(&id) { best = 110.0; reason = "direct_id"; }
        candidate["match"] = json!({"score":best.round(),"reason":reason,"matched_title":matched,"number_conflict":conflict,"strength":"review"});
        ranked.push(candidate);
    }
    ranked.sort_by(|a,b| b["match"]["score"].as_f64().unwrap().total_cmp(&a["match"]["score"].as_f64().unwrap()));
    let runner_up = ranked.get(1).and_then(|c| c["match"]["score"].as_f64()).unwrap_or(0.0);
    if let Some(first) = ranked.first_mut() {
        let score = first["match"]["score"].as_f64().unwrap();
        let short = input.titles.iter().any(|t| normalized(&branded(t).map(|(_,tail)|tail).unwrap_or_else(||t.clone())).chars().count() < 4);
        if score >= 98.0 && score - runner_up >= 8.0 && (!short || input.direct_id.is_some()) { first["match"]["strength"] = json!("strong"); }
        if score >= 98.0 && score - runner_up < 8.0 { first["match"]["ambiguous"] = json!(true); }
    }
    ranked.truncate(20); ranked
}

struct Provider { client: reqwest::Client, cache: HashMap<String, (Instant,Value)>, last: Option<Instant> }
static PROVIDER: OnceLock<tokio::sync::Mutex<Provider>> = OnceLock::new();
pub(crate) async fn known_artwork(url:&str)->bool{let Some(provider)=PROVIDER.get()else{return false;};provider.lock().await.cache.values().any(|(_,data)|crate::artwork::referenced(data,url))}
pub async fn search(input: Input) -> Result<Value,String> {
    let shared = PROVIDER.get_or_init(|| tokio::sync::Mutex::new(Provider { client:reqwest::Client::new(), cache:HashMap::new(), last:None }));
    let mut provider = shared.try_lock().map_err(|_| "Another VNDB search is running. Try again shortly.".to_string())?;
    retrieve(&input, &mut provider, "https://api.vndb.org/kana/vn", Duration::from_millis(1600)).await
}
const WORK_FIELDS: &str = "title,alttitle,titles.title,titles.latin,aliases,description,image.url,released,developers.name,tags.name";
async fn request(provider: &mut Provider, endpoint: &str, body: Value, spacing: Duration) -> Result<Value,String> {
    let key=format!("{endpoint}:{body}");
    if let Some((_,data))=provider.cache.get(&key) { return Ok(data.clone()); }
    if let Some(last)=provider.last { if last.elapsed()<spacing { tokio::time::sleep(spacing-last.elapsed()).await; } }
    provider.last=Some(Instant::now());
    let result=provider.client.post(endpoint).timeout(Duration::from_secs(8)).json(&body).send().await;
    let data=match result {
        Ok(response) if response.status().is_success()=>response.json::<Value>().await.map_err(|_|"VNDB returned an unreadable response".to_string())?,
        Ok(response)=>return Err(format!("VNDB returned {}. Try again later.",response.status())),
        Err(_)=>return Err("VNDB could not be reached within 8 seconds. Try again later.".into())
    };
    if !data["results"].is_array() { return Err("VNDB returned no results field".into()); }
    if provider.cache.len()>=32 { provider.cache.clear(); }
    provider.cache.insert(key,(Instant::now(),data.clone())); Ok(data)
}
async fn retrieve(input: &Input, provider: &mut Provider, endpoint: &str, spacing: Duration) -> Result<Value,String> {
    let queries=if let Some(id)=&input.direct_id {vec![id.clone()]} else {input.queries.clone()};
    let mut candidates=Vec::new();let mut searched=Vec::new();let mut incomplete=false;let mut warning=None;
    provider.cache.retain(|_,(at,_)|at.elapsed()<Duration::from_secs(600));
    // Up to three work queries, one release query and one batched work lookup.
    for (index,query) in queries.into_iter().enumerate() {
        let filter=if input.direct_id.is_some(){"id"}else{"search"};
        let body=json!({"filters":[filter,"=",query],"sort":if filter=="id"{"id"}else{"searchrank"},"fields":WORK_FIELDS,"results":30});
        let data=match request(provider,endpoint,body,spacing).await {
            Ok(data)=>data,Err(message)=>{if candidates.is_empty(){return Err(message);}warning=Some(message);incomplete=true;break;}
        };
        searched.push(query.clone());incomplete|=data["more"].as_bool().unwrap_or(false);
        candidates.extend(data["results"].as_array().unwrap().iter().cloned());
        if rank(input,candidates.clone()).first().is_some_and(|c|c["match"]["strength"]=="strong") {break;}
        if index==0 && input.direct_id.is_none() {
            let release_url=format!("{}/release",endpoint.trim_end_matches("/vn"));
            let body=json!({"filters":["search","=",query],"sort":"searchrank","fields":"title,alttitle,vns.id","results":15});
            let releases=match request(provider,&release_url,body,spacing).await {
                Ok(data)=>data,Err(message)=>{if candidates.is_empty(){return Err(message);}warning=Some(message);incomplete=true;break;}
            };
            incomplete|=releases["more"].as_bool().unwrap_or(false);
            let mut links:HashMap<String,Vec<Value>>=HashMap::new();
            for release in releases["results"].as_array().unwrap() {
                let exact=["title","alttitle"].iter().filter_map(|k|release[k].as_str()).find(|title|input.titles.iter().any(|q|normalized(q)==normalized(&clean_name(title))));
                if let Some(title)=exact { if let Some(vns)=release["vns"].as_array() { for vn in vns.iter().take(30) { if let Some(id)=vn["id"].as_str() {
                    links.entry(id.into()).or_default().push(json!({"id":release["id"],"title":title}));
                } } } }
            }
            // A compilation release can point to multiple works: keep the ambiguity visible.
            let ids:Vec<String>=links.keys().filter(|id|!candidates.iter().any(|c|c["id"].as_str()==Some(id.as_str()))).cloned().collect();
            if !ids.is_empty() {
                let body=json!({"filters":["id","=",ids.iter().take(30).collect::<Vec<_>>()],"fields":WORK_FIELDS,"results":30});
                match request(provider,endpoint,body,spacing).await {Ok(data)=>{incomplete|=data["more"].as_bool().unwrap_or(false)||ids.len()>30;candidates.extend(data["results"].as_array().unwrap().iter().cloned());},Err(message)=>{warning=Some(message);incomplete=true;break;}}
            }
            for candidate in &mut candidates { if let Some(id)=candidate["id"].as_str(){if let Some(matches)=links.get(id){candidate["release_matches"]=json!(matches);}} }
            if rank(input,candidates.clone()).first().is_some_and(|c|c["match"]["strength"]=="strong") {break;}
        }
    }
    let mut results=rank(input,candidates);
    if incomplete {for c in &mut results{c["match"]["strength"]=json!("review");}}
    Ok(json!({"algorithm_version":ALGORITHM_VERSION,"results":results,"searched":searched,"titles":input.titles,"incomplete":incomplete,"warning":warning,"needs_title":input.queries.is_empty()&&input.direct_id.is_none()}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn ancillary_names_do_not_become_competing_titles_or_sequel_numbers(){
        for (query,hints,expected) in [
            ("マガルミナ",vec!["マガルミナ/magalumina_patch_v1_10.zip"],vec!["マガルミナ","magalumina"]),
            ("宿りし乙女の誓いと魔法",vec!["1326550/宿りし乙女の誓いと魔法 修正パッチVer1.01 (files).rar"],vec!["宿りし乙女の誓いと魔法"]),
            ("Actual Title",vec!["Actual Title/Sofmap/Sofmap.wav","Actual Title/ysbin/yst00000.ybn","Actual Title/updchk/bn.ypf","Actual Title/INH450.mds","Actual Title/html/images/colorbox/ie/border.png"],vec!["Actual Title"]),
            ("ダウニャーさんと飼い主くん",vec!["ダウニャーさんと飼い主くん/ダウニャーさんと飼い主くん_特典ASMR/wave/03.Track.wav","ダウニャーさんと飼い主くん_主題歌マキシ音源"],vec!["ダウニャーさんと飼い主くん"]),
        ]{let h:Vec<_>=hints.into_iter().map(str::to_string).collect();assert_eq!(prepare(query,&h).unwrap().titles,expected);}
        assert_eq!(clean_name("Game 2 -After Story-"),"Game 2 -After Story-");
        assert!(prepare("Actual Title",&["Other Game/data.xp3".into()]).unwrap().titles.contains(&"Other Game".into()));
    }
    #[test] fn compact_romanized_filenames_only_corroborate_the_same_full_identity(){
        let c=json!({"id":"v53486","title":"Haison Shoujo [Ni] ~Kageri Sasou Hime no Hako~"});
        assert!(corroborates_filename("haisonsyoujo2",&c));
        for wrong in ["haisonsyoujo","haisonsyoujo3","haisonsyoujo2afterstory"]{assert!(!corroborates_filename(wrong,&c));}
        assert_ne!(rank(&prepare("haisonsyoujo2",&[]).unwrap(),vec![c])[0]["match"]["strength"],"strong");
    }
    fn candidate(id:&str,name:&str)->Value { json!({"id":id,"title":name}) }
    #[test] fn real_distribution_names_keep_identity_and_remove_noise() {
        for (raw,want) in [
            ("(18禁ゲーム) [251219] [しばそふと] ママ×カノEX  豪華限定版 (mdf+mds+rr3)","ママ×カノEX"),
            ("[251128] [Whirlpool] 猫忍えくすはーとSPIN！ 2 通常版", "猫忍えくすはーとSPIN! 2"),
            ("[250725][1321849][エスクード] 廃村少女［弐］ ～陰り誘う秘姫の匣～ DL版 (files).rar", "廃村少女[弐] ~陰り誘う秘姫の匣~"),
            ("[251219] [FG REMAKE] 下級生リメイク 豪華版.part2.rar", "下級生リメイク"),
            ("[251031] [CloverGAME] やりなおしクランクイン やりなおしクランクイン + Theme Song + Voice Drama", "やりなおしクランクイン"),
            ("[250725][1327332][Archive] アンラベル・トリガー -Prelude to War- DL版 (files).rar", "アンラベル・トリガー -Prelude to War-"),
            ("[250926][1334584][Liquid] 野々村病院の人々 リメイク DL版 (files).rar", "野々村病院の人々 リメイク"),
            ("[250530][VJ01004242][Clover GAME] メイドちゃんは迷途ちゅう DL版&特典 (files).rar", "メイドちゃんは迷途ちゅう"),
            ("[250725][1321849][エスクード] 廃村少女［弐］ ～陰り誘う秘姫の匣～ FANZA特典 (wav).rar", "廃村少女[弐] ~陰り誘う秘姫の匣~"),
            ("[Title]", "[Title]"), ("(Actual Title) After Story", "(Actual Title) After Story"), ("CHAOS;HEAD NOAH", "CHAOS;HEAD NOAH")
        ] { assert_eq!(clean_name(raw),want,"{raw}"); }
    }
    #[test] fn paths_and_numeric_containers_use_members_without_guessing_store_ids() {
        let p = prepare("1327332", &[r"1327332\[250725][Archive] アンラベル・トリガー -Prelude to War- DL版 (files).rar".into()]).unwrap();
        assert_eq!(p.titles,vec!["アンラベル・トリガー -Prelude to War-"]); assert!(p.queries.len()<=3);
        assert!(prepare("VJ01004242", &[]).unwrap().queries.is_empty());
        assert!(prepare("metadata.json", &[]).unwrap().queries.is_empty());
        assert_eq!(prepare("https://vndb.org/v4", &[]).unwrap().direct_id,Some("v4".into()));
    }
    #[test] fn ranks_multilingual_titles_aliases_and_not_provider_id_order() {
        let p=prepare("命運石之門", &[]).unwrap();
        let r=rank(&p,vec![candidate("v1","Something else"),json!({"id":"v2002","title":"Steins;Gate","titles":[{"title":"命運石之門"}]})]);
        assert_eq!(r[0]["id"],"v2002"); assert_eq!(r[0]["match"]["strength"],"strong");
        let p=prepare("ＣＬＡＮＮＡＤ", &[]).unwrap(); assert_eq!(rank(&p,vec![candidate("v4","Clannad")])[0]["match"]["reason"],"exact_title");
        let p=prepare("シュタゲ", &[]).unwrap(); let r=rank(&p,vec![json!({"id":"v2002","title":"Steins;Gate","aliases":["しゅたげ"]})]); assert_eq!(r[0]["match"]["reason"],"exact_alias");
        let p=prepare("Fate/stay night", &[]).unwrap();assert_eq!(p.titles,vec!["Fate/stay night"]);
        let r=rank(&p,vec![candidate("v50","Fate/hollow ataraxia"),candidate("v11","Fate/stay night")]);assert_eq!(r[0]["id"],"v11");assert_eq!(r[0]["match"]["reason"],"exact_title");
    }
    #[test] fn sequels_subtitles_and_collisions_require_review() {
        let p=prepare("猫忍えくすはーとSPIN！ 2", &[]).unwrap();let r=rank(&p,vec![candidate("v1","猫忍えくすはーとSPIN！"),candidate("v2","猫忍えくすはーとSPIN！ 2")]);assert_eq!(r[0]["id"],"v2");assert_eq!(r[1]["match"]["reason"],"number_conflict");
        let p=prepare("アンラベル・トリガー -Prelude to War-", &[]).unwrap();let r=rank(&p,vec![candidate("v1","アンラベル・トリガー")]);assert_eq!(r[0]["match"]["strength"],"review");assert_eq!(r[0]["match"]["reason"],"different_subtitle");
        let p=prepare("Same title", &[]).unwrap();let r=rank(&p,vec![candidate("v1","Same title"),candidate("v2","Same title"),candidate("v1","Same title")]);assert_eq!(r.len(),2);assert_eq!(r[0]["match"]["ambiguous"],true);assert_eq!(r[0]["match"]["strength"],"review");
        let p=prepare("AIR", &[]).unwrap();assert_eq!(rank(&p,vec![candidate("v36","Air")])[0]["match"]["strength"],"review");
    }
    #[tokio::test] async fn release_identity_is_followed_and_compilations_stay_ambiguous() {
        use axum::{routing::post,Json,Router};
        let app=Router::new().route("/vn",post(|Json(v):Json<Value>|async move {
            if v["filters"][0]=="id" {Json(json!({"results":[{"id":"v1","title":"Original name"},{"id":"v2","title":"Other work"}],"more":false}))}
            else {Json(json!({"results":[],"more":false}))}
        })).route("/release",post(||async{Json(json!({"results":[{"id":"r1","alttitle":"Different release name DL版","vns":[{"id":"v1"},{"id":"v2"}]}],"more":false}))}));
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/vn",listener.local_addr().unwrap());let task=tokio::spawn(async move{axum::serve(listener,app).await.unwrap()});
        let mut provider=Provider{client:reqwest::Client::new(),cache:HashMap::new(),last:None};
        let result=retrieve(&prepare("Different release name",&[]).unwrap(),&mut provider,&url,Duration::ZERO).await.unwrap();
        let candidates=result["results"].as_array().unwrap();assert_eq!(candidates.len(),2);
        for c in candidates {assert_eq!(c["match"]["reason"],"exact_release");assert_eq!(c["match"]["strength"],"review");assert_eq!(c["release_matches"][0]["id"],"r1");}
        assert_eq!(candidates[0]["match"]["ambiguous"],true);task.abort();
    }
    #[tokio::test] async fn resource_search_is_authenticated_and_never_writes_catalog() {
        use rusqlite::params;
        let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();
        {let c=app.db.lock().unwrap();c.execute("INSERT INTO roots(id,path,label) VALUES('root',?1,'fixture')",params![t.path().to_string_lossy()]).unwrap();c.execute("INSERT INTO resources(id,root_id,relative_path,title,kind) VALUES('r','root','1327332','1327332','folder')",[]).unwrap();}
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/api/vndb/search",listener.local_addr().unwrap());let a=app.clone();let task=tokio::spawn(async move{crate::serve_listener(a,listener).await.unwrap()});let client=reqwest::Client::new();
        assert_eq!(client.post(&url).json(&json!({"query":"1327332","resource_id":"r"})).send().await.unwrap().status(),401);
        let reply=client.post(&url).bearer_auth(&app.token).json(&json!({"query":"1327332","resource_id":"r"})).send().await.unwrap();assert!(reply.status().is_success());let data:Value=reply.json().await.unwrap();assert_eq!(data["needs_title"],true);assert_eq!(data["searched"],json!([]));
        let c=app.db.lock().unwrap();for table in ["works","jobs","plans","resource_bindings"]{let n:i64=c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0)).unwrap();assert_eq!(n,0);}task.abort();
    }
    #[tokio::test] async fn retrieval_is_bounded_cached_and_surfaces_partial_failures() {
        use axum::{routing::post,Json,Router};
        let count=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let calls=count.clone();
        let app=Router::new().route("/vn",post(move|Json(v):Json<Value>|{let calls=calls.clone();async move {
            calls.fetch_add(1,std::sync::atomic::Ordering::SeqCst);assert_eq!(v["sort"],"searchrank");assert_eq!(v["results"],30);
            if v["filters"][2]=="Second title" {(axum::http::StatusCode::TOO_MANY_REQUESTS,Json(json!({}))) }else {(axum::http::StatusCode::OK,Json(json!({"results":[{"id":"v1","title":"Possible title"}],"more":false})))}
        }})).route("/release",post(||async{Json(json!({"results":[],"more":false}))}));
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/vn",listener.local_addr().unwrap());let task=tokio::spawn(async move{axum::serve(listener,app).await.unwrap()});
        let mut provider=Provider{client:reqwest::Client::new(),cache:HashMap::new(),last:None};
        let input=prepare("First title",&["Second title".into(),"Third title".into()]).unwrap();
        let result=retrieve(&input,&mut provider,&url,Duration::ZERO).await.unwrap();assert_eq!(result["incomplete"],true);assert!(result["warning"].as_str().unwrap().contains("429"));assert_eq!(result["results"].as_array().unwrap().len(),1);assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst),2);
        let _=retrieve(&input,&mut provider,&url,Duration::ZERO).await.unwrap();assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst),3);task.abort();
    }
}

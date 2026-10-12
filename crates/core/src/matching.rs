//! Bounded candidate retrieval and explainable ranking. Never binds a resource.
use regex::Regex;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, sync::OnceLock, time::{Duration, Instant}};
use unicode_normalization::UnicodeNormalization;
pub const ALGORITHM_VERSION:i64=7;

fn replace(text: &str, pattern: &str, with: &str) -> String {
    Regex::new(pattern).unwrap().replace_all(text, with).into_owned()
}
pub fn normalized(text: &str) -> String {
    text.nfkc().flat_map(char::to_lowercase).map(|c| {
        // Katakana and hiragana spellings compare equally; retain long vowel marks.
        if ('\u{30a1}'..='\u{30f6}').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap() } else { c }
    // "+" names a different product (アマナツ is not アマナツ＋), so it survives like a letter.
    }).filter(|c| c.is_alphanumeric() || *c == '+').collect()
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
/// One " + …" part of a release name that lists bundled extras ("Drama CD", "OST", "Update 1.01", "Tokuten x6", "予約特典").
fn extras_segment(part: &str) -> bool {
    static WORDS: OnceLock<Regex> = OnceLock::new();
    let words = WORDS.get_or_init(|| Regex::new(r"(?i)\b(?:op|ed|opening|ending|theme|songs?|voice|vocals?|drama|audio|cd|dvd|disks?|discs?|soundtracks?|ost|original|sound|album|collection|maxi|single|mini|character|special|contents?|bonus|manual|wallpapers?|tokuten|update|patch(?:es)?|fix|crack(?:ed)?|omake|extra|cover|visual|fan|book|artbook|label|transparent|clothes|custom|invisible|harem|amazon|sofmap|getchu|gamers|toranoana|melonbooks|and|ver)\b|特典|予約|初回|同梱|店舗|修正|パッチ|マニュアル|オリジサントラ|サントラ|サウンドトラック|ハーレム|ドラマ|ボイス|壁紙|レーベル|解放|期間限定|DLC|ソフマップ|ボーカル|ヴォーカル|アルバム|主題歌|音声|設定資料|原画集|画集|ビジュアル|ファンブック|マキシ|シングル|おまけ").unwrap());
    let rest = words.replace_all(part, "");
    words.is_match(part) && rest.chars().all(|c| !c.is_alphanumeric() || c.is_ascii_digit() || c == 'x' || c == 'X' || c == 'v')
}
/// Drop a trailing run of " + extras" parts; a " + " joining two real titles stays.
fn strip_plus_extras(s: &str) -> String {
    let sep = Regex::new(r"\s+\+\s*").unwrap();
    let cuts: Vec<(usize, usize)> = sep.find_iter(s).map(|m| (m.start(), m.end())).collect();
    for (i, &(start, _)) in cuts.iter().enumerate() {
        let tail_ok = cuts[i..].iter().enumerate().all(|(j, &(_, end))| {
            let stop = cuts.get(i + j + 1).map_or(s.len(), |c| c.0);
            let part = &s[end..stop];
            part.trim().is_empty() || extras_segment(part)
        });
        if tail_ok && start > 0 { return s[..start].to_string(); }
    }
    s.to_string()
}
/// Strip distribution labels, not arbitrary title brackets or sequel subtitles.
pub fn clean_name(input: &str) -> String {
    let mut s: String = input.nfkc().collect();
    s = replace(&s, r"(?i)(?:\.part\d+)?\.(?:rar|zip|7z|iso|mdf|mds|bin|cue)(?:\.\d+)?$", "");
    s = replace(&s, r"(?i)^(?:\((?:18禁ゲーム|18禁|一般ゲーム|一般コミック|同人ゲーム|FN輸入)\)\s*)+", "");
    // Uploader tags such as "『girlcelly』" or "│2D.G.F.│"; a Japanese 『title』 survives.
    s = replace(&s, r"^\s*(?:『[\x20-\x7e]+』|│[^│]+│)\s*", "");
    // A run of distribution brackets is metadata. A single meaningful [title] survives.
    let prefix = Regex::new(r"^((?:\[[^\]]+\]\s*)+)").unwrap();
    if let Some(caps) = prefix.captures(&s) {
        let labels = caps[1].matches('[').count();
        let numeric = Regex::new(r"^\[(?:\d{6,8}|(?i:rj|vj)\d+)\]").unwrap().is_match(&caps[1]);
        if labels >= 2 || numeric { s = s[caps[0].len()..].to_string(); }
    }
    // "アマカノ2 ＋ + Tokuten Voice": a doubled "+" means the first one belongs to the title (アマカノ2＋).
    s = replace(&s, r"(\S)\s+\+(\s+\+)", "$1+$2");
    s = strip_plus_extras(&s);
    s = replace(&s, r"(?i)\s+(?:(?:豪華|予約|初回|特典付き|特装|通常|限定|パッケージ|ダウンロード|DL|多国語)+版|(?:FANZA|Sofmap|ソフマップ|予約|初回|限定)?特典|追加コンテンツ|壁紙セット|Crack(?:\s|$)).*$", "");
    s = replace(&s, r"(?i)\s*\((?:files|mdf|mds|iso|bin|cue|wav|mp3|pdf|jpg|rr\d+)(?:\+(?:files|mdf|mds|iso|bin|cue|wav|mp3|pdf|jpg|rr\d+))*\)\s*$", "");
    s = replace(&s, r"(?i)[\s_]+(?:修正パッチ|修正ファイル|アップデート|patch|update)(?:[\s_]|ver|v?\d|$).*$", "");
    s = replace(&s, r"[\s_]+(?:特典(?:ボイスドラマ|ASMR|音声)|限定版音声特典|主題歌(?:マキシ)?音源|パッケージ\s*&\s*リーフレット).*$", "");
    // Bonus and re-release descriptors: "Webアンケート特典", "エッチな音声データ", "フルHDリマスター版", "アニメ追加版".
    s = replace(&s, r"\s+\S*(?:特典|アンケート|音声データ|ボイスデータ)(?:[\s「(]|$).*$", "");
    s = replace(&s, r"\s+\S*(?:リマスター|リニューアル|アニメ追加|新装|廉価|普及)版(?:\s|$).*$", "");
    // A " +" left dangling after its extras were removed is a separator, not part of the title.
    s = replace(&s, r"\s+\+$", "");
    s = replace(&s, r"\s+", " ").trim().to_string();
    // Some release names accidentally repeat the whole title twice.
    for (i, _) in s.match_indices(' ') {
        if normalized(&s[..i]) == normalized(&s[i + 1..]) { return s[..i].to_string(); }
    }
    s
}
#[derive(Debug, Clone)]
pub struct Input { pub titles: Vec<String>, pub queries: Vec<String>, pub direct_id: Option<String>,
    /// Distribution brands and release dates read from [brackets] in the resource and member names.
    /// They only corroborate a title; they never identify a work by themselves.
    pub brands: Vec<String>, pub dates: Vec<String> }
/// Brand and date labels such as `[251219] [しばそふと]`. Store codes and numeric IDs are ignored.
fn labels(raw: &str, brands: &mut Vec<String>, dates: &mut Vec<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    let code = CODE.get_or_init(|| Regex::new(r"(?i)^(?:rj|vj|dmm)\d+$").unwrap());
    let s: String = raw.nfkc().collect();
    for c in RE.get_or_init(|| Regex::new(r"\[([^\]/\\]+)\]").unwrap()).captures_iter(&s) {
        let label = c[1].trim();
        let date = |y: &str, m: &str, d: &str| format!("{y}-{m}-{d}");
        let digits = label.chars().all(|c| c.is_ascii_digit());
        if digits && label.len() == 6 { let v = date(&format!("20{}", &label[..2]), &label[2..4], &label[4..]); if !dates.contains(&v) { dates.push(v); } }
        else if digits && label.len() == 8 { let v = date(&label[..4], &label[4..6], &label[6..]); if !dates.contains(&v) { dates.push(v); } }
        else if !digits && !code.is_match(label) && normalized(label).chars().count() >= 2 && !brands.contains(&label.to_string()) { brands.push(label.to_string()); }
    }
}
/// Bonus material such as store or survey extras, drama CDs, soundtracks and wallpapers.
/// Splits a name into the part before its first bonus/crack/patch/artwork word, dropping release labels and format notes.
fn extras_split(name: &str) -> (String, String) {
    let s: String = name.nfkc().collect();
    let s = replace(&replace(&replace(&strip_plus_extras(&s), r"^(?:\s*(?:\([^)]*\)|\[[^\]]*\]))+\s*", ""), r"\([^)]*\)", " "), r"※.*$", "");
    let words = Regex::new(r"(?i)特典|同梱|予約|初回|限定|豪華|通常版|パッケージ版|pkg|dl版|ダウンロード|アップデート|update|patch|パッチ|修正|プログラム|crack|crak|クラック|認証回避|ヘッダー|アイコン|イラスト|壁紙|wallpaper|サウンドトラック|soundtrack|\bost\b|サントラ|original|オリジナル|ボーカル|vocal|song|album|ドラマ|ボイス|voice|drama|音声|データ|manual|マニュアル|bonus|omake|tokuten|dlc|4コマ|チラシ|ビジュアル|visual ?book|ガイドブック|artbook|メロンブックス|ソフマップ|sofmap|amazon|fanza|\bver\.?\s*\d[\d.]*|\bx\d+\b").unwrap();
    let head = words.find(&s).map_or(s.clone(), |m| s[..m.start()].to_string());
    let rest = words.replace_all(&s, "").into_owned();
    (head, rest)
}
fn ascii_code(text: &str) -> bool { let t = text.trim().trim_matches(|c: char| !c.is_alphanumeric()); !t.is_empty() && t.is_ascii() && !t.contains(char::is_whitespace) && t.len() <= 32 }
// Match the title before interpreting its tail: bonus words can also occur inside a real title.
// Do not normalize the tail away; numbers, product suffixes and unlicensed quotations are evidence.
fn member_tail(name:&str,names:&[&str],brands:&[&str])->Option<(String,bool)>{
    static WRAPPERS:OnceLock<(Regex,Regex,Regex,Regex,Regex)>=OnceLock::new();
    let (uploader,distribution,brackets,quoted,reading)=WRAPPERS.get_or_init(||(
        Regex::new(r"^\s*\S+@\d+-\s*(\[\d{6}\])").unwrap(),Regex::new(r"^\s*(?:\(?(?:PD輸入|FN輸入|18禁ゲーム|18禁|一般ゲーム|同人ゲーム)\)\s*)+").unwrap(),
        Regex::new(r"^(?:\s*\[[^\]]+\])+\s*").unwrap(),Regex::new(r"^([^\s「『」』]*)[「『]([^「『」』]+)[」』](.*)$").unwrap(),Regex::new(r"\([^)]*\)").unwrap()));
    let s:String=name.nfkc().collect();let s=uploader.replace(&s,"$1");let s=distribution.replace(&s,"");let bracketed=brackets.is_match(&s);let s=brackets.replace(&s,"");let s=s.trim();
    let variants:Vec<String>=names.iter().flat_map(|n|{let n:String=n.nfkc().collect();vec![normalized(&n),normalized(&reading.replace_all(&n,""))]}).filter(|n|!n.is_empty()).collect();
    if let Some(c)=quoted.captures(s){
        let prefix=&c[1];let maker=prefix.is_empty()||prefix=="ゲーム"||brands.iter().any(|b|normalized(b)==normalized(prefix));
        if maker&&variants.contains(&normalized(&c[2])){return Some((c[3].to_string(),false));}
    }
    // A bracketed maker may use '+' as its separator. A '+' after the title still needs a real tail.
    let distribution_plus=bracketed&&s.starts_with('+');let s=if distribution_plus{s.trim_start_matches('+')}else{s};
    let mut ends:Vec<_>=s.char_indices().map(|(i,c)|i+c.len_utf8()).collect();ends.reverse();
    ends.into_iter().find(|&end|variants.contains(&normalized(&s[..end]))).map(|end|{
        let tail=&s[end..];let title_plus=tail.starts_with('+')&&!s[..end].ends_with(char::is_whitespace)&&!distribution_plus;
        (tail.to_string(),title_plus)
    })
}
// Balanced Japanese quotations also occur inside song/drama titles.
fn quoted_end(s:&str)->Option<usize>{
    let mut ends=Vec::new();
    for (i,c) in s.char_indices(){match c{
        '「'=>ends.push('」'),'『'=>ends.push('』'),'」'|'』'=>{if ends.pop()!=Some(c){return None;}if ends.is_empty(){return Some(i+c.len_utf8());}},_=>{}
    }}None
}
fn media_tail(tail:&str)->bool{
    static TOKEN:OnceLock<Regex>=OnceLock::new();
    static NOTE:OnceLock<Regex>=OnceLock::new();
    let token=TOKEN.get_or_init(||Regex::new(r"(?i)^(?:オープニング|エンディング|キャラクター|コンプリート|ミニ|サウンドトラック|サウンド|特別限定版|全店舗|げっちゅ屋|とらのあな|メロンブックス|ソフマップ|演出強化|演出調整|R18版|PlayDRM|認証回避|免认证补丁|特典版|(?:[\p{Han}]{1,4})?(?:システムボイス|テーマファイル)|凹んだときに聴きたい励ましCD|\d+週年スペシャルコンテンツ|ゲーム作中歌全\d+曲のFull_ver_音源|OP/ED|主題歌|ソング|ドラマ|ボイス|マキシ|シングル|サントラ|サウンドトラック|アルバム|ボーカル|ヴォーカル|パッチ|オリジナル|予約|同梱|店舗|特典|限定|壁紙|ヘッダー|アイコン|音声|データ|DLC|(?:special|complete|original|mini|character|soundtrack|sound|track|album|collection|vocal|song|drama|voice|cd|dvd|bonus|manual|wallpaper|patch|update|tokuten|amazon|sofmap|getchu|gamers|toranoana|melonbooks)(?-u:\b)|(?:ver\.?\s*|v)\d[\d.]*|vol\.\s*\d+)").unwrap());
    let note=NOTE.get_or_init(||Regex::new(r"(?i)^\((?:flac|tak|wav|mp3|ogg|cue|log|rr\d+|mdf|mds|iso|files)(?:\+(?:flac|tak|wav|mp3|ogg|cue|log|rr\d+|mdf|mds|iso|files))*\)").unwrap());
    let mut s=tail.trim();let mut audio=false;let mut any=false;
    while !s.is_empty(){
        s=s.trim_start_matches(|c:char|c.is_whitespace()||matches!(c,'+'|'_'|'-'|'&'|'!'|'.'));if s.is_empty(){return any;}
        if s.starts_with("(認証回避Patch)"){s=&s["(認証回避Patch)".len()..];any=true;continue;}
        if let Some(m)=note.find(s){s=&s[m.end()..];any=true;continue;}
        if audio&&(s.starts_with('「')||s.starts_with('『')){
            let Some(end)=quoted_end(s)else{return false;};s=&s[end..];continue;
        }
        // Performers and track/drama names are payload only immediately after an audio label.
        if audio&&s.starts_with('/') {return !s[1..].trim().is_empty()&&!s[1..].contains(['「','『','」','』']);}
        if audio&&s.to_lowercase().starts_with("feat. "){return !s[6..].trim().is_empty()&&!s[6..].contains(['「','『','」','』']);}
        let Some(m)=token.find(s)else{return false;};let word=m.as_str().to_lowercase();if competing_tail(&word){return false;}
        if !word.starts_with("vol.")&&word!="dlc"{audio=word.contains("ソング")||word.contains("主題歌")||word.contains("ドラマ")||word=="シングル"||word.ends_with("cd")||["song","drama","cd"].contains(&word.as_str());}
        // Edition/modifier words alone are outside this rule; require actual bonus/media content.
        any|=!["special","complete","original","mini","character","オープニング","エンディング","キャラクター","コンプリート","ミニ","特別限定版","全店舗","げっちゅ屋","とらのあな","メロンブックス","ソフマップ","演出強化","演出調整","r18版","playdrm","予約","同梱","店舗","限定"].contains(&word.as_str())&&!word.starts_with("vol.");s=&s[m.end()..];
    }any||tail.trim().is_empty()
}
// Exclusions for the new fallback only; do not apply them to legacy bonus/code support.
fn competing_tail(tail:&str)->bool{
    static RE:OnceLock<(Regex,Regex)>=OnceLock::new();
    let (audio,product)=RE.get_or_init(||(Regex::new(r"(?i)(?:ソング|主題歌|ドラマ|\bsong\b|\bdrama\b|cd)(?:DLC)?\s*(?:vol\.\s*\d+\s*)?[「『]").unwrap(),
        Regex::new(r"(?i)[「『」』]|ファンディスク|ふぁんでぃすく|アペンド|スピンオフ|外伝|リメイク|ミニゲーム|(?:^|[^a-z])(?:fd|after|remake|append|spin[ -]?off)(?:$|[^a-z])").unwrap()));
    let mut plain=String::new();let mut copied=0;
    for m in audio.find_iter(tail){
        if m.start()<copied{continue;}let start=m.end()-'「'.len_utf8();let mut end=start;
        while tail[end..].starts_with(['「','『']){let Some(n)=quoted_end(&tail[end..])else{break;};end+=n;}
        if end>start{plain.push_str(&tail[copied..start]);copied=end;}
    }
    plain.push_str(&tail[copied..]);product.is_match(&plain)
}
/// Additive A/B fallback: title-anchored media tails and distribution/quoted wrappers.
/// Product and quotation exclusions belong only here; legacy bonus admission is unchanged.
pub fn title_anchored_member(member:&str,names:&[&str],brands:&[&str])->bool{
    let Some((tail,title_plus))=member_tail(member,names,brands)else{return false;};
    !title_plus&&!competing_tail(&tail)&&media_tail(&tail)
}
/// The name is exactly one of `names` followed only by bonus/crack/patch/artwork words.
pub fn names_with_extras(name: &str, names: &[&str]) -> bool {
    let (head, _) = extras_split(name); let key = normalized(&head);
    // Parenthesised readings are dropped from the name, so compare against the bare spelling too.
    !key.is_empty() && names.iter().any(|n| { let n: String = n.nfkc().collect(); normalized(&n) == key || normalized(&replace(&n, r"\([^)]*\)", "")) == key })
}
/// A member name (file or subfolder inside a resource) that adds nothing against the matched work:
/// only bonus/crack/patch/artwork words, an ASCII code name, or one of `names` followed by such words.
pub fn ancillary_member(member: &str, names: &[&str]) -> bool {
    let (head, rest) = extras_split(member);
    if normalized(&head).is_empty() { return normalized(&rest).is_empty() || ascii_code(&rest); }
    ascii_code(&head) || names_with_extras(member, names)
}
/// Every name VNDB knows a candidate work by.
pub fn candidate_names(candidate: &Value) -> Vec<&str> {
    let mut names: Vec<&str> = ["title", "alttitle"].iter().filter_map(|k| candidate[*k].as_str()).collect();
    for t in candidate["titles"].as_array().into_iter().flatten() { names.extend(["title", "latin"].iter().filter_map(|k| t[*k].as_str())); }
    names.extend(candidate["aliases"].as_array().into_iter().flatten().filter_map(Value::as_str)); names
}
pub fn bonus_name(name: &str) -> bool { let s: String = name.nfkc().collect(); Regex::new(r"特典|アンケート|音声データ|ボイスデータ|ドラマCD|サウンドトラック|設定資料|壁紙").unwrap().is_match(&s) }
/// A local remake ("下級生リメイク") identified through a VNDB release that VNDB files under the original work.
/// Returns that release (lowest id among equally named ones) so the remake can have its own card.
pub fn remake_release(input: &Input, candidate: &Value) -> Option<Value> {
    if candidate["match"]["reason"] != "exact_release" { return None; }
    let remake = |t: &str| { let n = normalized(t); n.contains("りめいく") || n.contains("remake") };
    if !input.titles.iter().any(|t| remake(t)) { return None; }
    let own = ["title", "alttitle"].iter().filter_map(|k| candidate[*k].as_str()).chain(candidate["aliases"].as_array().into_iter().flatten().filter_map(Value::as_str)).any(remake);
    if own { return None; }
    let id = |r: &Value| r["id"].as_str().and_then(|s| s.trim_start_matches('r').parse::<u64>().ok()).unwrap_or(u64::MAX);
    candidate["release_matches"].as_array()?.iter()
        .filter(|r| r["title"].as_str().is_some_and(|t| remake(t) && input.titles.iter().any(|q| normalized(q) == normalized(&clean_name(t)))))
        .min_by_key(|r| id(r)).cloned()
}
/// Card metadata for a remake release: its own title, date, producers and package art; the rest stays with the work.
pub fn release_card(work: &Value, release: &Value) -> Value {
    let original = release["alttitle"].as_str().or(release["title"].as_str()).map(clean_name).unwrap_or_default();
    let title = release["release_title"].as_str().or(release["title"].as_str()).map(|t| main_title(t).to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| original.clone());
    let image = release["images"].as_array().and_then(|a| ["pkgfront", "dig", "pkgback"].iter().find_map(|kind| a.iter().find(|i| i["type"] == *kind)).or(a.first())).and_then(|i| i["url"].as_str());
    let producers: Vec<&str> = release["producers"].as_array().map(|a| a.iter().filter_map(|p| p["name"].as_str()).collect()).unwrap_or_default();
    json!({"title": title, "original_title": if original.is_empty() { title.clone() } else { original },
        "released": release["released"].as_str().unwrap_or(""), "developer": producers.join(", "),
        "cover": image.or(work["image"]["url"].as_str()).unwrap_or(""), "vndb_release": release["id"]})
}
/// The part of a provider title before its ～subtitle～ or -subtitle-.
pub fn main_title(name: &str) -> &str { Regex::new(r"\s*[～~〜]|\s+[-－]").unwrap().split(name).next().unwrap_or(name).trim() }
/// A brand or release date from the local names agrees with this candidate.
fn corroborated(input: &Input, candidate: &Value) -> bool {
    let names = |list: &Value| -> Vec<String> { list.as_array().map(|ds| ds.iter().flat_map(|d| [d["name"].as_str(), d["original"].as_str()]).flatten().map(normalized).collect()).unwrap_or_default() };
    let mut makers = names(&candidate["developers"]);
    let mut dates: Vec<&str> = candidate["released"].as_str().into_iter().collect();
    // A release found by title (remake, port, DL edition) carries its own producers and date.
    for release in candidate["release_matches"].as_array().into_iter().flatten() { makers.extend(names(&release["producers"])); dates.extend(release["released"].as_str()); }
    input.brands.iter().any(|b| { let b = normalized(b); !b.is_empty() && makers.contains(&b) })
        || dates.iter().any(|r| input.dates.iter().any(|d| d == r))
}
/// The folder names both a brand and a release date, and neither belongs to this candidate:
/// a same-named work from another maker (コイカツ！ by ILLUSION is not コイ☆カツ！ by Silhouette).
fn contradicted(input: &Input, candidate: &Value) -> bool { !input.brands.is_empty() && !input.dates.is_empty() && !corroborated(input, candidate) }
pub fn prepare(query: &str, hints: &[String]) -> Result<Input, String> {
    if query.len() > 4096 || query.trim().is_empty() { return Err("Enter a title or VNDB work ID (up to 4096 bytes)".into()); }
    let direct = Regex::new(r"(?i)^(?:https://vndb\.org/)?(v[0-9]+)/*$").unwrap();
    if let Some(c) = direct.captures(query.trim()) { return Ok(Input { titles: vec![query.into()], queries: vec![], direct_id: Some(c[1].to_lowercase()), brands: vec![], dates: vec![] }); }
    let mut titles = Vec::new(); let mut seen = HashSet::new(); let (mut brands, mut dates) = (Vec::new(), Vec::new());
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
            labels(part, &mut brands, &mut dates);
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
    Ok(Input { titles, queries, direct_id: None, brands, dates })
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
    // An update archive such as "mamakano_ex_2" or "mamakanoex101" names the work plus a version.
    // Sequel digits stay ("haisonsyoujo2"); only a separated or 3+ digit version is dropped.
    let versionless=roman(&replace(query,r"(?i)(?:[_-](?:ver)?\d+|\d{3,})$",""));let nq=roman(query);let mut names=vec![];
    for key in ["title","alttitle"]{if let Some(s)=candidate[key].as_str(){names.push(s);}}
    if let Some(titles)=candidate["titles"].as_array(){for t in titles{if let Some(s)=t["latin"].as_str(){names.push(s);}}}
    names.into_iter().filter(|s|s.is_ascii()).any(|name|{
        let base=Regex::new(r"\s+[~～〜-]").unwrap().split(name).next().unwrap().trim();
        // "Mama x Kano EX" is written "mamakano" in filenames: a standalone x is a separator.
        let unseparated=|s:&str|roman(&replace(s,r"(?i)\s+x\s+"," "));
        [&nq,&versionless].into_iter().any(|q|*q==roman(name)||*q==roman(base)||*q==unseparated(name)||*q==unseparated(base))
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
            // Local names often omit the provider's ～subtitle～. The main title alone is not enough:
            // the folder's brand or release date must agree with this very candidate.
            let base = main_title(name);
            let main = !exact && base.len() < name.len() && nq.chars().count() >= 4 && normalized(base) == nq && corroborated(input, &candidate);
            if main { score = 99.0; why = "exact_main_title"; }
            let mismatch = !exact && numbers(query) != numbers(if main { base } else { name });
            if mismatch { score = score.min(42.0); why = "number_conflict"; }
            if score > best { best = score; reason = why; matched = name.clone(); conflict = mismatch; }
        } }
        let mut primary = false;
        if let Some(releases) = candidate["release_matches"].as_array() {
            for release in releases { if let Some(title) = release["title"].as_str() {
                if input.titles.iter().any(|q| normalized(q)==normalized(&clean_name(title))) && best < 100.0 {
                    // A compilation named after one of its works ("君が望む永遠 ～Enhanced Edition R～") belongs to that work.
                    // The rest of the name must not start with a number ("Foo 2 Box" is not an edition of "Foo"),
                    // except an anniversary ("10th Anniversary edition", "15周年").
                    let nr = normalized(&clean_name(title)); let anniversary = Regex::new(r"^\d+(?:st|nd|rd|th|周年)").unwrap();
                    let lead = names.iter().filter(|(_, alias)| !alias).map(|(n, _)| normalized(n)).any(|nn| nn.chars().count() >= 4 && nr.strip_prefix(nn.as_str()).is_some_and(|rest|
                        !rest.is_empty() && (!rest.starts_with(|c: char| c.is_ascii_digit()) || anniversary.is_match(rest))));
                    if best < 99.0 || lead { best = if lead { 100.0 } else { 99.0 }; reason="exact_release"; matched=title.into(); conflict=false; primary = lead; }
                }
            } }
        }
        if let Some(titles)=candidate["confirmed_titles"].as_array(){for title in titles.iter().filter_map(|v|v["title"].as_str()){
            if input.titles.iter().any(|q|normalized(q)==normalized(title))&&best<105.0 {best=105.0;reason="confirmed_title";matched=title.into();conflict=false;}
        }}
        if input.direct_id.as_deref() == Some(&id) { best = 110.0; reason = "direct_id"; }
        // A person's own confirmation or an explicit VNDB id outweighs folder labels.
        if best >= 90.0 && !matches!(reason, "confirmed_title" | "direct_id") && contradicted(input, &candidate) { best = 90.0; reason = "release_conflict"; }
        candidate["match"] = json!({"score":best.round(),"reason":reason,"matched_title":matched,"number_conflict":conflict,"strength":"review"});
        if primary && reason == "exact_release" { candidate["match"]["primary"] = json!(true); }
        ranked.push(candidate);
    }
    ranked.sort_by(|a,b| b["match"]["score"].as_f64().unwrap().total_cmp(&a["match"]["score"].as_f64().unwrap()));
    // An exact title outranks another work matched only through its main title (e.g. the sequel "X -Prelude to War-").
    let exact_first = ranked.first().is_some_and(|c| c["match"]["reason"].as_str().is_some_and(|r| r.starts_with("exact") && r != "exact_main_title"));
    // The work a compilation is named after outranks the other works on that same release.
    let primary_first = ranked.first().is_some_and(|c| c["match"]["primary"] == true);
    let sibling = |c: &Value| primary_first && c["match"]["reason"] == "exact_release" && c["match"]["primary"] != true;
    let runner_up = ranked.iter().skip(1).filter(|c| !(exact_first && c["match"]["reason"] == "exact_main_title") && !sibling(c)).find_map(|c| c["match"]["score"].as_f64()).unwrap_or(0.0);
    if let Some(first) = ranked.first_mut() {
        let score = first["match"]["score"].as_f64().unwrap();
        let short = input.titles.iter().any(|t| normalized(&branded(t).map(|(_,tail)|tail).unwrap_or_else(||t.clone())).chars().count() < 4);
        if score >= 98.0 && score - runner_up >= 8.0 && (!short || input.direct_id.is_some()) { first["match"]["strength"] = json!("strong"); }
        if score >= 98.0 && score - runner_up < 8.0 { first["match"]["ambiguous"] = json!(true); }
    }
    ranked.truncate(20); ranked
}

/// VNDB Kana base URL. GALROON_VNDB_API overrides it so acceptance tests can inject provider outages.
pub fn vndb_api() -> String { std::env::var("GALROON_VNDB_API").ok().filter(|v|!v.is_empty()).unwrap_or_else(||"https://api.vndb.org/kana".into()).trim_end_matches('/').to_string() }
struct Provider { client: reqwest::Client, cache: HashMap<String, (Instant,Value)>, last: Option<Instant> }
static PROVIDER: OnceLock<tokio::sync::Mutex<Provider>> = OnceLock::new();
pub(crate) async fn known_artwork(url:&str)->bool{let Some(provider)=PROVIDER.get()else{return false;};provider.lock().await.cache.values().any(|(_,data)|crate::artwork::referenced(data,url))}
pub async fn search(input: Input) -> Result<Value,String> {
    let shared = PROVIDER.get_or_init(|| tokio::sync::Mutex::new(Provider { client:reqwest::Client::new(), cache:HashMap::new(), last:None }));
    let mut provider = shared.try_lock().map_err(|_| "Another VNDB search is running. Try again shortly.".to_string())?;
    retrieve(&input, &mut provider, &format!("{}/vn",vndb_api()), Duration::from_millis(1600)).await
}
const WORK_FIELDS: &str = "title,alttitle,titles.title,titles.latin,aliases,description,image.url,released,developers.name,developers.original,tags.name";
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
// VNDB compares `id` with one value; several ids must be OR-ed.
fn id_filter(ids:&[String])->Value{if let [id]=ids{json!(["id","=",id])}else{let mut f=vec![json!("or")];f.extend(ids.iter().map(|id|json!(["id","=",id])));Value::Array(f)}}
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
            // A full page (VNDB's maximum): popular names have dozens of editions, and a partial page forces review.
            let body=json!({"filters":["search","=",query],"sort":"searchrank","fields":"title,alttitle,vns.id,released,producers.name,producers.original,producers.developer,producers.publisher,images.url,images.type","results":100});
            let releases=match request(provider,&release_url,body,spacing).await {
                Ok(data)=>data,Err(message)=>{if candidates.is_empty(){return Err(message);}warning=Some(message);incomplete=true;break;}
            };
            incomplete|=releases["more"].as_bool().unwrap_or(false);
            let mut links:HashMap<String,Vec<Value>>=HashMap::new();
            for release in releases["results"].as_array().unwrap() {
                let exact=["title","alttitle"].iter().filter_map(|k|release[k].as_str()).find(|title|input.titles.iter().any(|q|normalized(q)==normalized(&clean_name(title))));
                if let Some(title)=exact { if let Some(vns)=release["vns"].as_array() { for vn in vns.iter().take(30) { if let Some(id)=vn["id"].as_str() {
                    links.entry(id.into()).or_default().push(json!({"id":release["id"],"title":title,"release_title":release["title"],"alttitle":release["alttitle"],"released":release["released"],"producers":release["producers"],"images":release["images"]}));
                } } } }
            }
            // A compilation release can point to multiple works: keep the ambiguity visible.
            let ids:Vec<String>=links.keys().filter(|id|!candidates.iter().any(|c|c["id"].as_str()==Some(id.as_str()))).cloned().collect();
            if !ids.is_empty() {
                let body=json!({"filters":id_filter(&ids[..ids.len().min(30)]),"fields":WORK_FIELDS,"results":30});
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
    #[test] fn bonus_crack_and_code_members_add_no_competing_title(){
        // Real member names from a NAS library; `names` are the matched work's and the folder's own names.
        let amatsu=["アマツツミ","Amatsutsumi"];
        for member in ["Bonus","修正プログラム","Wallpaper","AlphaROM Cracker x2","UchiAruji Crak","FlipRun CrakV2","APT-183","has_0116p","09809cb540551d3e97cbfa174be33bed","patch_ss_101","tsubaki_wallpaper","特典データ",
            "アマツツミ 同梱特典 アマツツミサウンドトラック","アマツツミ ヘッダー & アイコン","(FN輸入) (18禁ゲーム) [191025] [戯画] アマツツミ","アマツツミ (mdf+mds+rr3) ※自炊","『アマツツミ』パッチver1.0.1","アマツツミ + Original Soundtrack + Song Collection"] {
            assert!(ancillary_member(member,&amatsu),"{member}");
        }
        for member in ["エッチな皇女バニーのご奉仕","アマツツミ2 特典","アマツツミ ファンディスク","Song of Saya","Boku Sensei Crak","オリジナルデジタルコンテンツ『美少女探偵アリアンナ』"] {
            assert!(!ancillary_member(member,&amatsu),"{member}");
        }
        assert!(names_with_extras("恋愛×ロワイアル + Bonus",&["恋愛×ロワイアル"]));
        assert!(names_with_extras("癒しの女神の実験台(モルモット) ヘッダー&アイコン",&["癒しの女神の実験台（モルモット）"]));
        // The resource's own name never passes on bonus words or a code name alone.
        assert!(!names_with_extras("Bonus",&amatsu));assert!(!names_with_extras("KOIROWA_AS1",&amatsu));
    }
    #[test] fn ancillary_names_do_not_become_competing_titles_or_sequel_numbers(){
        for (query,hints,expected) in [
            ("マガルミナ",vec!["マガルミナ/magalumina_patch_v1_10.zip"],vec!["マガルミナ","magalumina"]),
            ("宿りし乙女の誓いと魔法",vec!["1326550/宿りし乙女の誓いと魔法 修正パッチVer1.01 (files).rar"],vec!["宿りし乙女の誓いと魔法"]),
            ("Actual Title",vec!["Actual Title/Sofmap/Sofmap.wav","Actual Title/ysbin/yst00000.ybn","Actual Title/updchk/bn.ypf","Actual Title/INH450.mds","Actual Title/html/images/colorbox/ie/border.png"],vec!["Actual Title"]),
            ("ダウニャーさんと飼い主くん",vec!["ダウニャーさんと飼い主くん/ダウニャーさんと飼い主くん_特典ASMR/wave/03.Track.wav","ダウニャーさんと飼い主くん_主題歌マキシ音源"],vec!["ダウニャーさんと飼い主くん"]),
        ]{let h:Vec<_>=hints.into_iter().map(str::to_string).collect();assert_eq!(prepare(query,&h).unwrap().titles,expected);}
        assert_eq!(clean_name("Game 2 -After Story-"),"Game 2 -After Story-");
        assert_eq!(clean_name("[230428] [あざらしそふと] アマカノ2 ＋ + Tokuten Voice"),"アマカノ2+");assert_eq!(clean_name("[200424] [あざらしそふと] アマカノ2 + Bonus + Update 1.1"),"アマカノ2");
        assert_eq!(clean_name("[240830][1278460][あざらしそふと] アマナツ＋ DL版 (files)"),"アマナツ+");
    }
    #[test] fn a_plus_suffix_names_a_different_work(){
        let works=||vec![json!({"id":"v34848","title":"Amanatsu","alttitle":"アマナツ","released":"2022-08-26","developers":[{"name":"あざらしそふと"}]}),json!({"id":"v47551","title":"Amanatsu+","alttitle":"アマナツ＋","released":"2024-04-12","developers":[{"name":"あざらしそふと"}]})];
        for (folder,id) in [("[220826][1178934][あざらしそふと] アマナツ DL版 (files).rar","v34848"),("[240830][1278460][あざらしそふと] アマナツ＋ DL版 (files)","v47551")]{
            let r=rank(&prepare(folder,&[]).unwrap(),works());assert_eq!(r[0]["id"],id);assert_eq!(r[0]["match"]["strength"],"strong","{folder}");
        }
        let amakano=||vec![json!({"id":"v26307","title":"Amakano 2","alttitle":"アマカノ２","released":"2020-04-24","developers":[{"name":"あざらしそふと"}]}),json!({"id":"v31813","title":"Amakano 2+","alttitle":"アマカノ２＋","released":"2023-04-28","developers":[{"name":"あざらしそふと"}]})];
        for (folder,id) in [("[200424] [あざらしそふと] アマカノ2 + Bonus + Update 1.1","v26307"),("[230428] [あざらしそふと] アマカノ2 ＋ + Tokuten Voice","v31813"),("アマカノ2+ ソフマップ特典","v31813")]{
            let r=rank(&prepare(folder,&[]).unwrap(),amakano());assert_eq!(r[0]["id"],id,"{folder}");assert_eq!(r[0]["match"]["strength"],"strong","{folder}");
        }
        assert!(prepare("Actual Title",&["Other Game/data.xp3".into()]).unwrap().titles.contains(&"Other Game".into()));
    }
    #[test] fn compact_romanized_filenames_only_corroborate_the_same_full_identity(){
        let c=json!({"id":"v53486","title":"Haison Shoujo [Ni] ~Kageri Sasou Hime no Hako~"});
        assert!(corroborates_filename("haisonsyoujo2",&c));
        for wrong in ["haisonsyoujo","haisonsyoujo3","haisonsyoujo2afterstory"]{assert!(!corroborates_filename(wrong,&c));}
        assert_ne!(rank(&prepare("haisonsyoujo2",&[]).unwrap(),vec![c])[0]["match"]["strength"],"strong");
    }
    fn candidate(id:&str,name:&str)->Value { json!({"id":id,"title":name}) }
    #[test] fn main_title_needs_brand_or_date_corroboration() {
        let mama = json!({"id":"v59911","title":"Mama x Kano EX ~Ryoushu~","alttitle":"ママ×カノEX ～領主貴族に嫁ぎたくない娘の為に～","released":"2025-12-19","developers":[{"name":"Shibasoft","original":"しばそふと"}]});
        let input = prepare("ママ×カノEX 豪華限定版 + Vocal CD + Update", &["[251219] [しばそふと] ママ×カノEX 豪華限定版 + Vocal CD + Update".into()]).unwrap();
        assert_eq!(input.brands, vec!["しばそふと"]); assert_eq!(input.dates, vec!["2025-12-19"]);
        let r = rank(&input, vec![mama.clone()]); assert_eq!(r[0]["match"]["reason"], "exact_main_title"); assert_eq!(r[0]["match"]["strength"], "strong");
        // Same name without brand or date evidence stays a review case.
        let bare = prepare("ママ×カノEX", &[]).unwrap(); assert_eq!(rank(&bare, vec![mama.clone()])[0]["match"]["strength"], "review");
        // A different brand and date do not corroborate.
        let other = prepare("x", &["[240101] [Other] ママ×カノEX".into()]).unwrap(); let mut o = other.clone(); o.titles = vec!["ママ×カノEX".into()];
        assert_eq!(rank(&o, vec![mama.clone()])[0]["match"]["strength"], "review");
        // Two series entries sharing the main title and brand are ambiguous, never strong.
        let twin = json!({"id":"v2","title":"ママ×カノEX ～別の話～","released":"2026-01-01","developers":[{"name":"Shibasoft","original":"しばそふと"}]});
        let brand_only = prepare("x", &["[しばそふと] [x] ママ×カノEX".into()]).unwrap(); let mut b = brand_only.clone(); b.titles = vec!["ママ×カノEX".into()];
        let r = rank(&b, vec![mama.clone(), twin]); assert_ne!(r[0]["match"]["strength"], "strong");
        // Sequel numbers still conflict.
        let sequel = json!({"id":"v3","title":"猫忍えくすはーとSPIN! 3 ～x～","released":"2025-11-28","developers":[{"name":"Whirlpool"}]});
        let two = prepare("x", &["[251128] [Whirlpool] 猫忍えくすはーとSPIN! 2".into()]).unwrap(); let mut t = two.clone(); t.titles = vec!["猫忍えくすはーとSPIN! 2".into()];
        assert_ne!(rank(&t, vec![sequel])[0]["match"]["strength"], "strong");
        // An exact title beats a sibling that only shares the main title.
        let unravel = prepare("x", &["[240329][1261651][Archive] アンラベル・トリガー DL版 (files).rar".into()]).unwrap(); let mut u = unravel.clone(); u.titles = vec!["アンラベル・トリガー".into()];
        let r = rank(&u, vec![json!({"id":"v55273","title":"アンラベル・トリガー -Prelude to War-","released":"2025-07-25","developers":[{"name":"Archive"}]}), json!({"id":"v47547","title":"アンラベル・トリガー","released":"2024-03-29","developers":[{"name":"Archive"}]})]);
        assert_eq!(r[0]["id"], "v47547"); assert_eq!(r[0]["match"]["strength"], "strong");
        // Update archives corroborate the romanized title.
        assert!(corroborates_filename("mamakano_ex_2", &mama)); assert!(corroborates_filename("mamakanoex101", &mama));
    }
    #[test] fn bonus_archives_reduce_to_their_work_and_short_titles_stay_review() {
        assert_eq!(clean_name("[240830][1297042][WAFFLE] 善悪 フルHDリマスター版 Webアンケート特典 (files).rar"), "善悪");
        assert_eq!(clean_name("[241025][1297034][WAFFLE] 恋愛弱者な幼馴染少女と恋愛強者な彼女 エッチな音声データ (ogg).rar"), "恋愛弱者な幼馴染少女と恋愛強者な彼女");
        assert_eq!(clean_name("[241025][1305021][WAFFLE] 奴隷姫騎士と奴隷侍女とのスローライフ アニメ追加版 オフィシャル通販特典 「奴隷たちとの畑づくり」 (files).rar"), "奴隷姫騎士と奴隷侍女とのスローライフ");
        assert!(bonus_name("善悪 フルHDリマスター版 Webアンケート特典 (files)"));
        let zenaku = json!({"id":"v20672","title":"Zen'aku","alttitle":"善悪","released":"2017-07-28","developers":[{"name":"Waffle"}]});
        // Short titles stay review cases even when the brand agrees (same rule as "[Key] AIR").
        let with_brand = prepare("善悪 フルHDリマスター版 Webアンケート特典 (files)", &["[240830][1297042][WAFFLE] 善悪 フルHDリマスター版 Webアンケート特典 (files).rar".into()]).unwrap();
        assert_eq!(rank(&with_brand, vec![zenaku])[0]["match"]["strength"], "review");
    }
    #[test] fn same_name_from_another_maker_and_year_is_not_admitted() {
        let silhouette = json!({"id":"v6366","title":"Koi☆Katsu!","alttitle":"コイ☆カツ！","released":"2011-02-25","developers":[{"name":"Silhouette"}]});
        let input = prepare("コイカツ！ 予約特典 魔法少女衣装データ", &["(18禁ゲーム) [180427] [ILLUSION] コイカツ！ 予約特典 魔法少女衣装データ.rar".into()]).unwrap();
        let r = rank(&input, vec![silhouette.clone()]); assert_eq!(r[0]["match"]["reason"], "release_conflict"); assert_ne!(r[0]["match"]["strength"], "strong");
        // Either the brand or the date agreeing keeps an exact title admissible.
        let same_brand = prepare("x", &["[180427] [Silhouette] コイ☆カツ！".into()]).unwrap(); let mut b = same_brand.clone(); b.titles = vec!["コイ☆カツ！".into()];
        assert_eq!(rank(&b, vec![silhouette])[0]["match"]["strength"], "strong");
    }
    #[test] fn remakes_found_through_a_release_get_that_release() {
        let input = prepare("下級生リメイク 豪華版", &["[251219] [FG REMAKE] 下級生リメイク 豪華版 + Voice Drama + Tokuten".into()]).unwrap();
        let release = |id: &str, alt: &str| json!({"id":id,"title":alt,"release_title":"Kakyuusei Remake - Package Edition","alttitle":alt,"released":"2025-12-19","producers":[{"name":"FG Remake"}],"images":[{"type":"dig","url":"https://t.vndb.org/cv/83/115383.jpg"}]});
        let work = json!({"id":"v2341","title":"Kakyuusei","alttitle":"下級生","image":{"url":"old.jpg"},"release_matches":[release("r163685","下級生リメイク ダウンロード豪華版"),release("r140695","下級生リメイク パッケージ版")]});
        let ranked = rank(&input, vec![work]); assert_eq!(ranked[0]["match"]["reason"], "exact_release");
        let r = remake_release(&input, &ranked[0]).unwrap(); assert_eq!(r["id"], "r140695");
        let card = release_card(&ranked[0], &r);
        assert_eq!(card["original_title"], "下級生リメイク"); assert_eq!(card["title"], "Kakyuusei Remake"); assert_eq!(card["released"], "2025-12-19"); assert_eq!(card["developer"], "FG Remake"); assert_eq!(card["cover"], "https://t.vndb.org/cv/83/115383.jpg");
        // The original title alone is not a remake.
        assert!(remake_release(&prepare("下級生", &[]).unwrap(), &ranked[0]).is_none());
    }
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
    #[test] fn bundled_extras_after_plus_signs_are_dropped_but_joined_titles_stay() {
        for (raw,want) in [
            ("[150227] [ゆずソフト] サノバウィッチ ‐SABBAT OF THE WITCH‐ + Drama CD + Character Songs + Bonus + Manual + Update 1.1", "サノバウィッチ ‐SABBAT OF THE WITCH‐"),
            ("[160729] [ゆずソフト] 千恋＊万花 + Character Songs + Drama CD + Bonus + Manual + Wallpaper + Update 1.1", "千恋*万花"),
            ("[181130] [sprite] 蒼の彼方のフォーリズム Perfect Edition + Manual", "蒼の彼方のフォーリズム Perfect Edition"),
            ("[201127] [ALICESOFT] ドーナドーナ いっしょにわるいことをしよう + Sound Album", "ドーナドーナ いっしょにわるいことをしよう"),
            ("[171124] [あかべぇそふとすりぃ] まほ×ろば + Clothes Transparent Patch + Drama CD + Manual", "まほ×ろば"),
            ("[210226][1114110][アストロノーツ・シリウス] 極限痴漢特異点2 痴漢の証明 + オリジサントラ + ハーレムパッチ", "極限痴漢特異点2 痴漢の証明"),
            ("[260925] [ASa Project] 王様恋愛 豪華限定版 + OST + Voice Drama + Tokuten x6 + Manual + Wallpaper", "王様恋愛"),
            ("桜花裁き 初回限定特装版 + 予約特典 + 初回特典 + Sofmap特典 + レーベル", "桜花裁き"),
            ("ゆらめく心に満ちた世界で、君の夢と欲望は叶うか + OST + 解放パッチ", "ゆらめく心に満ちた世界で、君の夢と欲望は叶うか"),
            ("『girlcelly』 [260101] [ぱんみみそふと] おにいちゃんコンティニュー！　ゆうりとぷらすめいくらぶ♥", "おにいちゃんコンティニュー! ゆうりとぷらすめいくらぶ♥"),
            ("│2D.G.F.│[251226][1338954][枕] サクラノ詩 〜櫻の森の上を舞う〜 10th Anniversary Edition DL版 [8690MB]", "サクラノ詩 〜櫻の森の上を舞う〜 10th Anniversary Edition"),
            ("(FN輸入) (18禁ゲーム) [200424] [あざらしそふと] アマカノ2", "アマカノ2"),
            ("[231215][1132424][WAFFLE] 奴隷姫騎士と奴隷侍女とのスローライフ 多国語版 Chinese-English DL版 (files).rar", "奴隷姫騎士と奴隷侍女とのスローライフ"),
            // A " + " that joins real titles or adds story content is identity, not an extra.
            ("(同人ゲーム) [2021-12-24][RJ351325][ディーゼルマイン] SUMMER-田舎の性活- +おでかけ", "SUMMER-田舎の性活- +おでかけ"),
            ("Fate/stay night + hollow ataraxia", "Fate/stay night + hollow ataraxia"),
            ("[211029] [ま～まれぇど] PRIMAL×HEARTS 1＋2パック", "PRIMAL×HEARTS 1+2パック"),
            ("『虜ノ誓』追加パッチ「男だらけの運動会パッチ」", "『虜ノ誓』追加パッチ「男だらけの運動会パッチ」"),
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
    #[test] fn a_compilation_named_after_one_of_its_works_belongs_to_that_work(){
        let on=|id:&str,title:&str,release:&str|json!({"id":id,"title":title,"release_matches":[{"id":"r141029","title":release}]});
        let release="君が望む永遠 ～Enhanced Edition R～ - 初回生産版";let input=prepare("君が望む永遠 ～Enhanced Edition R～ 初回生産版 + Tokuten",&[]).unwrap();
        let r=rank(&input,vec![on("v358","君が望む永遠 ～special FanDisk～",release),on("v94","君が望む永遠",release),on("v28307","君が望む永遠 another episode 悲しみは風のように",release)]);
        assert_eq!(r[0]["id"],"v94");assert_eq!(r[0]["match"]["strength"],"strong");assert_eq!(r[0]["match"]["reason"],"exact_release");
        // A pure box set names none of its works, and "Foo 2 Box" is not an edition of "Foo".
        let r=rank(&prepare("キスイロ",&[]).unwrap(),vec![on("v10641","キスベル","キスイロ"),on("v13632","キスアト","キスイロ")]);assert_eq!(r[0]["match"]["strength"],"review");assert_eq!(r[0]["match"]["ambiguous"],true);
        let r=rank(&prepare("Sample Title 2 Twin Box",&[]).unwrap(),vec![on("v1","Sample Title","Sample Title 2 Twin Box"),on("v2","Other Work","Sample Title 2 Twin Box")]);assert_eq!(r[0]["match"]["strength"],"review");
        let r=rank(&prepare("サクラノ詩－櫻の森の上を舞う－10th Anniversary edition",&[]).unwrap(),vec![on("v562","サクラノ詩－櫻の森の上を舞う－","サクラノ詩－櫻の森の上を舞う－10th Anniversary edition"),on("v60562","サクラノ詩 Re:Graphic アペンドディスク","サクラノ詩－櫻の森の上を舞う－10th Anniversary edition")]);assert_eq!(r[0]["id"],"v562");assert_eq!(r[0]["match"]["strength"],"strong");
        // Two works that both lead the name leave the choice to the person.
        let r=rank(&prepare("Sample Title Plus Complete",&[]).unwrap(),vec![on("v1","Sample Title","Sample Title Plus Complete"),on("v2","Sample Title Plus","Sample Title Plus Complete")]);assert_eq!(r[0]["match"]["strength"],"review");
    }
    #[tokio::test] async fn release_identity_is_followed_and_compilations_stay_ambiguous() {
        use axum::{routing::post,Json,Router};
        let app=Router::new().route("/vn",post(|Json(v):Json<Value>|async move {
            // Mirrors VNDB: `id` takes one string; several ids arrive OR-ed, anything else is a 400.
            let ids=|f:&Value|f[0]=="id"&&f[1]=="="&&f[2].is_string();
            let f=&v["filters"];
            if ids(f)||(f[0]=="or"&&f.as_array().unwrap()[1..].iter().all(ids)) {(axum::http::StatusCode::OK,Json(json!({"results":[{"id":"v1","title":"Original name"},{"id":"v2","title":"Other work"}],"more":false})))}
            else if f[0]=="search" {(axum::http::StatusCode::OK,Json(json!({"results":[],"more":false})))}
            else {(axum::http::StatusCode::BAD_REQUEST,Json(json!({})))}
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
        let t=tempfile::tempdir().unwrap();let app=crate::initialize(t.path().join("state")).unwrap();app.db.lock().unwrap().execute("INSERT INTO settings(key,value) VALUES('auto_match_enabled','0')",[]).unwrap();
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

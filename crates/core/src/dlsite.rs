//! Bounded DLsite product-code (VJ/RJ) title lookup for bare store-ID folders.
//! Never guesses or fabricates titles; any network, HTTP or parse failure falls back to review.
use regex::Regex;
use serde_json::Value;
use std::{collections::HashMap,sync::OnceLock,time::{Duration,Instant}};

const DEFAULT_API:&str="https://www.dlsite.com";
const MAX_BODY_BYTES:usize=512*1024;
const CACHE_TTL:Duration=Duration::from_secs(600);
const CACHE_LIMIT:usize=64;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Product{pub code:String,pub title:String,pub maker:Option<String>}

/// Canonical uppercase DLsite product code (`VJ` or `RJ` + 6-8 digits).
pub fn valid_code(code:&str)->bool{
    static RE:OnceLock<Regex>=OnceLock::new();
    RE.get_or_init(||Regex::new(r"^(?:VJ|RJ)\d{6,8}$").unwrap()).is_match(code)
}

/// Extract distinct DLsite product codes (`VJ`/`RJ` + 6-8 digits) from text in uppercase.
pub fn extract_codes(text:&str)->Vec<String>{
    static RE:OnceLock<Regex>=OnceLock::new();
    let re=RE.get_or_init(||Regex::new(r"(?i)(?:^|[^A-Za-z0-9])((?:VJ|RJ)\d{6,8})(?:$|[^A-Za-z0-9])").unwrap());
    let mut out=Vec::new();
    for caps in re.captures_iter(text){
        let code=caps[1].to_ascii_uppercase();
        if !out.contains(&code){out.push(code);}
    }
    out
}

/// DLsite API base URL. `GALROON_DLSITE_API` overrides it for deterministic tests.
/// Only HTTPS origins or loopback HTTP origins (for local mock servers) are accepted.
pub fn api_base()->Result<String,String>{
    match std::env::var("GALROON_DLSITE_API").ok().map(|s|s.trim().to_string()).filter(|s|!s.is_empty()){
        None=>Ok(DEFAULT_API.into()),
        Some(raw)=>validate_base_url(&raw),
    }
}

pub fn validate_base_url(raw:&str)->Result<String,String>{
    let trimmed=raw.trim().trim_end_matches('/');
    let url=reqwest::Url::parse(trimmed).map_err(|_|"Invalid DLsite API URL".to_string())?;
    if !url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some(){
        return Err("DLsite API URL must not contain credentials, query or fragment".into());
    }
    let host=url.host_str().ok_or("DLsite API URL is missing host")?;
    let loopback=matches!(host,"127.0.0.1"|"localhost"|"[::1]")||host.trim_matches(['[',']']).parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback());
    match url.scheme(){
        "https"=>{}
        "http" if loopback=>{}
        _=>return Err("DLsite API URL must use HTTPS (or loopback HTTP for tests)".into()),
    }
    let path=url.path().trim_end_matches('/');
    let port=url.port().map(|p|format!(":{p}")).unwrap_or_default();
    Ok(format!("{}://{host}{port}{path}",url.scheme()))
}

struct ClientState{client:reqwest::Client,cache:HashMap<(String,String),(Instant,Result<Product,String>)>}
static CLIENT:OnceLock<tokio::sync::Mutex<ClientState>>=OnceLock::new();

fn http_client()->Result<reqwest::Client,String>{
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(6))
        .user_agent(concat!("Galroon/",env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e|e.to_string())
}

fn sections(code:&str)->&'static [&'static str]{
    if code.starts_with("VJ"){&["pro","soft"]}else{&["maniax","home"]}
}

/// Look up a DLsite product title for a validated `VJ`/`RJ` code.
pub async fn lookup(code:&str)->Result<Product,String>{
    let code=code.trim().to_ascii_uppercase();
    if !valid_code(&code){return Err("Invalid DLsite product code".into());}
    let base=api_base()?;
    lookup_at(&base,&code).await
}

pub async fn lookup_at(base:&str,code:&str)->Result<Product,String>{
    let code=code.trim().to_ascii_uppercase();
    if !valid_code(&code){return Err("Invalid DLsite product code".into());}
    let base=validate_base_url(base)?;
    let key=(base.clone(),code.clone());
    let shared=CLIENT.get_or_init(||tokio::sync::Mutex::new(ClientState{client:http_client().expect("dlsite http client"),cache:HashMap::new()}));
    let client={
        let mut state=shared.lock().await;
        state.cache.retain(|_,(at,_)|at.elapsed()<CACHE_TTL);
        if let Some((_,cached))=state.cache.get(&key){return cached.clone();}
        state.client.clone()
    };
    let result=fetch_product(&client,&base,&code).await;
    let mut state=shared.lock().await;
    if state.cache.len()>=CACHE_LIMIT{state.cache.clear();}
    state.cache.insert(key,(Instant::now(),result.clone()));
    result
}

async fn fetch_product(client:&reqwest::Client,base:&str,code:&str)->Result<Product,String>{
    let mut last_error=None;
    for section in sections(code){
        let url=format!("{base}/{section}/api/=/product.json?workno={code}");
        let response=match client.get(&url).send().await{
            Ok(r)=>r,
            Err(e)=>{
                let msg=if e.is_timeout(){"DLsite request timed out"}else{"DLsite could not be reached"};
                return Err(format!("{msg} for {code}"));
            }
        };
        let status=response.status();
        if status==reqwest::StatusCode::NOT_FOUND{
            last_error=Some(format!("DLsite has no record for {code}"));
            continue;
        }
        if !status.is_success(){
            return Err(format!("DLsite returned HTTP {status} for {code}"));
        }
        let body=read_bounded(response,MAX_BODY_BYTES).await?;
        let value:Value=serde_json::from_slice(&body).map_err(|_|format!("DLsite returned unreadable JSON for {code}"))?;
        match parse_product(&value,code)?{
            Some(product)=>return Ok(product),
            None=>{last_error=Some(format!("DLsite has no title for {code}"));}
        }
    }
    Err(last_error.unwrap_or_else(||format!("DLsite has no title for {code}")))
}

async fn read_bounded(mut response:reqwest::Response,limit:usize)->Result<Vec<u8>,String>{
    if response.content_length().is_some_and(|n|n>limit as u64){return Err("DLsite response exceeds size limit".into());}
    let mut body=Vec::with_capacity(response.content_length().unwrap_or(0).min(limit as u64) as usize);
    while let Some(chunk)=response.chunk().await.map_err(|_|"DLsite response stream failed".to_string())?{
        if chunk.len()>limit-body.len(){return Err("DLsite response exceeds size limit".into());}
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_product(value:&Value,code:&str)->Result<Option<Product>,String>{
    let item=match value{
        Value::Array(items)=>{
            if items.is_empty(){return Ok(None);}
            // If multiple entries appear, require an exact workno match.
            if let Some(exact)=items.iter().find(|entry|entry["workno"].as_str().is_some_and(|w|w.eq_ignore_ascii_case(code))){exact}
            else if items.len()==1{&items[0]}
            else{return Err(format!("DLsite returned ambiguous entries for {code}"));}
        }
        Value::Object(_)=>value,
        _=>return Err(format!("DLsite returned unexpected payload for {code}")),
    };
    if let Some(returned)=item["workno"].as_str().filter(|s|!s.trim().is_empty()){
        if !returned.trim().eq_ignore_ascii_case(code){
            return Err(format!("DLsite product code mismatch: expected {code}, got {returned}"));
        }
    }
    let title=item["work_name"].as_str().map(str::trim).filter(|s|!s.is_empty()&&s.len()<=512&&!s.chars().any(|c|c.is_control()));
    let Some(title)=title else{return Ok(None);};
    let maker=item["maker_name"].as_str().map(str::trim).filter(|s|!s.is_empty()&&s.len()<=256&&!s.chars().any(|c|c.is_control())).map(str::to_owned);
    Ok(Some(Product{code:code.to_string(),title:title.to_string(),maker}))
}

#[cfg(test)]mod tests{
    use super::*;
    use axum::{extract::Query,http::StatusCode,routing::get,Json,Router};
    use serde_json::json;

    #[test]fn extracts_and_validates_codes_strictly(){
        assert_eq!(extract_codes("VJ01001192"),vec!["VJ01001192"]);
        assert_eq!(extract_codes("[231222][vj01001192][Clover GAME] title"),vec!["VJ01001192"]);
        assert_eq!(extract_codes("RJ123456/sub/RJ123456"),vec!["RJ123456"]);
        assert_eq!(extract_codes("VJ01001192 and RJ12345678"),vec!["VJ01001192","RJ12345678"]);
        for bad in ["","1368659","VJ12345","VJ123456789","AVJ01001192","VJ01001192X","DMM123456","v48419"]{
            assert!(extract_codes(bad).is_empty(),"{bad}");
            assert!(!valid_code(bad),"{bad}");
        }
        assert!(validate_base_url("https://www.dlsite.com/").is_ok());
        assert!(validate_base_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_base_url("http://localhost:8080/mock/").is_ok());
        for bad in ["http://www.dlsite.com","http://192.168.1.2:80","https://user:pass@www.dlsite.com","https://www.dlsite.com?x=1","ftp://www.dlsite.com"]{
            assert!(validate_base_url(bad).is_err(),"{bad}");
        }
    }

    #[tokio::test]async fn mock_server_resolves_vj_and_rj_and_rejects_mismatches(){
        let app=Router::new()
            .route("/pro/api/=/product.json",get(|Query(q):Query<HashMap<String,String>>|async move{
                match q.get("workno").map(String::as_str){
                    Some("VJ01001192")=>(StatusCode::OK,Json(json!([{"workno":"VJ01001192","work_name":"こあくまちゃんの誘惑っ！","maker_name":"Clover GAME"}]))),
                    Some("VJ01004242")=>(StatusCode::OK,Json(json!([{"workno":"VJ01004242","work_name":"メイドちゃんは迷途ちゅう","maker_name":"Clover GAME"}]))),
                    Some("VJ00000001")=>(StatusCode::OK,Json(json!([{"workno":"VJ99999999","work_name":"Wrong work"}]))),
                    Some("VJ00000500")=>(StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"boom"}))),
                    _ =>(StatusCode::OK,Json(json!([]))),
                }
            }))
            .route("/soft/api/=/product.json",get(|Query(q):Query<HashMap<String,String>>|async move{
                if q.get("workno").map(String::as_str)==Some("VJ01009999"){
                    (StatusCode::OK,Json(json!([{"workno":"VJ01009999","work_name":"All Ages Title"}])))
                }else{(StatusCode::OK,Json(json!([])))}
            }))
            .route("/maniax/api/=/product.json",get(|Query(q):Query<HashMap<String,String>>|async move{
                if q.get("workno").map(String::as_str)==Some("RJ123456"){
                    (StatusCode::OK,Json(json!([{"workno":"RJ123456","work_name":"同人タイトル"}])))
                }else{(StatusCode::OK,Json(json!([])))}
            }))
            .route("/home/api/=/product.json",get(||async{(StatusCode::OK,Json(json!([])))}));
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base=format!("http://{}",listener.local_addr().unwrap());
        let server=tokio::spawn(async move{axum::serve(listener,app).await.unwrap();});

        let a=lookup_at(&base,"VJ01001192").await.unwrap();
        assert_eq!(a.title,"こあくまちゃんの誘惑っ！");
        assert_eq!(a.maker.as_deref(),Some("Clover GAME"));

        let b=lookup_at(&base,"vj01004242").await.unwrap();
        assert_eq!(b.code,"VJ01004242");
        assert_eq!(b.title,"メイドちゃんは迷途ちゅう");

        let c=lookup_at(&base,"VJ01009999").await.unwrap();
        assert_eq!(c.title,"All Ages Title");

        let d=lookup_at(&base,"RJ123456").await.unwrap();
        assert_eq!(d.title,"同人タイトル");

        // Mismatched workno, 500 error, and missing product all return Err (never false match).
        assert!(lookup_at(&base,"VJ00000001").await.unwrap_err().contains("mismatch"));
        assert!(lookup_at(&base,"VJ00000500").await.unwrap_err().contains("500"));
        assert!(lookup_at(&base,"VJ00000404").await.is_err());

        server.abort();
    }
}

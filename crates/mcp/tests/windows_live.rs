//! Native Windows acceptance: a real galroon-core.exe found through the control pipe (no --url),
//! a real source folder, VNDB matching and rediscovery after Core restarts on a new port.
//! Run: GALROON_TEST_CORE=<release galroon-core.exe> GALROON_TEST_SOURCE=<folder> GALROON_TEST_STATE=<empty dir>
//!      cargo test -p galroon-mcp --test windows_live -- --ignored --nocapture
#![cfg(windows)]
use galroon_core::local_core;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn var(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required"))).canonicalize().unwrap()
}

async fn owner(session: &local_core::Session, method: reqwest::Method, path: &str, body: Option<Value>) -> Value {
    let mut request = reqwest::Client::new().request(method, format!("{}/api{path}", session.url)).bearer_auth(&session.token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let value: Value = response.json().await.unwrap();
    assert!(status.is_success(), "{path}: {value}");
    value
}

/// Metadata only: the sources are gigabytes and organize previews must not touch them.
fn snapshot(dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        if meta.is_dir() {
            out.extend(snapshot(&entry.path()));
        } else {
            out.push((entry.path(), meta.len(), meta.modified().unwrap()));
        }
    }
    out.sort();
    out
}

struct Mcp {
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: i64,
}

impl Mcp {
    async fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.id += 1;
        let message = json!({"jsonrpc": "2.0", "id": self.id, "method": "tools/call", "params": {"name": name, "arguments": arguments}});
        self.stdin.write_all(format!("{message}\n").as_bytes()).await.unwrap();
        let line = tokio::time::timeout(Duration::from_secs(600), self.stdout.next_line()).await.unwrap().unwrap().unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("{response}")).to_owned();
        let value = serde_json::from_str(&text).unwrap_or(json!(text));
        if response["result"]["isError"] == true { Err(value.as_str().map(str::to_owned).unwrap_or(text)) } else { Ok(value) }
    }
}

#[tokio::test]
#[ignore]
async fn native_discovery_matching_and_restart() {
    let exe = var("GALROON_TEST_CORE");
    let source = var("GALROON_TEST_SOURCE");
    let base = var("GALROON_TEST_STATE");
    let mcp_exe = PathBuf::from(env!("CARGO_BIN_EXE_galroon-mcp"));
    let credential = base.join("mcp-credential.json");
    let env = [("GALROON_DATA", base.as_os_str()), ("GALROON_CORE_EXE", exe.as_os_str()), ("GALROON_MCP_TOKEN_FILE", credential.as_os_str())];
    let mut report = serde_json::Map::new();

    let session = local_core::connect_or_start(base.clone(), exe.clone()).await.unwrap();
    report.insert("first_url".into(), json!(session.url));
    if owner(&session, reqwest::Method::GET, "/access/status", None).await["configured"] != true {
        owner(&session, reqwest::Method::POST, "/access/setup", Some(json!({"password": "Generated sandbox owner password"}))).await;
    }
    let roots = owner(&session, reqwest::Method::GET, "/roots", None).await;
    if roots.as_array().unwrap().is_empty() {
        let root = owner(&session, reqwest::Method::POST, "/roots", Some(json!({"path": source, "label": "Sandbox"}))).await;
        let job = owner(&session, reqwest::Method::POST, "/scans", Some(json!({"root_id": root["id"], "scope": "", "exclude": []}))).await;
        loop {
            let detail = owner(&session, reqwest::Method::GET, &format!("/jobs/{}", job["id"].as_str().unwrap()), None).await;
            if !matches!(detail["state"].as_str(), Some("queued" | "running")) {
                report.insert("scan".into(), detail["summary"].clone());
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    let before = snapshot(&source);

    // Pairing and check go through pipe discovery: no --url anywhere below.
    let code = owner(&session, reqwest::Method::POST, "/access/pairing", Some(json!({"name": "MCP native acceptance"}))).await["code"].as_str().unwrap().to_owned();
    let paired = tokio::process::Command::new(&mcp_exe).args(["pair", &code]).envs(env).output().await.unwrap();
    assert!(paired.status.success(), "{}", String::from_utf8_lossy(&paired.stderr));
    let check = tokio::process::Command::new(&mcp_exe).arg("check").envs(env).output().await.unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));

    let mut child = tokio::process::Command::new(&mcp_exe).envs(env).stdin(Stdio::piped()).stdout(Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    let mut mcp = Mcp { stdin: child.stdin.take().unwrap(), stdout: BufReader::new(child.stdout.take().unwrap()).lines(), id: 0 };

    let overview = mcp.call("galroon_overview", json!({})).await.unwrap();
    report.insert("overview".into(), overview.clone());
    let resources = mcp.call("list_resources", json!({})).await.unwrap();
    report.insert("resources".into(), json!(resources["items"].as_array().unwrap().iter().map(|r| json!([r["title"], r["work_id"]])).collect::<Vec<_>>()));

    // VNDB path: search with file-name hints, then create + bind from the first candidate.
    let unmatched = mcp.call("list_resources", json!({"status": "unmatched"})).await.unwrap();
    let target = unmatched["items"].as_array().unwrap().iter().find(|r| r["title"].as_str().unwrap_or("").contains("kelp")).or(unmatched["items"].get(0)).cloned();
    if let Some(target) = target {
        let query = target["title"].as_str().unwrap().to_owned();
        let found = mcp.call("search_vndb", json!({"query": query, "resource_id": target["id"]})).await.unwrap();
        report.insert("vndb_candidates".into(), json!(found["results"].as_array().unwrap().iter().take(3).map(|c| json!([c["id"], c["title"], c["match"]])).collect::<Vec<_>>()));
        if let Some(first) = found["results"].get(0) {
            let matched = mcp.call("match_resource", json!({"resource_id": target["id"], "revision": target["revision"], "vndb_id": first["id"], "query": query})).await.unwrap();
            let work = mcp.call("get_work", json!({"work_id": matched["work_id"]})).await.unwrap();
            assert_eq!(work["work"]["vndb_id"], first["id"]);
            report.insert("matched".into(), json!({"resource": query, "work": work["work"]["title"], "vndb": first["id"]}));
            let updated = mcp.call("update_work", json!({"work_id": matched["work_id"], "revision": work["work"]["revision"], "status": "playing"})).await.unwrap();
            report.insert("updated".into(), updated);

            let managed = base.join("managed");
            std::fs::create_dir_all(&managed).unwrap();
            let preview = mcp.call("preview_organize", json!({"resource_id": target["id"], "destination": managed})).await.unwrap();
            assert_eq!(preview["plan"]["state"], "ready");
            assert!(snapshot(&managed).is_empty());
            report.insert("organize_preview".into(), json!({"file_count": preview["plan"]["file_count"], "first_target": preview["plan"]["moves"][0]["target"]}));
        }
    }

    // Restart Core on a new port; the same MCP process must rediscover it.
    local_core::request(&base, &exe, "stop").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let down = mcp.call("galroon_overview", json!({})).await;
    assert!(down.is_err());
    report.insert("while_stopped".into(), json!(down.unwrap_err()));
    let restarted = local_core::connect_or_start(base.clone(), exe.clone()).await.unwrap();
    report.insert("second_url".into(), json!(restarted.url));
    let again = mcp.call("galroon_overview", json!({})).await.unwrap();
    assert_eq!(again["collection"]["id"], overview["collection"]["id"]);

    assert_eq!(snapshot(&source), before, "source files must be untouched");
    println!("REPORT {}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
    local_core::request(&base, &exe, "stop").await.unwrap();
}

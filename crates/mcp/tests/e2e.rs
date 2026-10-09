//! Drives the real galroon-mcp binary over stdio against a real Core with a generated catalog.
mod support;
use reqwest::Method;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Server {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    next: i64,
}

impl Server {
    fn spawn(url: &str, token_file: &std::path::Path) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
            .args(["serve", "--url", url])
            .env("GALROON_MCP_TOKEN_FILE", token_file)
            .env_remove("GALROON_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        Server { child, stdin, stdout, next: 1 }
    }

    async fn send(&mut self, message: Value) {
        self.stdin.write_all(format!("{message}\n").as_bytes()).await.unwrap();
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).await;
        let line = self.stdout.next_line().await.unwrap().expect("server closed stdout");
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id, "{response}");
        response
    }

    /// Returns (parsed result, is_error).
    async fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let response = self.request("tools/call", json!({"name": name, "arguments": arguments})).await;
        let result = &response["result"];
        let text = result["content"][0]["text"].as_str().unwrap_or_else(|| panic!("{response}"));
        let is_error = result["isError"].as_bool().unwrap();
        (serde_json::from_str(text).unwrap_or(json!(text)), is_error)
    }

    async fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments).await;
        assert!(!is_error, "{name} failed: {value}");
        value
    }
}

async fn pair(f: &support::Fixture, token_file: &std::path::Path) {
    let code = f.pairing_code().await;
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp")).args(["pair", &code, "--url", &f.url]).env("GALROON_MCP_TOKEN_FILE", token_file).output().await.unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "pair must not print the credential to stdout");
}

#[tokio::test]
async fn pairing_creates_a_private_revocable_desktop_credential() {
    let f = support::Fixture::start().await;
    let t = tempfile::tempdir().unwrap();
    let token_file = t.path().join("nested/credential.json");
    pair(&f, &token_file).await;
    let saved: Value = serde_json::from_slice(&std::fs::read(&token_file).unwrap()).unwrap();
    let context = f.owner(Method::GET, "/context", None).await;
    assert_eq!(saved["library_id"], context["library"]["id"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&token_file).unwrap().permissions().mode() & 0o777, 0o600);
    }

    let check = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp")).args(["check", "--url", &f.url]).env("GALROON_MCP_TOKEN_FILE", &token_file).output().await.unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    let overview: Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(overview["can_write"], true);

    // The device is a paired desktop, not the owner: it cannot administer access.
    let token = saved["token"].as_str().unwrap();
    let devices = reqwest::Client::new().get(format!("{}/api/access/devices", f.url)).bearer_auth(token).send().await.unwrap();
    assert_eq!(devices.status(), 403);

    // A reused code fails and leaves the saved credential intact.
    let before = std::fs::read(&token_file).unwrap();
    let reuse = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp")).args(["pair", "AAAA-BBBB-CCCC", "--url", &f.url]).env("GALROON_MCP_TOKEN_FILE", &token_file).output().await.unwrap();
    assert!(!reuse.status.success());
    assert_eq!(std::fs::read(&token_file).unwrap(), before);

    // Owner revocation takes effect immediately with an actionable message.
    let devices = f.owner(Method::GET, "/access/devices", None).await;
    let id = devices.as_array().unwrap().iter().find(|d| d["role"] == "desktop").unwrap()["id"].as_str().unwrap().to_owned();
    f.owner(Method::POST, &format!("/access/devices/{id}/revoke"), Some(json!({}))).await;
    let mut server = Server::spawn(&f.url, &token_file);
    let (message, is_error) = server.call("galroon_overview", json!({})).await;
    assert!(is_error);
    assert!(message.as_str().unwrap().contains("revoked or expired"), "{message}");
}

#[tokio::test]
async fn mcp_session_tidies_the_catalog_without_touching_files() {
    let f = support::Fixture::start().await;
    f.seed().await;
    let t = tempfile::tempdir().unwrap();
    let token_file = t.path().join("credential.json");
    pair(&f, &token_file).await;
    let files_before: Vec<_> = walk(&f.source);
    let mut server = Server::spawn(&f.url, &token_file);

    let init = server.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}})).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "galroon");
    server.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).await;
    let unknown_version = server.request("initialize", json!({"protocolVersion": "1999-01-01"})).await;
    assert_eq!(unknown_version["result"]["protocolVersion"], "2025-11-25");

    let list = server.request("tools/list", json!({})).await;
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for forbidden in ["execute", "approve", "quarantine", "delete", "merge", "split", "access"] {
        assert!(!names.iter().any(|n| n.contains(forbidden)), "{forbidden} must not be exposed: {names:?}");
    }
    for t in list["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object");
        assert_eq!(t["annotations"]["destructiveHint"], false);
    }

    let overview = server.ok("galroon_overview", json!({})).await;
    assert_eq!(overview["counts"]["works"], 2);
    assert_eq!(overview["counts"]["unassigned"], 3);

    let found = server.ok("search_collection", json!({"query": "sakura"})).await;
    assert_eq!(found["matching"], 1);
    let sakura = found["items"][0].clone();
    assert_eq!(sakura["title"], "Sakura Note");
    assert_eq!(sakura["tags"], json!(["Romance"]));
    let by_tag = server.ok("search_collection", json!({"tags": ["romance"], "exclude_tags": []})).await;
    assert_eq!(by_tag["matching"], 2);
    let excluded = server.ok("search_collection", json!({"exclude_tags": ["Romance"]})).await;
    assert_eq!(excluded["matching"], 0);
    let studio = server.ok("search_collection", json!({"studio": "studio test", "sort": "title"})).await;
    assert_eq!(studio["items"][0]["title"], "Sakura Note");

    let unmatched = server.ok("list_resources", json!({"status": "unmatched"})).await;
    assert_eq!(unmatched["total"], 3);
    let game = unmatched["items"].as_array().unwrap().iter().find(|r| r["title"] == "Sakura Note").unwrap().clone();
    assert!(game.get("root_path").is_none());

    let issues = server.ok("list_issues", json!({})).await;
    assert!(issues["items"].as_array().unwrap().iter().any(|i| i["kind"] == "unmatched"));

    // Stale revisions are rejected by Core, then the current one succeeds.
    let (stale, is_error) = server.call("match_resource", json!({"resource_id": game["id"], "revision": 999, "work_id": sakura["id"]})).await;
    assert!(is_error, "{stale}");
    let matched = server.ok("match_resource", json!({"resource_id": game["id"], "revision": game["revision"], "work_id": sakura["id"], "edition": "Download edition"})).await;
    assert_eq!(matched["matched"], true);
    let remaining = server.ok("list_resources", json!({"status": "unmatched"})).await;
    assert_eq!(remaining["total"], 2);
    let (both, is_error) = server.call("match_resource", json!({"resource_id": game["id"], "revision": 1, "work_id": "a", "vndb_id": "v1"})).await;
    assert!(is_error && both.as_str().unwrap().contains("exactly one"));

    let detail = server.ok("get_work", json!({"work_id": sakura["id"]})).await;
    assert_eq!(detail["work"]["title"], "Sakura Note");
    assert!(detail["assets"].to_string().contains("Download edition"), "{detail}");

    // Matching advanced the work revision, so the earlier search result is stale.
    let (stale, is_error) = server.call("update_work", json!({"work_id": sakura["id"], "revision": sakura["revision"], "status": "playing"})).await;
    assert!(is_error && stale.as_str().unwrap().contains("changed"), "{stale}");
    let updated = server.ok("update_work", json!({"work_id": sakura["id"], "revision": detail["work"]["revision"], "status": "playing", "favorite": true})).await;
    let playing = server.ok("search_collection", json!({"status": "playing"})).await;
    assert_eq!(playing["items"][0]["favorite"], true);
    let (bad, is_error) = server.call("update_work", json!({"work_id": sakura["id"], "revision": updated["revision"], "status": "finished"})).await;
    assert!(is_error, "{bad}");
    let (nothing, is_error) = server.call("update_work", json!({"work_id": sakura["id"], "revision": updated["revision"]})).await;
    assert!(is_error, "{nothing}");
    let (path, is_error) = server.call("get_work", json!({"work_id": "../access/devices"})).await;
    assert!(is_error && path.as_str().unwrap().contains("Invalid id"));

    let created = server.ok("create_list", json!({"name": "Winter queue"})).await;
    let list_id = created["list_id"].as_str().unwrap().to_owned();
    let list = server.ok("get_list", json!({"list_id": list_id})).await;
    let all = server.ok("search_collection", json!({})).await;
    let ids: Vec<Value> = all["items"].as_array().unwrap().iter().map(|w| w["id"].clone()).collect();
    let added = server.ok("add_to_list", json!({"list_id": list_id, "revision": list["revision"], "work_ids": ids})).await;
    assert_eq!(added["added"], 2, "{added}");
    let list = server.ok("get_list", json!({"list_id": list_id})).await;
    assert!(list.to_string().contains("Winter Lantern"), "{list}");
    let lists = server.ok("list_lists", json!({})).await;
    assert!(lists.to_string().contains("Winter queue"));
    let (missing, is_error) = server.call("add_to_list", json!({"list_id": list_id, "revision": list["revision"], "work_ids": ["no-such-work"]})).await;
    assert!(is_error, "{missing}");

    // Paging past the first 50 entries needs the integer cursor plus the first page's revision.
    let mut ids = Vec::new();
    for i in 0..55 {
        ids.push(f.owner(Method::POST, "/works", Some(json!({"title": format!("Paging fixture {i:02}")}))).await["id"].clone());
    }
    let long = server.ok("create_list", json!({"name": "Long list"})).await["list_id"].as_str().unwrap().to_owned();
    let mut revision = server.ok("get_list", json!({"list_id": long})).await["revision"].clone();
    for chunk in ids.chunks(30) {
        server.ok("add_to_list", json!({"list_id": long, "revision": revision, "work_ids": chunk})).await;
        revision = server.ok("get_list", json!({"list_id": long})).await["revision"].clone();
    }
    let first = server.ok("get_list", json!({"list_id": long})).await;
    let next = first["next_after"].clone();
    assert!(next.is_i64(), "{first}");
    let (no_revision, is_error) = server.call("get_list", json!({"list_id": long, "cursor": next})).await;
    assert!(is_error, "{no_revision}");
    let second = server.ok("get_list", json!({"list_id": long, "cursor": next, "revision": first["revision"]})).await;
    assert!(second.to_string().contains("Paging fixture 54"), "{second}");
    assert!(!second.to_string().contains("Paging fixture 00"), "{second}");

    let jobs = server.ok("list_jobs", json!({})).await;
    assert_eq!(jobs["jobs"][0]["kind"], "scan");
    server.ok("list_plans", json!({})).await;

    // Organizing outside a managed folder is refused by Core; nothing on disk changes either way.
    let (refused, is_error) = server.call("preview_organize", json!({"resource_id": game["id"], "destination": f.source})).await;
    assert!(is_error, "{refused}");
    assert_eq!(walk(&f.source), files_before);
    let managed = f.source.parent().unwrap().join("managed");
    std::fs::create_dir(&managed).unwrap();
    let preview = server.ok("preview_organize", json!({"resource_id": game["id"], "destination": managed})).await;
    assert_eq!(preview["plan"]["file_count"], 1);
    assert!(preview["plan"]["moves"][0]["target"].as_str().unwrap().contains("Download edition"));
    assert_eq!(walk(&f.source), files_before, "a preview must not move files");
    assert!(walk(&managed).is_empty());
    let plans = server.ok("list_plans", json!({})).await;
    assert_eq!(plans["plans"][0]["id"], preview["plan"]["id"]);
    assert_ne!(plans["plans"][0]["state"], "completed");

    let raw = server.request("tools/call", json!({"name": "no_such_tool"})).await;
    assert_eq!(raw["error"]["code"], -32602);
    let missing_method = server.request("resources/list", json!({})).await;
    assert_eq!(missing_method["error"]["code"], -32601);
    server.stdin.write_all(b"not json\n").await.unwrap();
    let parse: Value = serde_json::from_str(&server.stdout.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(parse["error"]["code"], -32700);

    drop(server.stdin);
    assert!(server.child.wait().await.unwrap().success());
}

#[tokio::test]
async fn requests_are_pinned_to_the_paired_collection() {
    let f = support::Fixture::start().await;
    let code = f.pairing_code().await;
    let session = galroon_mcp::client::redeem(&f.url, &code).await.unwrap();
    let token = session["token"].as_str().unwrap().to_owned();
    let other = galroon_mcp::client::Core::new(f.url.clone(), token.clone(), Some("another-collection".into())).unwrap();
    let error = galroon_mcp::tools::Tools::fixed(other).call("galroon_overview", &json!({})).await.unwrap_err();
    assert!(error.contains("different collection"), "{error}");
    let right = galroon_mcp::client::Core::new(f.url.clone(), token, None).unwrap();
    assert_eq!(right.library().await.unwrap(), f.owner(Method::GET, "/context", None).await["library"]["id"].as_str().unwrap());
}

fn walk(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push((path.clone(), std::fs::read(&path).unwrap()));
        }
    }
    out.sort();
    out
}

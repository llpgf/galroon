//! A real Core on an ephemeral loopback port with a generated catalog.
use serde_json::{json, Value};
use std::time::Duration;

pub struct Fixture {
    pub _dir: tempfile::TempDir,
    pub url: String,
    pub owner: String,
    pub source: std::path::PathBuf,
}

impl Fixture {
    pub async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // macOS temp paths sit behind a symlink, which Core refuses to mutate through.
        let root = dir.path().canonicalize().unwrap();
        let app = galroon_core::initialize(root.join("state")).unwrap();
        let owner = app.token.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        tokio::spawn(galroon_core::serve_listener(app, listener));
        let source = root.join("source");
        for (folder, file) in [("Sakura Note", "game.exe"), ("Sakura Note OST", "01.flac"), ("Unknown Pack", "data.bin")] {
            std::fs::create_dir_all(source.join(folder)).unwrap();
            std::fs::write(source.join(folder).join(file), folder.as_bytes()).unwrap();
        }
        Fixture { _dir: dir, url, owner, source }
    }

    pub async fn owner(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Value {
        let mut request = reqwest::Client::new().request(method, format!("{}/api{path}", self.url)).bearer_auth(&self.owner);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        assert!(status.is_success(), "{path}: {body}");
        body
    }

    pub async fn seed(&self) {
        use reqwest::Method;
        for (title, original, vndb) in [("Sakura Note", "さくらノート", "v90001"), ("Winter Lantern", "冬の灯籠", "v90002")] {
            self.owner(Method::POST, "/works", Some(json!({"title": title, "original_title": original, "vndb_id": vndb, "developer": "Studio Test", "released": "2020-01-01", "tags": [{"name": "Romance"}]}))).await;
        }
        let root = self.owner(Method::POST, "/roots", Some(json!({"path": self.source, "label": "Fixture"}))).await;
        let job = self.owner(Method::POST, "/scans", Some(json!({"root_id": root["id"], "scope": "", "exclude": []}))).await;
        for _ in 0..200 {
            let detail = self.owner(Method::GET, &format!("/jobs/{}", job["id"].as_str().unwrap()), None).await;
            if !matches!(detail["state"].as_str(), Some("queued" | "running")) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("Scan did not finish");
    }

    pub async fn pairing_code(&self) -> String {
        use reqwest::Method;
        let status = self.owner(Method::GET, "/access/status", None).await;
        if status["configured"] != true {
            self.owner(Method::POST, "/access/setup", Some(json!({"password": "Generated fixture owner password"}))).await;
        }
        self.owner(Method::POST, "/access/pairing", Some(json!({"name": "MCP fixture"}))).await["code"].as_str().unwrap().to_owned()
    }
}

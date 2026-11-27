//! Verifies MCP remote LAN HTTPS Core connections with strict SPKI pinning against a real LanTls listener.
use galroon_core::{
    lan_tls::{format_spki_fingerprint, spki_fingerprint, LanTls},
    network::{self, NetworkEdit},
    pairing::format_pairing_string,
};
use galroon_mcp::{client, tools::Tools};
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

struct LanFixture {
    _dir: tempfile::TempDir,
    app: galroon_core::App,
    lan_url: String,
    lan_port: u16,
    fingerprint: String,
    wrong_fingerprint: String,
    loopback_url: String,
    owner: String,
    library_id: String,
    device_id: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    server: tokio::task::JoinHandle<Result<(), String>>,
}

impl Drop for LanFixture {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.server.abort();
    }
}

impl LanFixture {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        let plain = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let lan = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let loopback_url = format!("http://127.0.0.1:{}", plain.local_addr().unwrap().port());
        let lan_port = lan.local_addr().unwrap().port();
        // Configure valid LAN settings allowing Host: localhost (resolving to our local LanTls listener).
        let settings = network::save(
            &state,
            NetworkEdit {
                address: "192.168.50.154".parse().unwrap(),
                port: lan_port,
                hostnames: vec!["localhost".into()],
                enabled: Some(true),
            },
        )
        .unwrap();
        let tls = LanTls::load_or_generate(&state, &settings).unwrap();
        let fingerprint = format_spki_fingerprint(&spki_fingerprint(&tls.cert_der));
        let wrong_fingerprint = format_spki_fingerprint(&[0u8; 32]);
        let app = galroon_core::initialize(state).unwrap();
        let owner = app.token.clone();
        let library_id = galroon_core::context::library(&app.db.lock().unwrap()).unwrap().id;
        let device_id = app.device.id.clone();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let served = app.clone();
        let server = tokio::spawn(async move {
            galroon_core::serve_tls_listeners(
                served,
                vec![
                    galroon_core::Listener::Loopback(plain),
                    galroon_core::Listener::Lan(lan, tls.config),
                ],
                async {
                    let _ = stopped.await;
                },
            )
            .await
        });
        Self {
            _dir: dir,
            app,
            lan_url: format!("https://localhost:{lan_port}"),
            lan_port,
            fingerprint,
            wrong_fingerprint,
            loopback_url,
            owner,
            library_id,
            device_id,
            stop: Some(stop),
            server,
        }
    }

    async fn pairing_code(&self) -> String {
        let http = reqwest::Client::new();
        let status: Value = http.get(format!("{}/api/access/status", self.loopback_url)).send().await.unwrap().json().await.unwrap();
        if status["configured"] != true {
            http.post(format!("{}/api/access/setup", self.loopback_url))
                .bearer_auth(&self.owner)
                .json(&json!({"password": "Generated LAN MCP fixture password"}))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
        }
        let created: Value = http
            .post(format!("{}/api/access/pairing", self.loopback_url))
            .bearer_auth(&self.owner)
            .json(&json!({"name": "LAN MCP test"}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        created["code"].as_str().unwrap().to_owned()
    }
}

#[tokio::test]
async fn lan_client_enforces_spki_pin_and_identity_headers() {
    let f = LanFixture::start().await;
    let code = f.pairing_code().await;

    // 1. Mismatched SPKI pin fails before sending the pairing code (code remains unredeemed).
    let err = client::redeem_lan(&f.lan_url, &f.wrong_fingerprint, &code).await.unwrap_err();
    assert!(err.contains("certificate fingerprint doesn't match"), "{err}");
    let remaining: i64 = f.app.db.lock().unwrap().query_row("SELECT count(*) FROM pairing_codes", [], |r| r.get(0)).unwrap();
    assert_eq!(remaining, 1, "Pairing code must not be sent when SPKI pin fails");

    // 2. Correct SPKI pin redeems the pairing code and connects over LAN TLS.
    let session = client::redeem_lan(&f.lan_url, &f.fingerprint, &code).await.unwrap();
    let token = session["token"].as_str().unwrap().to_owned();

    let pinned = client::Core::new_lan(f.lan_url.clone(), &f.fingerprint, token.clone(), Some(f.library_id.clone()), Some(f.device_id.clone())).unwrap();
    let (lib, dev) = pinned.identity().await.unwrap();
    assert_eq!(lib, f.library_id);
    assert_eq!(dev.as_deref(), Some(f.device_id.as_str()));

    let overview = Tools::fixed(pinned).call("galroon_overview", &json!({})).await.unwrap();
    assert_eq!(overview["can_write"], true);

    // 3. Mismatched SPKI pin on authenticated requests is rejected.
    let bad_pin = client::Core::new_lan(f.lan_url.clone(), &f.wrong_fingerprint, token.clone(), Some(f.library_id.clone()), Some(f.device_id.clone())).unwrap();
    let pin_err = Tools::fixed(bad_pin).call("galroon_overview", &json!({})).await.unwrap_err();
    assert!(pin_err.contains("certificate fingerprint doesn't match"), "{pin_err}");

    // 4. Mismatched x-galroon-library or x-galroon-device headers trigger 409 collection/device mismatch.
    let wrong_lib = client::Core::new_lan(f.lan_url.clone(), &f.fingerprint, token.clone(), Some("wrong-library".into()), Some(f.device_id.clone())).unwrap();
    let lib_err = Tools::fixed(wrong_lib).call("galroon_overview", &json!({})).await.unwrap_err();
    assert!(lib_err.contains("different collection or device"), "{lib_err}");

    let wrong_dev = client::Core::new_lan(f.lan_url.clone(), &f.fingerprint, token, Some(f.library_id.clone()), Some("wrong-device".into())).unwrap();
    let dev_err = Tools::fixed(wrong_dev).call("galroon_overview", &json!({})).await.unwrap_err();
    assert!(dev_err.contains("different collection or device"), "{dev_err}");
}

#[tokio::test]
async fn cli_supports_lan_pairing_string_token_env_and_token_stdin() {
    let f = LanFixture::start().await;
    let code = f.pairing_code().await;
    let pairing = format_pairing_string("localhost", f.lan_port, &code.replace('-', ""), &f.fingerprint).unwrap();
    let t = tempfile::tempdir().unwrap();
    let token_file = t.path().join("lan-credential.json");

    // 1. Pair using a galroon-pair:v1 string; saves URL, SPKI pin, library_id, device_id to 0600 file.
    let pair = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args(["pair", &pairing])
        .env("GALROON_MCP_TOKEN_FILE", &token_file)
        .env_remove("GALROON_URL")
        .env_remove("GALROON_FINGERPRINT")
        .env_remove("GALROON_TOKEN")
        .output()
        .await
        .unwrap();
    assert!(pair.status.success(), "{}", String::from_utf8_lossy(&pair.stderr));
    assert!(pair.stdout.is_empty(), "pair must not leak token to stdout");

    let saved: Value = serde_json::from_slice(&std::fs::read(&token_file).unwrap()).unwrap();
    assert_eq!(saved["url"], f.lan_url);
    assert_eq!(saved["fingerprint"], f.fingerprint);
    assert_eq!(saved["library_id"], f.library_id);
    assert_eq!(saved["device_id"], f.device_id);
    let token = saved["token"].as_str().unwrap().to_owned();

    // 2. Subsequent `check` works directly from saved LAN credential without argv flags.
    let check_saved = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args(["check"])
        .env("GALROON_MCP_TOKEN_FILE", &token_file)
        .env_remove("GALROON_URL")
        .env_remove("GALROON_FINGERPRINT")
        .env_remove("GALROON_TOKEN")
        .output()
        .await
        .unwrap();
    assert!(check_saved.status.success(), "{}", String::from_utf8_lossy(&check_saved.stderr));
    let overview: Value = serde_json::from_slice(&check_saved.stdout).unwrap();
    assert_eq!(overview["can_write"], true);

    // 3. Explicit --url + --fingerprint + --token-env + --library + --device works without credential file.
    let check_env = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args([
            "check",
            "--url",
            &f.lan_url,
            "--fingerprint",
            &f.fingerprint,
            "--token-env",
            "TEST_LAN_MCP_SECRET",
            "--library",
            &f.library_id,
            "--device",
            &f.device_id,
        ])
        .env("TEST_LAN_MCP_SECRET", &token)
        .env("GALROON_MCP_TOKEN_FILE", t.path().join("nonexistent.json"))
        .env_remove("GALROON_TOKEN")
        .output()
        .await
        .unwrap();
    assert!(check_env.status.success(), "{}", String::from_utf8_lossy(&check_env.stderr));

    // 4. --token-stdin keeps secret out of argv and succeeds over pinned LAN TLS.
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args(["check", "--url", &f.lan_url, "--fingerprint", &f.fingerprint, "--token-stdin"])
        .env("GALROON_MCP_TOKEN_FILE", t.path().join("nonexistent.json"))
        .env_remove("GALROON_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(format!("{token}\n").as_bytes()).await.unwrap();
    let out = child.wait_with_output().await.unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // 5. Wrong --fingerprint and --token on argv are both rejected.
    let bad_fp = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args(["check", "--url", &f.lan_url, "--fingerprint", &f.wrong_fingerprint, "--token-env", "TEST_LAN_MCP_SECRET"])
        .env("TEST_LAN_MCP_SECRET", &token)
        .output()
        .await
        .unwrap();
    assert!(!bad_fp.status.success());
    assert!(String::from_utf8_lossy(&bad_fp.stderr).contains("certificate fingerprint doesn't match"));

    let argv_token = tokio::process::Command::new(env!("CARGO_BIN_EXE_galroon-mcp"))
        .args(["check", "--url", &f.lan_url, "--fingerprint", &f.fingerprint, "--token", "secret"])
        .output()
        .await
        .unwrap();
    assert!(!argv_token.status.success());
    assert!(String::from_utf8_lossy(&argv_token.stderr).contains("--token-env"));
}

//! Minimal Core HTTP client using a paired-device bearer credential.
use crate::config::R;
use reqwest::{header, Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

/// Prefix of errors where the connection was refused, i.e. the request never reached Core.
pub const UNREACHABLE: &str = "Galroon Core is not reachable";

pub struct Core {
    http: reqwest::Client,
    base: String,
    token: String,
    /// Pins every request to the paired collection; Core answers 409 if another collection is open.
    library_id: Option<String>,
}

fn http() -> R<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

async fn decode(response: reqwest::Response) -> R<Value> {
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    let message = body["error"].as_str().unwrap_or("").to_owned();
    Err(match status {
        StatusCode::CONFLICT if body["code"] == "collection_changed" => {
            "Galroon now has a different collection open than the one this MCP server was paired with. Switch back or pair again.".into()
        }
        StatusCode::UNAUTHORIZED if message == "Authentication required" => {
            "The MCP credential was revoked or expired. Create a new pairing code in Galroon and run: galroon-mcp pair <CODE>".into()
        }
        _ if message.is_empty() => format!("Core returned HTTP {status}"),
        _ => message,
    })
}

impl Core {
    pub fn new(base: String, token: String, library_id: Option<String>) -> R<Self> {
        Ok(Self { http: http()?, base, token, library_id: library_id.filter(|id| !id.is_empty()) })
    }

    pub async fn request(&self, method: Method, path: &str, query: &[(&str, String)], body: Option<&Value>) -> R<Value> {
        self.request_within(method, path, query, body, None).await
    }

    async fn request_within(&self, method: Method, path: &str, query: &[(&str, String)], body: Option<&Value>, timeout: Option<Duration>) -> R<Value> {
        let mut request = self.http.request(method, format!("{}/api{}", self.base, path)).bearer_auth(&self.token).query(query);
        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
        }
        if let Some(id) = &self.library_id {
            request = request.header("x-galroon-library", id);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|e| {
            if e.is_connect() {
                format!("{UNREACHABLE} at {}. Is Galroon open?", self.base)
            } else if e.is_timeout() {
                "Galroon Core did not answer in time and may still be working on this request. Check list_plans or list_jobs before retrying.".into()
            } else {
                format!("Request to Galroon Core failed: {e}")
            }
        })?;
        decode(response).await
    }

    pub async fn get(&self, path: &str, query: &[(&str, String)]) -> R<Value> {
        self.request(Method::GET, path, query, None).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> R<Value> {
        self.request(Method::POST, path, &[], Some(body)).await
    }

    /// For requests whose work grows with file size, such as hashing every file of an organize preview.
    pub async fn post_long(&self, path: &str, body: &Value) -> R<Value> {
        self.request_within(Method::POST, path, &[], Some(body), Some(Duration::from_secs(30 * 60))).await
    }

    /// The library identity Core reports for this credential.
    pub async fn library(&self) -> R<String> {
        let response = self.http.get(format!("{}/api/access/me", self.base)).bearer_auth(&self.token).send().await.map_err(|e| e.to_string())?;
        let library = response.headers().get("x-galroon-library").and_then(|v| v.to_str().ok()).map(str::to_owned);
        decode(response).await?;
        library.ok_or_else(|| "Core did not report its collection identity".into())
    }
}

/// Redeems a one-use pairing code. Core requires a same-origin Origin header for this route.
pub async fn redeem(base: &str, code: &str) -> R<Value> {
    let response = http()?
        .post(format!("{base}/api/access/pair"))
        .header(header::ORIGIN, base)
        .json(&json!({ "code": code }))
        .send()
        .await
        .map_err(|e| format!("{UNREACHABLE} at {base} ({e})"))?;
    decode(response).await
}

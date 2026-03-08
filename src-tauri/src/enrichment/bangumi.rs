//! Bangumi API client — secondary enrichment source.
//!
//! GET https://api.bgm.tv/search/subject/{keyword}
//!
//! Fills gaps where VNDB has no data.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::{BangumiAuthConfig, SharedConfig};

use super::rate_limit::RateLimiter;

const BANGUMI_API_URL: &str = "https://api.bgm.tv";
const BANGUMI_OAUTH_URL: &str = "https://bgm.tv/oauth/access_token";
const BANGUMI_DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:48573/bangumi/callback";

/// Bangumi API client.
#[derive(Clone)]
pub struct BangumiClient {
    inner: std::sync::Arc<RwLock<BangumiClientInner>>,
    rate_limiter: RateLimiter,
    shared_config: Option<SharedConfig>,
}

struct BangumiClientInner {
    http: reqwest::Client,
    auth: Option<BangumiAuthConfig>,
}

/// Bangumi search response.
#[derive(Debug, Deserialize)]
pub struct BangumiSearchResponse {
    pub results: Option<u32>,
    pub list: Option<Vec<BangumiSubject>>,
}

/// A subject (game) from Bangumi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiSubject {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub name_cn: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub rating: Option<BangumiRating>,
    #[serde(default)]
    pub images: Option<BangumiImages>,
    #[serde(rename = "type")]
    pub subject_type: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiRating {
    pub score: f64,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiImages {
    pub large: Option<String>,
    pub medium: Option<String>,
    #[serde(default)]
    pub grid: Option<String>,
    pub small: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiPersonRelation {
    #[serde(default)]
    pub images: Option<BangumiImages>,
    pub name: String,
    pub relation: String,
    #[serde(default)]
    pub career: Vec<String>,
    #[serde(rename = "type")]
    pub person_type: u32,
    pub id: u64,
    #[serde(default)]
    pub eps: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiCharacterRelationActor {
    #[serde(default)]
    pub images: Option<BangumiImages>,
    pub name: String,
    #[serde(default)]
    pub short_summary: Option<String>,
    #[serde(default)]
    pub career: Vec<String>,
    pub id: u64,
    #[serde(rename = "type")]
    pub actor_type: u32,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiCharacterRelation {
    #[serde(default)]
    pub images: Option<BangumiImages>,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub relation: String,
    #[serde(default)]
    pub actors: Vec<BangumiCharacterRelationActor>,
    #[serde(rename = "type")]
    pub character_type: u32,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BangumiMe {
    pub id: u64,
    pub username: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub avatar: Option<BangumiImages>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BangumiOAuthToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

impl BangumiClient {
    pub fn new(
        rate_limiter: RateLimiter,
        auth: Option<BangumiAuthConfig>,
        shared_config: Option<SharedConfig>,
    ) -> Self {
        let http = build_http_client(auth.as_ref());
        let inner = BangumiClientInner { http, auth };

        Self {
            inner: std::sync::Arc::new(RwLock::new(inner)),
            rate_limiter,
            shared_config,
        }
    }

    pub async fn update_auth(&self, auth: Option<BangumiAuthConfig>) {
        let mut inner = self.inner.write().await;
        inner.http = build_http_client(auth.as_ref());
        inner.auth = auth;
    }

    pub async fn auth_snapshot(&self) -> Option<BangumiAuthConfig> {
        self.inner.read().await.auth.clone()
    }

    pub async fn get_me(&self) -> Result<Option<BangumiMe>, String> {
        let resp = self
            .send_with_auto_refresh(|http| http.get(format!("{}/v0/me", BANGUMI_API_URL)))
            .await?;

        if resp.status() == 401 || resp.status() == 403 {
            return Ok(None);
        }

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Bangumi API error: {} - {}", status, body));
        }

        let me: BangumiMe = resp
            .json()
            .await
            .map_err(|e| format!("Bangumi parse error: {}", e))?;

        Ok(Some(me))
    }

    pub async fn exchange_oauth_code(
        &self,
        app_id: &str,
        app_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<BangumiOAuthToken, String> {
        self.rate_limiter.acquire("bangumi").await;

        let payload = serde_json::json!({
            "client_id": app_id,
            "client_secret": app_secret,
            "code": code,
            "grant_type": "authorization_code",
            "redirect_uri": redirect_uri,
        });

        let http = build_oauth_http_client(app_id);
        let resp = http
            .post(BANGUMI_OAUTH_URL)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Bangumi OAuth request failed: {}", e))?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Bangumi OAuth error: {} - {}", status, body));
        }

        resp.json()
            .await
            .map_err(|e| format!("Bangumi OAuth parse error: {}", e))
    }

    pub async fn refresh_oauth_token(
        &self,
        app_id: &str,
        app_secret: &str,
        refresh_token: &str,
        redirect_uri: &str,
    ) -> Result<BangumiOAuthToken, String> {
        self.rate_limiter.acquire("bangumi").await;

        let payload = serde_json::json!({
            "client_id": app_id,
            "client_secret": app_secret,
            "refresh_token": refresh_token,
            "grant_type": "refresh_token",
            "redirect_uri": redirect_uri,
        });

        let http = build_oauth_http_client(app_id);
        let resp = http
            .post(BANGUMI_OAUTH_URL)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Bangumi OAuth refresh failed: {}", e))?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Bangumi OAuth refresh error: {} - {}",
                status, body
            ));
        }

        resp.json()
            .await
            .map_err(|e| format!("Bangumi OAuth refresh parse error: {}", e))
    }

    /// Search Bangumi by keyword. Filters to type=4 (game).
    pub async fn search_by_title(
        &self,
        title: &str,
        limit: u32,
    ) -> Result<Vec<BangumiSubject>, String> {
        debug!(title = %title, "Bangumi search request");

        let url = format!(
            "{}/search/subject/{}?type=4&responseGroup=small&max_results={}",
            BANGUMI_API_URL,
            urlencoding_simple(title),
            limit
        );

        let resp = self.send_with_auto_refresh(|http| http.get(&url)).await?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, "Bangumi API error");
            return Err(format!("Bangumi API error: {} - {}", status, body));
        }

        let data: BangumiSearchResponse = resp
            .json()
            .await
            .map_err(|e| format!("Bangumi parse error: {}", e))?;

        let results = data.list.unwrap_or_default();
        info!(
            title = %title,
            results = results.len(),
            "Bangumi search complete"
        );

        Ok(results)
    }

    /// Fetch a single subject by Bangumi ID.
    pub async fn get_by_id(&self, bgm_id: u64) -> Result<Option<BangumiSubject>, String> {
        let resp = self
            .send_with_auto_refresh(|http| {
                http.get(format!("{}/v0/subjects/{}", BANGUMI_API_URL, bgm_id))
            })
            .await?;

        if resp.status() == 404 {
            return Ok(None);
        }

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Bangumi API error: {} - {}", status, body));
        }

        let subject: BangumiSubject = resp
            .json()
            .await
            .map_err(|e| format!("Bangumi parse error: {}", e))?;

        Ok(Some(subject))
    }

    pub async fn get_subject_persons(
        &self,
        bgm_id: u64,
    ) -> Result<Vec<BangumiPersonRelation>, String> {
        let resp = self
            .send_with_auto_refresh(|http| {
                http.get(format!(
                    "{}/v0/subjects/{}/persons",
                    BANGUMI_API_URL, bgm_id
                ))
            })
            .await?;

        if resp.status() == 404 {
            return Ok(Vec::new());
        }

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Bangumi API error: {} - {}", status, body));
        }

        resp.json()
            .await
            .map_err(|e| format!("Bangumi parse error: {}", e))
    }

    pub async fn get_subject_characters(
        &self,
        bgm_id: u64,
    ) -> Result<Vec<BangumiCharacterRelation>, String> {
        let resp = self
            .send_with_auto_refresh(|http| {
                http.get(format!(
                    "{}/v0/subjects/{}/characters",
                    BANGUMI_API_URL, bgm_id
                ))
            })
            .await?;

        if resp.status() == 404 {
            return Ok(Vec::new());
        }

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("bangumi").await;
            return Err("Rate limited by Bangumi (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Bangumi API error: {} - {}", status, body));
        }

        resp.json()
            .await
            .map_err(|e| format!("Bangumi parse error: {}", e))
    }

    async fn send_with_auto_refresh<F>(&self, build: F) -> Result<reqwest::Response, String>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        self.rate_limiter.acquire("bangumi").await;

        let http = { self.inner.read().await.http.clone() };
        let resp = build(&http)
            .send()
            .await
            .map_err(|e| format!("Bangumi request failed: {}", e))?;

        if resp.status() == 401 || resp.status() == 403 {
            if self.try_refresh_auth().await? {
                self.rate_limiter.acquire("bangumi").await;
                let refreshed_http = { self.inner.read().await.http.clone() };
                return build(&refreshed_http)
                    .send()
                    .await
                    .map_err(|e| format!("Bangumi request failed after refresh: {}", e));
            }
        }

        Ok(resp)
    }

    async fn try_refresh_auth(&self) -> Result<bool, String> {
        let auth = match self.auth_snapshot().await {
            Some(auth) => auth,
            None => return Ok(false),
        };

        let refresh_token = match auth
            .refresh_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => return Ok(false),
        };
        let app_id = match auth
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => return Ok(false),
        };
        let app_secret = match auth
            .app_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => return Ok(false),
        };

        let token = self
            .refresh_oauth_token(
                app_id,
                app_secret,
                refresh_token,
                BANGUMI_DEFAULT_REDIRECT_URI,
            )
            .await?;

        let refreshed = BangumiAuthConfig {
            access_token: Some(token.access_token),
            refresh_token: token.refresh_token.or_else(|| auth.refresh_token.clone()),
            expires_at: token.expires_in.map(|seconds| {
                (chrono::Utc::now() + chrono::Duration::seconds(seconds as i64)).to_rfc3339()
            }),
            app_id: auth.app_id.clone(),
            app_secret: auth.app_secret.clone(),
        };

        self.update_auth(Some(refreshed.clone())).await;
        if let Some(config) = &self.shared_config {
            config
                .update(|cfg| {
                    cfg.bangumi = Some(refreshed.clone());
                })
                .await
                .map_err(|e| format!("Failed to persist refreshed Bangumi auth: {}", e))?;
        }

        Ok(true)
    }
}

fn build_http_client(auth: Option<&BangumiAuthConfig>) -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    if let Some(token) = auth
        .and_then(|value| value.access_token.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let bearer = format!("Bearer {token}");
        if let Ok(value) = HeaderValue::from_str(&bearer) {
            headers.insert(AUTHORIZATION, value);
        }
    }

    let user_agent = auth
        .and_then(|value| value.app_id.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(|app_id| format!("Galroon/0.5.0 (Bangumi app {app_id})"))
        .unwrap_or_else(|| "Galroon/0.5.0 (galgame-library-manager)".to_string());

    reqwest::Client::builder()
        .default_headers(headers)
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

fn build_oauth_http_client(app_id: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers
        })
        .user_agent(format!("Galroon/0.5.0 (Bangumi app {app_id})"))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create OAuth HTTP client")
}

/// Simple URL encoding (no external crate).
fn urlencoding_simple(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                for b in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

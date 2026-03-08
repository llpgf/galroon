//! VNDB API client — single implementation for the Kana API.
//!
//! POST https://api.vndb.org/kana/vn
//!
//! Handles: search by title, fetch by ID, response parsing.

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::rate_limit::RateLimiter;

const VNDB_API_URL: &str = "https://api.vndb.org/kana";

/// VNDB API client.
#[derive(Clone)]
pub struct VndbClient {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
}

/// VNDB API response for VN queries.
#[derive(Debug, Deserialize)]
pub struct VndbResponse {
    pub results: Vec<VndbVn>,
    pub more: bool,
}

/// A visual novel entry from VNDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VndbVn {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub alttitle: Option<String>,
    #[serde(default)]
    pub titles: Vec<VndbTitle>,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default)]
    pub developers: Vec<VndbProducer>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<VndbImage>,
    #[serde(default)]
    pub tags: Vec<VndbTag>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub votecount: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VndbTitle {
    pub title: String,
    pub lang: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VndbProducer {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VndbImage {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VndbTag {
    pub id: String,
    pub name: String,
    pub rating: f64,
}

/// VNDB query filter.
#[derive(Debug, Serialize)]
struct VndbQuery {
    filters: serde_json::Value,
    fields: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<u32>,
}

impl VndbClient {
    pub fn new(rate_limiter: RateLimiter) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("Galroon/0.5.0 (galgame-library-manager)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { http, rate_limiter }
    }

    /// Search VNDB by title. Returns up to `limit` results.
    pub async fn search_by_title(&self, title: &str, limit: u32) -> Result<Vec<VndbVn>, String> {
        self.rate_limiter.acquire("vndb").await;

        let query = VndbQuery {
            filters: serde_json::json!(["search", "=", title]),
            fields: "id, title, alttitle, titles.title, titles.lang, titles.official, titles.main, released, developers.id, developers.name, description, image.url, tags.id, tags.name, tags.rating, rating, votecount".to_string(),
            results: Some(limit),
        };

        debug!(title = %title, "VNDB search request");

        let resp = self
            .http
            .post(format!("{}/vn", VNDB_API_URL))
            .json(&query)
            .send()
            .await
            .map_err(|e| format!("VNDB request failed: {}", e))?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("vndb").await;
            return Err("Rate limited by VNDB (429)".to_string());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "VNDB API error");
            return Err(format!("VNDB API error: {} - {}", status, body));
        }

        let data: VndbResponse = resp
            .json()
            .await
            .map_err(|e| format!("VNDB parse error: {}", e))?;

        info!(
            title = %title,
            results = data.results.len(),
            "VNDB search complete"
        );

        Ok(data.results)
    }

    /// Fetch a single VN by VNDB ID (e.g., "v12345").
    pub async fn get_by_id(&self, vndb_id: &str) -> Result<Option<VndbVn>, String> {
        self.rate_limiter.acquire("vndb").await;

        let query = VndbQuery {
            filters: serde_json::json!(["id", "=", vndb_id]),
            fields: "id, title, alttitle, titles.title, titles.lang, titles.official, titles.main, released, developers.id, developers.name, description, image.url, tags.id, tags.name, tags.rating, rating, votecount".to_string(),
            results: Some(1),
        };

        let resp = self
            .http
            .post(format!("{}/vn", VNDB_API_URL))
            .json(&query)
            .send()
            .await
            .map_err(|e| format!("VNDB request failed: {}", e))?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("vndb").await;
            return Err("Rate limited by VNDB (429)".to_string());
        }

        let data: VndbResponse = resp
            .json()
            .await
            .map_err(|e| format!("VNDB parse error: {}", e))?;

        Ok(data.results.into_iter().next())
    }
}

pub fn preferred_display_title(vn: &VndbVn) -> String {
    vn.titles
        .iter()
        .find(|title| title.lang == "ja" && title.main)
        .or_else(|| {
            vn.titles
                .iter()
                .find(|title| title.lang == "ja" && title.official)
        })
        .map(|title| title.title.clone())
        .or_else(|| vn.alttitle.clone())
        .unwrap_or_else(|| vn.title.clone())
}

pub fn candidate_titles(vn: &VndbVn) -> Vec<String> {
    let mut titles = Vec::new();
    titles.push(vn.title.clone());
    if let Some(title) = &vn.alttitle {
        if !titles.iter().any(|existing| existing == title) {
            titles.push(title.clone());
        }
    }
    for title in &vn.titles {
        if !titles.iter().any(|existing| existing == &title.title) {
            titles.push(title.title.clone());
        }
    }
    titles
}

//! DLsite API client — search by RJ code + keyword.
//!
//! DLsite doesn't have an official public API, so we scrape the
//! product page and use the AJAX search endpoint.
//!
//! Endpoints:
//!   Search: https://www.dlsite.com/maniax/fsr/=/language/jp/keyword/{term}/order/trend
//!   Product: https://www.dlsite.com/maniax/work/=/product_id/{RJ_CODE}.html
//!   API (unofficial): https://www.dlsite.com/maniax/product/info/ajax?product_id={RJ_CODE}

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::rate_limit::RateLimiter;

const DLSITE_API_URL: &str = "https://www.dlsite.com/maniax/product/info/ajax";

/// DLsite API client.
#[derive(Clone)]
pub struct DlsiteClient {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
}

/// A product entry from DLsite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlsiteProduct {
    pub product_id: String,
    pub product_name: Option<String>,
    pub maker_name: Option<String>,
    pub maker_id: Option<String>,
    pub price: Option<String>,
    pub work_type: Option<String>,
    pub age_category: Option<String>,
    pub regist_date: Option<String>,
    pub image_main: Option<String>,
    pub genres: Vec<String>,
    pub description: Option<String>,
    pub dl_count: Option<String>,
    pub rate_average: Option<f64>,
    pub rate_count: Option<u32>,
}

/// Raw DLsite AJAX response (product_id → product info map).
#[derive(Debug, Deserialize)]
struct DlsiteAjaxEntry {
    product_name: Option<String>,
    maker_name: Option<String>,
    maker_id: Option<String>,
    price: Option<serde_json::Value>,
    work_type: Option<String>,
    age_category: Option<String>,
    regist_date: Option<String>,
    image_main: Option<serde_json::Value>,
    #[serde(default)]
    genre: Option<Vec<DlsiteGenre>>,
    intro: Option<String>,
    dl_count: Option<serde_json::Value>,
    rate_average: Option<serde_json::Value>,
    rate_count: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DlsiteGenre {
    name: Option<String>,
}

impl DlsiteClient {
    pub fn new(rate_limiter: RateLimiter) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("Galroon/0.5.0 (galgame-library-manager)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { http, rate_limiter }
    }

    /// Fetch product info by RJ code (e.g., "RJ123456").
    pub async fn get_by_rj_code(&self, rj_code: &str) -> Result<Option<DlsiteProduct>, String> {
        self.rate_limiter.acquire("dlsite").await;

        let code = rj_code.to_uppercase();
        let url = format!("{}?product_id={}", DLSITE_API_URL, code);

        debug!(rj_code = %code, "DLsite product lookup");

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("DLsite request failed: {}", e))?;

        if resp.status() == 429 {
            self.rate_limiter.signal_rate_limited("dlsite").await;
            return Err("Rate limited by DLsite (429)".to_string());
        }

        if resp.status() == 404 || !resp.status().is_success() {
            return Ok(None);
        }

        let data: std::collections::HashMap<String, DlsiteAjaxEntry> = resp
            .json()
            .await
            .map_err(|e| format!("DLsite parse error: {}", e))?;

        let product = data.into_iter().next().map(|(id, entry)| DlsiteProduct {
            product_id: id,
            product_name: entry.product_name,
            maker_name: entry.maker_name,
            maker_id: entry.maker_id,
            price: entry
                .price
                .map(|v| v.to_string().trim_matches('"').to_string()),
            work_type: entry.work_type,
            age_category: entry.age_category,
            regist_date: entry.regist_date,
            image_main: entry
                .image_main
                .map(|v| v.to_string().trim_matches('"').to_string()),
            genres: entry
                .genre
                .unwrap_or_default()
                .into_iter()
                .filter_map(|g| g.name)
                .collect(),
            description: entry.intro,
            dl_count: entry
                .dl_count
                .map(|v| v.to_string().trim_matches('"').to_string()),
            rate_average: entry.rate_average.and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            }),
            rate_count: entry.rate_count.and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_u64().map(|x| x as u32),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            }),
        });

        if let Some(ref p) = product {
            info!(rj_code = %code, title = ?p.product_name, "DLsite product found");
        }

        Ok(product)
    }
}

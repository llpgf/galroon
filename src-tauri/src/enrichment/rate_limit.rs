//! Rate limiter — per-provider GCRA via `governor` crate (R8).
//!
//! Each API provider (VNDB, Bangumi) has its own rate-limited quota.
//! Handles 429 responses with automatic backoff.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use governor::{
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovLimiter,
};
use tokio::sync::Mutex;
use tracing::{debug, warn};

type GovRateLimiter = GovLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Per-provider state: governor limiter + 429 backoff tracking.
struct ProviderState {
    limiter: GovRateLimiter,
    backoff_until: Option<Instant>,
    backoff_duration: Duration,
}

/// Shared rate limiter for all API providers.
#[derive(Clone)]
pub struct RateLimiter {
    providers: Arc<Mutex<HashMap<String, ProviderState>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let mut providers = HashMap::new();

        // VNDB: 10 requests per 60 seconds
        let vndb_quota = Quota::per_minute(NonZeroU32::new(10).unwrap());
        providers.insert(
            "vndb".to_string(),
            ProviderState {
                limiter: GovLimiter::direct(vndb_quota),
                backoff_until: None,
                backoff_duration: Duration::from_secs(1),
            },
        );

        // Bangumi: 30 requests per 60 seconds
        let bgm_quota = Quota::per_minute(NonZeroU32::new(30).unwrap());
        providers.insert(
            "bangumi".to_string(),
            ProviderState {
                limiter: GovLimiter::direct(bgm_quota),
                backoff_until: None,
                backoff_duration: Duration::from_secs(1),
            },
        );

        // DLsite: 20 requests per 60 seconds
        let dl_quota = Quota::per_minute(NonZeroU32::new(20).unwrap());
        providers.insert(
            "dlsite".to_string(),
            ProviderState {
                limiter: GovLimiter::direct(dl_quota),
                backoff_until: None,
                backoff_duration: Duration::from_secs(1),
            },
        );

        Self {
            providers: Arc::new(Mutex::new(providers)),
        }
    }

    /// Wait until a request to the given provider is allowed.
    pub async fn acquire(&self, provider: &str) {
        loop {
            let wait = {
                let mut providers = self.providers.lock().await;
                if let Some(state) = providers.get_mut(provider) {
                    // Check 429 backoff first
                    if let Some(until) = state.backoff_until {
                        if Instant::now() < until {
                            Some(until - Instant::now())
                        } else {
                            state.backoff_until = None;
                            state.backoff_duration = Duration::from_secs(1);
                            None
                        }
                    } else {
                        // Use governor for normal rate limiting
                        match state.limiter.check() {
                            Ok(()) => None,
                            Err(not_until) => {
                                Some(not_until.wait_time_from(DefaultClock::default().now()))
                            }
                        }
                    }
                } else {
                    None // Unknown provider = no limit
                }
            };

            match wait {
                None => {
                    debug!(provider = %provider, "Rate limit token acquired");
                    return;
                }
                Some(duration) => {
                    debug!(provider = %provider, wait_ms = duration.as_millis(), "Waiting for rate limit");
                    tokio::time::sleep(duration).await;
                }
            }
        }
    }

    /// Signal that a 429 was received — exponential backoff, capped at 60s.
    pub async fn signal_rate_limited(&self, provider: &str) {
        let mut providers = self.providers.lock().await;
        if let Some(state) = providers.get_mut(provider) {
            let backoff = state.backoff_duration;
            warn!(provider = %provider, backoff_ms = backoff.as_millis(), "429 received, backing off (R8)");
            state.backoff_until = Some(Instant::now() + backoff);
            state.backoff_duration = (backoff * 2).min(Duration::from_secs(60));
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

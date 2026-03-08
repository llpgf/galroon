//! Observability — structured logging, metrics, debug bundle (R18).

mod debug_bundle;
mod metrics;

pub use debug_bundle::export_debug_bundle;
pub use metrics::Metrics;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize the tracing subscriber with structured JSON logging.
pub fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("galroon=info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .init();
}

//! Application metrics — key counters for scan and enrichment latency.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Application-level metrics counters.
#[derive(Debug, Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    scans_completed: AtomicU64,
    scan_total_ms: AtomicU64,
    works_ingested: AtomicU64,
    enrichment_jobs_completed: AtomicU64,
    enrichment_total_ms: AtomicU64,
    enrichment_failures: AtomicU64,
    thumbnails_generated: AtomicU64,
    db_writes: AtomicU64,
}

/// Snapshot of metrics for serialization.
#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub scans_completed: u64,
    pub scan_avg_ms: u64,
    pub works_ingested: u64,
    pub enrichment_jobs_completed: u64,
    pub enrichment_avg_ms: u64,
    pub enrichment_failures: u64,
    pub thumbnails_generated: u64,
    pub db_writes: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                scans_completed: AtomicU64::new(0),
                scan_total_ms: AtomicU64::new(0),
                works_ingested: AtomicU64::new(0),
                enrichment_jobs_completed: AtomicU64::new(0),
                enrichment_total_ms: AtomicU64::new(0),
                enrichment_failures: AtomicU64::new(0),
                thumbnails_generated: AtomicU64::new(0),
                db_writes: AtomicU64::new(0),
            }),
        }
    }

    pub fn record_scan(&self, duration_ms: u64, works_count: u64) {
        self.inner.scans_completed.fetch_add(1, Ordering::Relaxed);
        self.inner
            .scan_total_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.inner
            .works_ingested
            .fetch_add(works_count, Ordering::Relaxed);
    }

    pub fn record_enrichment(&self, duration_ms: u64, success: bool) {
        self.inner
            .enrichment_jobs_completed
            .fetch_add(1, Ordering::Relaxed);
        self.inner
            .enrichment_total_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        if !success {
            self.inner
                .enrichment_failures
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_thumbnail(&self) {
        self.inner
            .thumbnails_generated
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_db_write(&self) {
        self.inner.db_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Take a snapshot for serialization.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let scans = self.inner.scans_completed.load(Ordering::Relaxed);
        let scan_ms = self.inner.scan_total_ms.load(Ordering::Relaxed);
        let enrich = self.inner.enrichment_jobs_completed.load(Ordering::Relaxed);
        let enrich_ms = self.inner.enrichment_total_ms.load(Ordering::Relaxed);

        MetricsSnapshot {
            scans_completed: scans,
            scan_avg_ms: if scans > 0 { scan_ms / scans } else { 0 },
            works_ingested: self.inner.works_ingested.load(Ordering::Relaxed),
            enrichment_jobs_completed: enrich,
            enrichment_avg_ms: if enrich > 0 { enrich_ms / enrich } else { 0 },
            enrichment_failures: self.inner.enrichment_failures.load(Ordering::Relaxed),
            thumbnails_generated: self.inner.thumbnails_generated.load(Ordering::Relaxed),
            db_writes: self.inner.db_writes.load(Ordering::Relaxed),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

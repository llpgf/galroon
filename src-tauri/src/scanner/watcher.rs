//! Watcher — filesystem event monitoring with noise suppression.
//!
//! Mitigations:
//! - R3: Bounded channel (1024) + dirty-root HashSet folding + timer flush
//! - R20: Self-write suppression via recent_writes time-window filter

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Configuration for the file watcher.
pub struct WatcherConfig {
    /// Channel capacity for bounded backpressure (R3)
    pub channel_capacity: usize,
    /// Flush interval: how long to wait after last event before triggering scan
    pub flush_interval: Duration,
    /// Self-write suppression window (R20)
    pub self_write_window: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1024,
            flush_interval: Duration::from_millis(500),
            self_write_window: Duration::from_secs(2),
        }
    }
}

/// Tracks recent writes by the app for self-write suppression (R20).
///
/// Shared between the writer (metadata_io) and the watcher.
#[derive(Debug, Clone)]
pub struct RecentWrites {
    inner: Arc<Mutex<HashMap<PathBuf, Instant>>>,
}

impl RecentWrites {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record that the app wrote to a path (called from metadata_io).
    pub fn record(&self, path: PathBuf) {
        let mut map = self.inner.lock().unwrap();
        map.insert(path, Instant::now());
    }

    /// Check if a path was recently written by the app (R20).
    pub fn is_self_write(&self, path: &PathBuf, window: Duration) -> bool {
        let map = self.inner.lock().unwrap();
        if let Some(write_time) = map.get(path) {
            if write_time.elapsed() < window {
                return true;
            }
        }
        false
    }

    /// Purge stale entries older than the given duration.
    pub fn purge_stale(&self, max_age: Duration) {
        let mut map = self.inner.lock().unwrap();
        map.retain(|_, time| time.elapsed() < max_age);
    }
}

impl Default for RecentWrites {
    fn default() -> Self {
        Self::new()
    }
}

/// A dirty root that needs to be re-scanned.
/// Events are folded to the nearest library-root-child (game folder).
fn to_dirty_root(path: &PathBuf, library_roots: &[PathBuf]) -> Option<PathBuf> {
    for root in library_roots {
        if path.starts_with(root) {
            // Get the first component after the library root = the game folder
            let relative = path.strip_prefix(root).ok()?;
            let first_component = relative.components().next()?;
            return Some(root.join(first_component));
        }
    }
    None
}

/// Start the filesystem watcher.
///
/// Returns a receiver that yields sets of dirty game folder roots.
/// The watcher folds events into a HashSet and flushes periodically.
pub fn start_watcher(
    library_roots: Vec<PathBuf>,
    config: WatcherConfig,
    recent_writes: RecentWrites,
) -> Result<mpsc::Receiver<HashSet<PathBuf>>, notify::Error> {
    let (dirty_tx, dirty_rx) = mpsc::channel::<HashSet<PathBuf>>(8);
    let (event_tx, mut event_rx) = mpsc::channel::<PathBuf>(config.channel_capacity);

    // Start the notify watcher
    let roots_clone = library_roots.clone();
    let event_tx_clone = event_tx.clone();
    let rw_clone = recent_writes.clone();
    let self_write_window = config.self_write_window;

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only care about create, modify, remove, rename events
                let dominated = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if !dominated {
                    return;
                }

                for path in event.paths {
                    // R20: Check if this is a self-write
                    if rw_clone.is_self_write(&path, self_write_window) {
                        debug!(path = %path.display(), "Suppressed self-write event (R20)");
                        continue;
                    }

                    // Fold to dirty root (game folder level)
                    if let Some(dirty_root) = to_dirty_root(&path, &roots_clone) {
                        // Bounded channel (R3): try_send, don't block
                        if event_tx_clone.try_send(dirty_root).is_err() {
                            // Channel full — backpressure (R3)
                            // The dirty root is likely already queued
                            debug!("Watcher event channel full, backpressure active (R3)");
                        }
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    // Watch all library roots
    for root in &library_roots {
        if root.is_dir() {
            watcher.watch(root, RecursiveMode::Recursive)?;
            info!(root = %root.display(), "Watching library root");
        }
    }

    // Spawn the event folder + flush timer task
    let flush_interval = config.flush_interval;
    tokio::spawn(async move {
        // Keep ownership of watcher to prevent it from being dropped
        let _watcher = watcher;
        let mut dirty_set: HashSet<PathBuf> = HashSet::new();
        let mut flush_deadline: Option<tokio::time::Instant> = None;

        loop {
            tokio::select! {
                // Receive individual path events
                Some(dirty_root) = event_rx.recv() => {
                    dirty_set.insert(dirty_root);

                    // Reset flush timer on each new event
                    flush_deadline = Some(
                        tokio::time::Instant::now() + flush_interval
                    );
                }

                // Flush timer fires
                _ = async {
                    match flush_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    if !dirty_set.is_empty() {
                        let batch = std::mem::take(&mut dirty_set);
                        info!(dirty_roots = batch.len(), "Flushing dirty roots for re-scan");

                        if dirty_tx.send(batch).await.is_err() {
                            warn!("Dirty root receiver dropped, stopping watcher");
                            break;
                        }
                    }
                    flush_deadline = None;

                    // Periodically purge stale self-write records
                    recent_writes.purge_stale(Duration::from_secs(10));
                }
            }
        }
    });

    Ok(dirty_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_dirty_root() {
        let roots = vec![PathBuf::from("/games")];

        // File deep in a game folder → folds to game folder
        let result = to_dirty_root(&PathBuf::from("/games/my_game/save/data.sav"), &roots);
        assert_eq!(result, Some(PathBuf::from("/games/my_game")));

        // File directly in library root → folds to that file
        let result = to_dirty_root(&PathBuf::from("/games/my_game"), &roots);
        assert_eq!(result, Some(PathBuf::from("/games/my_game")));

        // File outside library roots → None
        let result = to_dirty_root(&PathBuf::from("/other/something"), &roots);
        assert_eq!(result, None);
    }

    #[test]
    fn test_recent_writes_suppression() {
        let rw = RecentWrites::new();
        let path = PathBuf::from("/games/test/metadata.json");

        // Not recorded yet
        assert!(!rw.is_self_write(&path, Duration::from_secs(2)));

        // Record it
        rw.record(path.clone());

        // Should be suppressed within window
        assert!(rw.is_self_write(&path, Duration::from_secs(2)));
    }
}

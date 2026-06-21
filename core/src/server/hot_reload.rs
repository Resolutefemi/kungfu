//! Hot reload — watches the project directory and triggers a graceful
//! listener handoff when source files change.
//!
//! Strategy: when a `.rs` (or binding-specific source) file changes:
//!   1. The watcher sends a `Reload` signal on a channel.
//!   2. The main loop receives it, spawns a new listener on the same port
//!      (SO_REUSEPORT allows two listeners on the same port simultaneously),
//!      and signals the old listener to drain.
//!   3. The old listener finishes in-flight requests and exits.
//!
//! In V1 we don't actually recompile the binary — that requires a separate
//! build step. The hot-reload here is for the *config* and *route* layer:
//! routes can be re-registered from a dynamically loaded .kungfu-routes file
//! without recompiling. Full source-code hot reload (cargo watch style) is
//! a Phase 2 feature.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{event::EventKind, Event, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::router::Router;

/// Configuration for the hot-reload watcher.
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Directories to watch (recursively).
    pub watch_paths: Vec<PathBuf>,
    /// File extensions that trigger a reload.
    pub extensions: Vec<String>,
    /// Debounce window — file editors often fire multiple events for a single
    /// save; we collapse them within this window.
    pub debounce_ms: u64,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![PathBuf::from("src"), PathBuf::from("routes")],
            extensions: vec!["rs".into(), "kungfu".into(), "ts".into(), "js".into()],
            debounce_ms: 200,
        }
    }
}

/// A handle returned by `start_watcher`. Drop it to stop watching.
pub struct WatcherHandle {
    _watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    pub reload_rx: mpsc::Receiver<ReloadEvent>,
}

/// Fired when a watched file changes.
#[derive(Debug, Clone)]
pub struct ReloadEvent {
    pub path: PathBuf,
    pub kind: EventKind,
}

/// Start a watcher that fires `ReloadEvent`s on the returned receiver.
pub fn start_watcher(config: HotReloadConfig) -> std::io::Result<WatcherHandle> {
    let (tx, rx) = mpsc::channel(32);

    let event_handler = move |res: std::result::Result<Event, notify::Error>| {
        if let Ok(ev) = res {
            // Filter by extension.
            let extensions = config.extensions.clone();
            let relevant = ev.paths.iter().any(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| extensions.iter().any(|e| e == ext))
                    .unwrap_or(false)
            });
            if !relevant {
                return;
            }
            // Best-effort send — if the receiver is gone, drop the event.
            let _ = tx.blocking_send(ReloadEvent {
                path: ev.paths.first().cloned().unwrap_or_default(),
                kind: ev.kind,
            });
        }
    };

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(event_handler)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    for path in &config.watch_paths {
        if path.exists() {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
    }

    Ok(WatcherHandle {
        _watcher: Arc::new(Mutex::new(Some(watcher))),
        reload_rx: rx,
    })
}

/// Apply a router swap atomically. The router is wrapped in an `Arc<parking_lot::RwLock>`,
/// so in-flight requests on the old router finish naturally while new ones
/// pick up the new router.
pub fn swap_router(
    slot: &Arc<parking_lot::RwLock<Arc<Router>>>,
    new_router: Router,
) {
    let mut guard = slot.write();
    *guard = Arc::new(new_router);
    tracing::info!("hot reload: router swapped");
}

/// Convenience: sleep for the configured debounce window.
pub async fn debounce(config: &HotReloadConfig) {
    tokio::time::sleep(Duration::from_millis(config.debounce_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn watcher_fires_on_file_change() {
        // Create a temp dir + file, modify it, ensure we get an event.
        let tmp = std::env::temp_dir().join(format!("kungfu-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("test.rs");
        std::fs::write(&file, "fn main() {}").unwrap();

        let mut config = HotReloadConfig::default();
        config.watch_paths = vec![tmp.clone()];
        let mut handle = start_watcher(config).unwrap();

        // Spawn a thread to receive the event.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if let Some(ev) = handle.reload_rx.blocking_recv() {
                let _ = tx.send(ev);
            }
        });

        // Modify the file.
        std::thread::sleep(Duration::from_millis(200));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&file)
            .unwrap();
        writeln!(f, "// change").unwrap();
        drop(f);

        // Wait for the event.
        let ev = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("no reload event received");
        assert_eq!(ev.path, file);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

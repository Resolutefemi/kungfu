//! WebSocket-based live reload server.
//!
//! In dev mode, every SSR page is injected with a `<script>` that opens
//! a WebSocket to `ws://host/__kungfu_livereload`. When the file watcher
//! detects a change, this server broadcasts a `reload` message to every
//! connected client.
//!
//! V1 ships a minimal WS implementation that's enough for dev work. For
//! production use we recommend running behind a reverse proxy that handles
//! WebSocket upgrade separately.

use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

/// A live-reload server. Cheap to clone — state is behind an `Arc`.
#[derive(Clone)]
pub struct LiveReloadServer {
    /// Broadcasts a `()` to all connected clients when a reload should fire.
    tx: broadcast::Sender<()>,
    /// Number of currently connected clients.
    client_count: Arc<Mutex<usize>>,
}

impl LiveReloadServer {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            tx,
            client_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Subscribe to reload events. Each connected client gets its own receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    /// Broadcast a reload signal to all connected clients.
    pub fn trigger_reload(&self) {
        let _ = self.tx.send(());
    }

    pub async fn client_count(&self) -> usize {
        *self.client_count.lock().await
    }

    pub async fn inc_client(&self) {
        *self.client_count.lock().await += 1;
    }

    pub async fn dec_client(&self) {
        let mut count = self.client_count.lock().await;
        if *count > 0 {
            *count -= 1;
        }
    }
}

impl Default for LiveReloadServer {
    fn default() -> Self {
        Self::new()
    }
}

/// The JavaScript client injected into every dev-mode page. Opens a
/// WebSocket and calls `window.location.reload()` on message.
pub const LIVERELOAD_CLIENT_JS: &str = r#"
(function() {
  const ws = new WebSocket(`ws://${location.host}/__kungfu_livereload`);
  ws.onmessage = (ev) => {
    if (ev.data === 'reload') {
      console.log('[kungfu] file change detected — reloading');
      window.location.reload();
    }
  };
  ws.onclose = () => {
    console.log('[kungfu] live reload disconnected — retrying in 1s');
    setTimeout(() => location.reload(), 1000);
  };
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_reaches_all_subscribers() {
        let server = LiveReloadServer::new();
        let mut rx1 = server.subscribe();
        let mut rx2 = server.subscribe();

        server.trigger_reload();

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn tracks_client_count() {
        let server = LiveReloadServer::new();
        assert_eq!(server.client_count().await, 0);
        server.inc_client().await;
        server.inc_client().await;
        assert_eq!(server.client_count().await, 2);
        server.dec_client().await;
        assert_eq!(server.client_count().await, 1);
    }
}

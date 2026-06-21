//! Top-level `Kungfu` builder.

use std::net::SocketAddr;

use crate::Result;
use crate::{Handler, Router, Server};
use crate::Middleware;

/// Builder for the Kungfu application. Consumes itself on `run`.
pub struct KungfuBuilder {
    router: Router,
    /// If true, register `/openapi.json` + `/docs` automatically on `run`.
    auto_docs: bool,
    /// If true, skip the default secure-by-default middleware stack.
    insecure: bool,
    /// Title used in the generated OpenAPI spec.
    title: String,
    /// Version used in the generated OpenAPI spec.
    version: String,
}

impl Default for KungfuBuilder {
    fn default() -> Self {
        Self {
            router: Router::new(),
            auto_docs: true,
            insecure: false,
            title: "Kungfu API".into(),
            version: kungfu_core::VERSION.to_string(),
        }
    }
}

impl KungfuBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a route from one of the `#[get]` / `#[post]` / etc. macros.
    pub fn route<F>(mut self, register: F) -> Self
    where
        F: FnOnce(&mut Router) -> Result<()>,
    {
        if let Err(e) = register(&mut self.router) {
            tracing::error!("route registration failed: {e}");
        }
        self
    }

    /// Add a handler directly. Lower-level than `.route()`.
    pub fn add(mut self, method: crate::Method, path: &str, handler: Handler) -> Self {
        let _ = self.router.add(
            method,
            path,
            handler,
            crate::RouteMeta {
                path: path.to_string(),
                method,
                ..Default::default()
            },
        );
        self
    }

    pub fn get(self, path: &str, handler: Handler) -> Self {
        self.add(crate::Method::Get, path, handler)
    }

    pub fn post(self, path: &str, handler: Handler) -> Self {
        self.add(crate::Method::Post, path, handler)
    }

    pub fn put(self, path: &str, handler: Handler) -> Self {
        self.add(crate::Method::Put, path, handler)
    }

    pub fn delete(self, path: &str, handler: Handler) -> Self {
        self.add(crate::Method::Delete, path, handler)
    }

    pub fn patch(self, path: &str, handler: Handler) -> Self {
        self.add(crate::Method::Patch, path, handler)
    }

    /// Register a route with full OpenAPI metadata.
    pub fn add_with_meta(mut self, meta: crate::RouteMeta, handler: Handler) -> Self {
        let _ = self.router.add_with_meta(meta, handler);
        self
    }

    /// Register a global middleware. Applied in registration order.
    pub fn use_middleware(mut self, mw: Middleware) -> Self {
        self.router.use_middleware(mw);
        self
    }

    /// Disable the default secure-by-default middleware stack. The framework
    /// emits a `tracing::warn!` per spec.
    pub fn insecure(mut self) -> Self {
        tracing::warn!(
            "Kungfu: insecure() called — default secure-by-default middleware \
             (security headers, CORS, rate limiter, logger) will NOT be installed."
        );
        self.insecure = true;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn disable_auto_docs(mut self) -> Self {
        self.auto_docs = false;
        self
    }

    /// Take ownership of the configured router — useful for tests.
    pub fn into_router(mut self) -> Router {
        self.apply_defaults();
        self.router
    }

    fn apply_defaults(&mut self) {
        if !self.insecure {
            for mw in crate::default_middleware_stack().into_iter().rev() {
                self.router.prepend_middleware(mw);
            }
        }
        if self.auto_docs {
            let _ = crate::openapi::register_docs_routes(
                &mut self.router,
                &self.title,
                &self.version,
            );
        }
    }

    /// Bind to `addr` and serve forever.
    pub async fn run(mut self, addr: &str) -> Result<()> {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                crate::KungfuError::internal(format!("invalid bind addr: {e}"))
            })?;

        self.apply_defaults();
        // The Server wraps the router in an Arc<RwLock<Arc<Router>>> for
        // hot-reload support. We pass ownership of the router in.
        let server = Server::new(self.router, addr);
        server.serve().await
    }

    /// Bind to `addr` and serve forever, with hot-reload enabled.
    ///
    /// Watches the configured directories for source file changes. When a
    /// change is detected, a `reload_callback` is invoked; the callback
    /// receives the current router slot and may swap in a new router via
    /// `kungfu_core::server::swap_router`.
    pub async fn run_with_hot_reload<F>(
        mut self,
        addr: &str,
        config: crate::server::HotReloadConfig,
        mut reload_callback: F,
    ) -> Result<()>
    where
        F: FnMut(&std::sync::Arc<crate::server::RouterSlot>) + Send + 'static,
    {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                crate::KungfuError::internal(format!("invalid bind addr: {e}"))
            })?;

        self.apply_defaults();
        let server = Server::new(self.router, addr);
        let router_slot = server.router.clone();

        // Start the file watcher.
        let mut watcher = crate::server::start_watcher(config)
            .map_err(|e| crate::KungfuError::internal(format!("watcher: {e}")))?;

        // Spawn the server task.
        let server_task = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        // Drain watcher events and invoke the callback.
        while let Some(_event) = watcher.reload_rx.recv().await {
            tracing::info!("hot reload: change detected, invoking callback");
            reload_callback(&router_slot);
        }
        let _ = server_task.await;
        Ok(())
    }
}

/// Entry point — `Kungfu::new()` returns a `KungfuBuilder`.
pub struct Kungfu;

impl Kungfu {
    pub fn new() -> KungfuBuilder {
        KungfuBuilder::new()
    }
}

// `Kungfu` itself has no state, so we don't implement `Default` for it —
// call `Kungfu::new()` to get a builder.

//! Node.js binding for the Kungfu.js framework.
//!
//! Exposes a `Kungfu` class with `.get()`, `.post()`, `.listen()` methods
//! that feel idiomatic to JavaScript/TypeScript developers.
//!
//! Under the hood, every call dispatches into the Rust core via napi-rs.
//! The router state lives in Rust; JavaScript handlers are wrapped in
//! `threadsafe_function`s so the Rust side can call back into JS.

#![deny(clippy::all)]

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::tokio::sync::Mutex;
use napi_derive::napi;

use kungfu_core::{
    Method, Request as CoreRequest, Response as CoreResponse, Router as CoreRouter,
    Server as CoreServer,
};

/// A Kungfu application. Construct with `new Kungfu()`.
#[napi]
pub struct Kungfu {
    router: Arc<Mutex<CoreRouter>>,
}

#[napi]
impl Kungfu {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            router: Arc::new(Mutex::new(CoreRouter::new())),
        }
    }

    /// Register a GET route.
    #[napi]
    pub async fn get(
        &self,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        self.add(Method::Get, path, handler).await
    }

    /// Register a POST route.
    #[napi]
    pub async fn post(
        &self,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        self.add(Method::Post, path, handler).await
    }

    /// Register a PUT route.
    #[napi]
    pub async fn put(
        &self,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        self.add(Method::Put, path, handler).await
    }

    /// Register a DELETE route.
    #[napi]
    pub async fn delete(
        &self,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        self.add(Method::Delete, path, handler).await
    }

    /// Register a PATCH route.
    #[napi]
    pub async fn patch(
        &self,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        self.add(Method::Patch, path, handler).await
    }

    async fn add(
        &self,
        method: Method,
        path: String,
        handler: ThreadsafeFunction<JsRequest, ErrorStrategy::CalleeHandled>,
    ) -> Result<()> {
        let mut router = self.router.lock().await;

        // The handler Arc wraps the ThreadsafeFunction — it can be called
        // from any tokio worker thread. We use `call_async` to await the
        // JS function's return value.
        let handler_arc: kungfu_core::Handler = {
            let handler = Arc::new(handler);
            Arc::new(move |req: CoreRequest| {
                let handler = handler.clone();
                Box::pin(async move {
                    let js_req = JsRequest::from_core(req);
                    // Call the JS handler. It returns a JsResponse which we
                    // convert back to a CoreResponse.
                    match handler.call_async::<JsRequest, JsResponse>(js_req).await {
                        Ok(js_resp) => js_resp.into_core(),
                        Err(e) => CoreResponse::new().error(
                            kungfu_core::KungfuError::internal(format!("JS handler error: {e:?}")),
                        ),
                    }
                })
            })
        };

        let meta = kungfu_core::RouteMeta {
            path: path.clone(),
            method,
            ..Default::default()
        };
        router.add(method, &path, handler_arc, meta).map_err(|e| {
            Error::new(Status::GenericFailure, format!("route registration: {e}"))
        })
    }

    /// Start the server on the given port. Blocks the calling thread.
    #[napi]
    pub async fn listen(&self, port: u16) -> Result<()> {
        let router = {
            let mut guard = self.router.lock().await;
            // Apply the default secure-by-default middleware + auto docs.
            for mw in kungfu_core::default_middleware_stack().into_iter().rev() {
                guard.prepend_middleware(mw);
            }
            let _ = kungfu_core::openapi::register_docs_routes(
                &mut guard,
                "Kungfu API",
                kungfu_core::VERSION,
            );
            guard
        };

        let addr: std::net::SocketAddr = format!("0.0.0.0:{port}")
            .parse()
            .map_err(|e| Error::new(Status::InvalidArg, format!("invalid port: {e}")))?;

        let server = CoreServer::new(router, addr);
        server
            .serve()
            .await
            .map_err(|e| Error::new(Status::GenericFailure, format!("server: {e}")))
    }
}

/// The request object passed to JS handlers.
#[napi(object)]
pub struct JsRequest {
    pub method: String,
    pub path: String,
    pub query: serde_json::Value,
    pub params: serde_json::Value,
    pub headers: serde_json::Value,
    pub body: serde_json::Value,
    pub raw_body: Buffer,
}

impl JsRequest {
    fn from_core(req: CoreRequest) -> Self {
        let method = req.method.as_str().to_string();
        let path = req.path.clone();
        let query: serde_json::Value =
            serde_json::to_value(&req.query).unwrap_or(serde_json::json!({}));
        let params: serde_json::Value =
            serde_json::to_value(&req.params).unwrap_or(serde_json::json!({}));
        let headers: serde_json::Value = {
            let mut map = serde_json::Map::new();
            for (k, v) in &req.headers {
                map.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            serde_json::Value::Object(map)
        };
        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);
        let raw_body = Buffer::from(req.body);

        Self {
            method,
            path,
            query,
            params,
            headers,
            body,
            raw_body,
        }
    }
}

/// The response object returned by JS handlers.
#[napi(object)]
pub struct JsResponse {
    pub status: u16,
    pub headers: serde_json::Value,
    pub body: serde_json::Value,
}

impl JsResponse {
    fn into_core(self) -> CoreResponse {
        let status = kungfu_core::StatusCode::from(self.status);
        let mut resp = CoreResponse::new().status(status);
        if let serde_json::Value::Object(map) = self.headers {
            for (k, v) in map {
                if let serde_json::Value::String(s) = v {
                    resp = resp.header(k, s);
                }
            }
        }
        // Body is always serialised as JSON for now. V2 will support raw bytes.
        resp.json(&self.body)
    }
}

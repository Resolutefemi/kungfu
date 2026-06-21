//! Simpler handler API — closures, no macros.
//!
//! The macro API (`get!`, `post!`, etc.) is powerful but verbose. Many users
//! want to write the simplest possible "hello world" without learning macros.
//! This module provides `KungfuBuilder::handle_get`, `handle_post`, etc.
//! that take plain Rust closures, automatically wrap them in async, and
//! register the route.
//!
//! ## Comparison
//!
//! Before (macros):
//! ```ignore
//! let hello = get!("/hello", |_req: Request| {
//!     Response::new().json(&serde_json::json!({"message":"world"}))
//! });
//! Kungfu::new().route(hello).run("0.0.0.0:3000")
//! ```
//!
//! After (closures, this module):
//! ```ignore
//! Kungfu::new()
//!     .handle_get("/hello", |_req, res| res.json(&serde_json::json!({"message":"world"})))
//!     .run("0.0.0.0:3000")
//! ```
//!
//! No macros, no `Arc::new`, no `Box::pin`, no `async move`. Just closures.

use std::sync::Arc;

use crate::{Handler, KungfuBuilder, Method, Request, Response, RouteMeta};

impl KungfuBuilder {
    /// Register a GET handler using a simple closure.
    ///
    /// The closure receives `Request` and `ResponseBuilder` and returns a `Response`.
    /// This is the simplest possible API — no macros, no async wrappers.
    ///
    /// # Example
    /// ```ignore
    /// Kungfu::new()
    ///     .handle_get("/hello", |_req, res| res.text("world"))
    ///     .run("0.0.0.0:3000")
    /// ```
    pub fn handle_get<F>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        self.handle(Method::Get, path, handler)
    }

    /// Register a POST handler using a simple closure.
    pub fn handle_post<F>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        self.handle(Method::Post, path, handler)
    }

    /// Register a PUT handler using a simple closure.
    pub fn handle_put<F>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        self.handle(Method::Put, path, handler)
    }

    /// Register a DELETE handler using a simple closure.
    pub fn handle_delete<F>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        self.handle(Method::Delete, path, handler)
    }

    /// Register a PATCH handler using a simple closure.
    pub fn handle_patch<F>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        self.handle(Method::Patch, path, handler)
    }

    /// Internal: register any method handler with a simple closure.
    fn handle<F>(self, method: Method, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
    {
        let handler = wrap_handler(handler);
        self.add_with_meta(
            RouteMeta {
                path: path.to_string(),
                method,
                ..Default::default()
            },
            handler,
        )
    }

    /// Register an async GET handler. The closure returns a `Future<Output = Response>`.
    ///
    /// # Example
    /// ```ignore
    /// Kungfu::new()
    ///     .handle_get_async("/users", |req, res| async move {
    ///         let users = User::all(&db).await.unwrap();
    ///         res.json(&users)
    ///     })
    /// ```
    pub fn handle_get_async<F, Fut>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.handle_async(Method::Get, path, handler)
    }

    /// Register an async POST handler.
    pub fn handle_post_async<F, Fut>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.handle_async(Method::Post, path, handler)
    }

    /// Register an async PUT handler.
    pub fn handle_put_async<F, Fut>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.handle_async(Method::Put, path, handler)
    }

    /// Register an async DELETE handler.
    pub fn handle_delete_async<F, Fut>(self, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.handle_async(Method::Delete, path, handler)
    }

    /// Internal: register any method handler with an async closure.
    fn handle_async<F, Fut>(self, method: Method, path: &str, handler: F) -> Self
    where
        F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        let handler = wrap_async_handler(handler);
        self.add_with_meta(
            RouteMeta {
                path: path.to_string(),
                method,
                ..Default::default()
            },
            handler,
        )
    }

    /// Register a GET handler that returns JSON, with even less boilerplate.
    ///
    /// # Example
    /// ```ignore
    /// Kungfu::new()
    ///     .json_get("/hello", || serde_json::json!({"message":"world"}))
    ///     .run("0.0.0.0:3000")
    /// ```
    pub fn json_get<F, V>(self, path: &str, handler: F) -> Self
    where
        F: Fn() -> V + Send + Sync + 'static,
        V: serde::Serialize + 'static,
    {
        self.json_handler(Method::Get, path, move |_req| handler())
    }

    /// Register a POST handler that returns JSON, taking the parsed body.
    pub fn json_post<F, V, B>(self, path: &str, handler: F) -> Self
    where
        F: Fn(B) -> V + Send + Sync + 'static,
        V: serde::Serialize + 'static,
        B: serde::de::DeserializeOwned + Send + 'static,
    {
        let handler = Arc::new(handler);
        let wrapped: Handler = Arc::new(move |req: Request| {
            let h = handler.clone();
            Box::pin(async move {
                match req.json::<B>() {
                    Ok(body) => {
                        let v = h(body);
                        Response::new().json(&v)
                    }
                    Err(e) => Response::new().error(e),
                }
            })
        });
        self.add_with_meta(
            RouteMeta {
                path: path.to_string(),
                method: Method::Post,
                ..Default::default()
            },
            wrapped,
        )
    }

    fn json_handler<F, V>(self, method: Method, path: &str, handler: F) -> Self
    where
        F: Fn(Request) -> V + Send + Sync + 'static,
        V: serde::Serialize + 'static,
    {
        let handler = Arc::new(handler);
        let wrapped: Handler = Arc::new(move |req: Request| {
            let h = handler.clone();
            Box::pin(async move {
                let v = h(req);
                Response::new().json(&v)
            })
        });
        self.add_with_meta(
            RouteMeta {
                path: path.to_string(),
                method,
                ..Default::default()
            },
            wrapped,
        )
    }
}

/// Chainable response builder for the simple-handler API.
///
/// Usage:
/// ```ignore
/// Kungfu::new()
///     .handle_get("/hello", |_req, res| res.status(200).text("world"))
/// ```
#[derive(Debug)]
pub struct ResponseBuilder {
    inner: Response,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        Self {
            inner: Response::new(),
        }
    }

    pub fn status(mut self, code: u16) -> Self {
        self.inner.set_status(crate::StatusCode::from(code));
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.set_header(key, value);
        self
    }

    pub fn text(mut self, s: impl Into<String>) -> Response {
        let s = s.into();
        self.inner
            .headers
            .insert("content-type".into(), "text/plain; charset=utf-8".into());
        self.inner.body = bytes::Bytes::from(s);
        self.inner.finalised = true;
        self.inner
    }

    pub fn html(mut self, s: impl Into<String>) -> Response {
        let s = s.into();
        self.inner
            .headers
            .insert("content-type".into(), "text/html; charset=utf-8".into());
        self.inner.body = bytes::Bytes::from(s);
        self.inner.finalised = true;
        self.inner
    }

    pub fn json(mut self, value: &impl serde::Serialize) -> Response {
        let body = serde_json::to_vec(value).expect("response json serialisation failed");
        self.inner
            .headers
            .insert("content-type".into(), "application/json; charset=utf-8".into());
        self.inner.body = bytes::Bytes::from(body);
        self.inner.finalised = true;
        self.inner
    }

    pub fn send(mut self, data: impl Into<bytes::Bytes>) -> Response {
        self.inner.body = data.into();
        self.inner.finalised = true;
        self.inner
    }

    /// Set an error response from a KungfuError.
    pub fn error(self, err: kungfu_core::KungfuError) -> Response {
        kungfu_core::Response::new().error(err)
    }

    /// Set an error response with a status code + message.
    pub fn error_msg(self, code: u16, message: impl Into<String>) -> Response {
        kungfu_core::Response::new().error(
            kungfu_core::KungfuError::new(
                kungfu_core::StatusCode::from(code),
                message,
            )
        )
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap a simple `(Request, ResponseBuilder) -> Response` closure into a
/// `Handler` (async, Arc<dyn Fn>) that the router expects.
fn wrap_handler<F>(f: F) -> Handler
where
    F: Fn(Request, ResponseBuilder) -> Response + Send + Sync + 'static,
{
    let f = Arc::new(f);
    Arc::new(move |req: Request| {
        let f = f.clone();
        Box::pin(async move {
            let builder = ResponseBuilder::new();
            f(req, builder)
        })
    })
}

/// Wrap an async `(Request, ResponseBuilder) -> Future<Response>` closure
/// into a `Handler` that the router expects.
fn wrap_async_handler<F, Fut>(f: F) -> Handler
where
    F: Fn(Request, ResponseBuilder) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Response> + Send + 'static,
{
    let f = Arc::new(f);
    Arc::new(move |req: Request| {
        let f = f.clone();
        Box::pin(async move {
            let builder = ResponseBuilder::new();
            f(req, builder).await
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_builder_chains() {
        let resp = ResponseBuilder::new()
            .status(201)
            .header("x-test", "yes")
            .text("created");
        assert_eq!(resp.status, crate::StatusCode::Created);
        assert_eq!(resp.header_value("x-test"), Some("yes"));
        assert_eq!(resp.body, bytes::Bytes::from_static(b"created"));
    }

    #[test]
    fn response_builder_json() {
        let resp = ResponseBuilder::new().json(&serde_json::json!({"ok": true}));
        assert_eq!(resp.body, bytes::Bytes::from_static(br#"{"ok":true}"#));
        assert_eq!(
            resp.header_value("content-type"),
            Some("application/json; charset=utf-8")
        );
    }
}

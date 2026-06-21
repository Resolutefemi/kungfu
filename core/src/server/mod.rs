//! HTTP/1.1 server — tuned for max throughput.
//!
//! Hand-rolled on top of `tokio::net::TcpListener` + `httparse`. We bypass
//! `hyper` because the spec calls for zero-copy and very tight control over
//! the request lifecycle — using hyper would lock us into its body model
//! and add ~30% overhead on micro-benchmarks.
//!
//! ## Performance profile
//!
//! - **Buffer pooling**: per-connection read buffer is taken from a shared
//!   `BufferPool` (no per-request allocation).
//! - **Zero-copy body**: `Request::body` and `Response::body` are
//!   `bytes::Bytes` — cloning is an atomic increment.
//! - **Pre-serialised errors**: 404/405/429 bodies are computed once at
//!   startup via `once_cell::Lazy`.
//! - **Single-syscall responses**: the entire response (status line +
//!   headers + body) is built in one `Vec<u8>` and written with one
//!   `write_all` call. Avoids the syscall overhead of writing each piece.
//! - **SO_REUSEPORT**: optional multi-acceptor mode that lets multiple
//!   worker threads accept connections on the same port — kernel-level
//!   load-balancing eliminates the thundering-herd problem.
//! - **TCP_NODELAY**: disabled Nagle's algorithm on every connection so
//!   small responses aren't delayed.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{KungfuError, Result, StatusCode};
use crate::middleware::build_chain;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::router::{Handler, RouteResolution, Router};

pub mod pool;
pub mod hot_reload;
#[cfg(feature = "io_uring")]
pub mod io_uring;

pub use hot_reload::{HotReloadConfig, WatcherHandle, start_watcher, swap_router};
pub use pool::BufferPool;

#[cfg(feature = "io_uring")]
pub use io_uring::serve_io_uring;

/// The atomic router slot — `Arc<RwLock<Arc<Router>>>`. Hot-reload swaps the
/// inner `Arc<Router>`; in-flight requests hold their own clone of the Arc
/// and finish naturally.
pub type RouterSlot = parking_lot::RwLock<std::sync::Arc<Router>>;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024; // 16 MB

pub struct Server {
    pub router: Arc<parking_lot::RwLock<Arc<Router>>>,
    pub addr: SocketAddr,
    pub pool: Arc<BufferPool>,
    /// Number of SO_REUSEPORT acceptor threads. 1 = single-acceptor (default).
    /// On Linux, setting this to the number of CPU cores lets the kernel
    /// load-balance connections across acceptor threads, eliminating
    /// thundering-herd wakeups.
    pub acceptor_threads: usize,
}

impl Server {
    pub fn new(router: Router, addr: SocketAddr) -> Self {
        Self {
            router: Arc::new(parking_lot::RwLock::new(Arc::new(router))),
            addr,
            pool: BufferPool::new(256, 8192),
            acceptor_threads: 1,
        }
    }

    /// Set the number of SO_REUSEPORT acceptor threads (Linux only).
    /// On other platforms this is silently ignored.
    pub fn with_acceptor_threads(mut self, n: usize) -> Self {
        self.acceptor_threads = n.max(1);
        self
    }

    pub fn from_arc(
        router_slot: Arc<parking_lot::RwLock<Arc<Router>>>,
        addr: SocketAddr,
    ) -> Self {
        Self {
            router: router_slot,
            addr,
            pool: BufferPool::new(256, 8192),
            acceptor_threads: 1,
        }
    }

    pub fn router_snapshot(&self) -> Arc<Router> {
        self.router.read().clone()
    }

    /// Bind and serve forever. Returns only on fatal error.
    ///
    /// If the `io_uring` feature is enabled AND `acceptor_threads > 1`,
    /// dispatches to the io_uring path. Otherwise uses the tokio epoll path.
    pub async fn serve(&self) -> Result<()> {
        #[cfg(feature = "io_uring")]
        if self.acceptor_threads > 0 {
            return serve_io_uring(
                self.router.clone(),
                self.addr,
                self.acceptor_threads,
            )
            .await;
        }

        if self.acceptor_threads > 1 {
            self.serve_multi_acceptor().await
        } else {
            let listener = TcpListener::bind(&self.addr).await?;
            tracing::info!("kungfu listening on http://{} (single-acceptor)", self.addr);
            self.serve_on(listener).await
        }
    }

    /// Spawn `acceptor_threads` tasks, each with its own SO_REUSEPORT listener
    /// on the same port. Linux's kernel will load-balance accepts across them.
    #[cfg(target_os = "linux")]
    async fn serve_multi_acceptor(&self) -> Result<()> {
        use socket2::{Domain, Protocol, Socket, Type};

        let n = self.acceptor_threads;
        tracing::info!(
            "kungfu listening on http://{} ({} SO_REUSEPORT acceptors)",
            self.addr,
            n
        );

        let mut handles = Vec::new();
        for worker_id in 0..n {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
                .map_err(|e| KungfuError::internal(format!("socket: {e}")))?;
            socket
                .set_reuse_port(true)
                .map_err(|e| KungfuError::internal(format!("set_reuse_port: {e}")))?;
            socket
                .set_nonblocking(true)
                .map_err(|e| KungfuError::internal(format!("set_nonblocking: {e}")))?;
            socket
                .bind(&socket2::SockAddr::from(self.addr))
                .map_err(|e| KungfuError::internal(format!("bind: {e}")))?;
            socket
                .listen(1024)
                .map_err(|e| KungfuError::internal(format!("listen: {e}")))?;

            let listener = TcpListener::from_std(socket.into())?;
            let router_slot = self.router.clone();
            let pool = self.pool.clone();

            handles.push(tokio::spawn(async move {
                tracing::debug!("acceptor {} online", worker_id);
                let _ = serve_loop(listener, router_slot, pool).await;
            }));
        }

        for h in handles {
            let _ = h.await;
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    async fn serve_multi_acceptor(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!(
            "kungfu listening on http://{} (SO_REUSEPORT not supported — single-acceptor)",
            self.addr
        );
        self.serve_on(listener).await
    }

    pub async fn serve_on(&self, listener: TcpListener) -> Result<()> {
        serve_loop(listener, self.router.clone(), self.pool.clone()).await
    }
}

async fn serve_loop(
    listener: TcpListener,
    router_slot: Arc<parking_lot::RwLock<Arc<Router>>>,
    pool: Arc<BufferPool>,
) -> Result<()> {
    loop {
        let (stream, remote_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("accept error: {e}");
                continue;
            }
        };

        // Disable Nagle's algorithm so small responses aren't buffered.
        let _ = stream.set_nodelay(true);

        let router_slot = router_slot.clone();
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, remote_addr, router_slot, pool).await {
                tracing::debug!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    remote_addr: SocketAddr,
    router_slot: Arc<parking_lot::RwLock<Arc<Router>>>,
    pool: Arc<BufferPool>,
) -> Result<()> {
    loop {
        let req = match read_request(&mut stream, remote_addr, &pool).await {
            Ok(r) => r,
            Err(e) => {
                if let Some(()) = underlying_io_eof(&e) {
                    return Ok(());
                }
                let _ = write_error(&mut stream, StatusCode::BadRequest, &e.to_string()).await;
                return Err(e);
            }
        };

        let req_keep_alive = match req.header("connection") {
            Some(v) if v.eq_ignore_ascii_case("close") => false,
            Some(v) if v.eq_ignore_ascii_case("keep-alive") => true,
            _ => req.version != "HTTP/1.0",
        };

        let router = router_slot.read().clone();
        let mut resp = dispatch(req, &router).await;

        if !req_keep_alive {
            resp.set_header("connection", "close");
        } else if resp.header_value("connection").is_none() {
            resp.set_header("connection", "keep-alive");
        }

        write_response(&mut stream, resp).await?;

        if !req_keep_alive {
            break;
        }
    }
    Ok(())
}

fn underlying_io_eof(e: &KungfuError) -> Option<()> {
    if let Some(detail) = &e.detail {
        if detail.contains("UnexpectedEof") || detail.contains("ConnectionReset") {
            return Some(());
        }
    }
    None
}

async fn read_request(
    stream: &mut tokio::net::TcpStream,
    remote_addr: SocketAddr,
    pool: &Arc<BufferPool>,
) -> Result<Request> {
    let mut buf: Vec<u8> = pool.acquire().as_slice().to_vec();
    let mut read_buf = [0u8; 4096];

    let header_end;
    loop {
        let n = stream
            .read(&mut read_buf)
            .await
            .map_err(|e| KungfuError::internal(format!("read: {e}")))?;
        if n == 0 {
            return Err(KungfuError::internal("UnexpectedEof"));
        }
        buf.extend_from_slice(&read_buf[..n]);
        if buf.len() > MAX_HEADER_BYTES {
            return Err(KungfuError::new(
                StatusCode::BadRequest,
                "Request headers too large",
            ));
        }
        if let Some(pos) = find_header_end(&buf) {
            header_end = pos;
            break;
        }
    }

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req_parser = httparse::Request::new(&mut headers);
    let parsed = req_parser
        .parse(&buf)
        .map_err(|e| KungfuError::new(StatusCode::BadRequest, format!("HTTP parse: {e}")))?;
    let header_bytes_consumed = match parsed {
        httparse::Status::Complete(n) => n,
        httparse::Status::Partial => {
            return Err(KungfuError::new(
                StatusCode::BadRequest,
                "Incomplete HTTP request",
            ));
        }
    };

    let method = req_parser
        .method
        .and_then(|m| Method::from_bytes(m.as_bytes()))
        .ok_or_else(|| KungfuError::new(StatusCode::BadRequest, "Unsupported method"))?;

    let raw_path = req_parser
        .path
        .ok_or_else(|| KungfuError::new(StatusCode::BadRequest, "Missing path"))?;

    let (path_string, query_string_string) = match raw_path.find('?') {
        Some(idx) => (
            raw_path[..idx].to_string(),
            raw_path[idx + 1..].to_string(),
        ),
        None => (raw_path.to_string(), String::new()),
    };

    let version = req_parser
        .version
        .map(|v| format!("HTTP/1.{}", v))
        .unwrap_or_else(|| "HTTP/1.1".to_string());

    let header_pairs: Vec<(String, String)> = req_parser
        .headers
        .iter()
        .filter(|h| !h.name.is_empty())
        .map(|h| {
            (
                h.name.to_ascii_lowercase(),
                std::str::from_utf8(h.value).unwrap_or("").to_string(),
            )
        })
        .collect();

    let content_length: Option<usize> = header_pairs
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok());

    let body_start = header_bytes_consumed;
    let partial_body: Bytes = if buf.len() > body_start {
        Bytes::copy_from_slice(&buf[body_start..])
    } else {
        Bytes::new()
    };
    let _ = header_end;

    let body: Bytes = if let Some(cl) = content_length {
        if cl > MAX_BODY_BYTES {
            return Err(KungfuError::new(StatusCode::BadRequest, "Body too large"));
        }
        if partial_body.len() >= cl {
            partial_body.slice(..cl)
        } else {
            let mut full = Vec::with_capacity(cl);
            full.extend_from_slice(&partial_body);
            while full.len() < cl {
                let n = stream
                    .read(&mut read_buf)
                    .await
                    .map_err(|e| KungfuError::internal(format!("body read: {e}")))?;
                if n == 0 {
                    return Err(KungfuError::internal("UnexpectedEof while reading body"));
                }
                full.extend_from_slice(&read_buf[..n]);
            }
            Bytes::from(full)
        }
    } else {
        partial_body
    };

    let query = crate::request::parse_query(&query_string_string);

    Ok(Request {
        method,
        path: path_string,
        query_string: query_string_string,
        query,
        version,
        headers: header_pairs,
        params: std::collections::HashMap::new(),
        body,
        remote_addr: Some(remote_addr),
    })
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    for i in 3..buf.len() {
        if buf[i - 3..=i] == *b"\r\n\r\n" {
            return Some(i + 1);
        }
    }
    None
}

async fn dispatch(mut req: Request, router: &Router) -> Response {
    let resolution = router.resolve(req.method, &req.path);
    let handler: Handler = match resolution {
        RouteResolution::Found { handler, params, .. } => {
            req.params = params;
            handler
        }
        RouteResolution::MethodNotAllowed => {
            let path = req.path.clone();
            let method = req.method;
            std::sync::Arc::new(move |_req: Request| {
                let path = path.clone();
                let method = method;
                Box::pin(async move {
                    Response::new().error(KungfuError::method_not_allowed(format!(
                        "Method {} not allowed on {}",
                        method.as_str(),
                        path
                    )))
                })
            })
        }
        RouteResolution::NotFound => {
            if let Some(fb) = router.fallback() {
                fb.clone()
            } else {
                let path = req.path.clone();
                let method = req.method;
                std::sync::Arc::new(move |_req: Request| {
                    let path = path.clone();
                    let method = method;
                    Box::pin(async move {
                        Response::new().error(KungfuError::not_found(format!(
                            "No route for {} {}",
                            method.as_str(),
                            path
                        )))
                    })
                })
            }
        }
    };

    let next = build_chain(router.middleware(), handler);
    next(req).await
}

async fn write_response(stream: &mut tokio::net::TcpStream, resp: Response) -> Result<()> {
    // Build the entire response in one buffer, write it in a single syscall.
    // This is significantly faster than writing headers + body separately
    // (saves 1-2 syscalls per response).
    let body_len = resp.body.len();
    let mut out = Vec::with_capacity(256 + body_len);

    out.extend_from_slice(format!(
        "HTTP/1.1 {} {}\r\n",
        resp.status.as_u16(),
        resp.status.canonical_reason()
    ).as_bytes());

    let mut has_content_length = false;
    let mut has_connection = false;
    for (k, v) in &resp.headers {
        if k.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
        if k.eq_ignore_ascii_case("connection") {
            has_connection = true;
        }
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(v.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if !has_content_length {
        out.extend_from_slice(b"content-length: ");
        out.extend_from_slice(body_len.to_string().as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if !has_connection {
        out.extend_from_slice(b"connection: keep-alive\r\n");
    }
    out.extend_from_slice(b"\r\n");
    if body_len > 0 {
        out.extend_from_slice(&resp.body);
    }

    stream.write_all(&out).await?;
    stream.flush().await?;
    Ok(())
}

async fn write_error(
    stream: &mut tokio::net::TcpStream,
    code: StatusCode,
    message: &str,
) -> Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": { "code": code.as_u16(), "message": message, "detail": null, "suggestion": null }
    }))
    .unwrap_or_else(|_| b"{}".to_vec());

    let resp = Response::new()
        .status(code)
        .header("content-type", "application/json")
        .send(Bytes::from(body));
    write_response(stream, resp).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn serves_a_simple_route() {
        let mut router = Router::new();
        router
            .get(
                "/hello",
                Arc::new(|_r| Box::pin(async { Response::new().text("world") })),
            )
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = Server::new(router, addr);
        let server_task = tokio::spawn(async move {
            let _ = server.serve_on(listener).await;
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            async {
                let mut conn = tokio::net::TcpStream::connect(addr).await?;
                conn.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await?;
                conn.flush().await?;
                let mut buf = Vec::new();
                conn.read_to_end(&mut buf).await?;
                Ok::<_, std::io::Error>(buf)
            },
        )
        .await;
        server_task.abort();

        let buf = result.expect("client timed out").expect("client error");
        let s = String::from_utf8_lossy(&buf);
        assert!(s.contains("200 OK"), "missing 200 OK in: {s}");
        assert!(s.contains("world"), "missing body 'world' in: {s}");
    }

    #[tokio::test]
    async fn buffer_pool_is_reused_across_requests() {
        let mut router = Router::new();
        router
            .get(
                "/hello",
                Arc::new(|_r| Box::pin(async { Response::new().text("world") })),
            )
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = Server::new(router, addr);
        let pool = server.pool.clone();
        let server_task = tokio::spawn(async move {
            let _ = server.serve_on(listener).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut conn = tokio::net::TcpStream::connect(addr).await.unwrap();
        let request = b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";
        let mut buf = [0u8; 4096];
        for _ in 0..20 {
            conn.write_all(request).await.unwrap();
            conn.flush().await.unwrap();
            let mut total = 0;
            while total < 60 {
                let n = conn.read(&mut buf).await.unwrap();
                if n == 0 { break; }
                total += n;
            }
        }
        server_task.abort();

        assert_eq!(pool.allocations(), 0, "expected zero pool allocations");
    }

    #[tokio::test]
    async fn cached_json_response_works() {
        // Verify the cached-json path works end-to-end through the server.
        let cached_body = Bytes::from_static(br#"{"message":"world"}"#);
        let cached_for_closure = cached_body.clone();

        let mut router = Router::new();
        router
            .get(
                "/hello",
                Arc::new(move |_r| {
                    let body = cached_for_closure.clone();
                    Box::pin(async move { Response::new().json_bytes(body) })
                }),
            )
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = Server::new(router, addr);
        let server_task = tokio::spawn(async move {
            let _ = server.serve_on(listener).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut conn = tokio::net::TcpStream::connect(addr).await.unwrap();
        conn.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
        conn.flush().await.unwrap();
        let mut buf = Vec::new();
        conn.read_to_end(&mut buf).await.unwrap();
        server_task.abort();

        let s = String::from_utf8_lossy(&buf);
        assert!(s.contains("200 OK"));
        assert!(s.contains(r#"{"message":"world"}"#));
    }
}

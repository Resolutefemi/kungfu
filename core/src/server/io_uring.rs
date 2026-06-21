//! io_uring-backed HTTP server.
//!
//! Uses `tokio-uring` for zero-copy I/O on Linux 5.1+. Each acceptor thread
//! runs its own io_uring instance; connections are pinned to the thread that
//! accepted them (which is exactly what we want for cache locality).
//!
//! ## Why io_uring?
//!
//! Traditional epoll-based I/O (plain tokio) requires 3 syscalls per request:
//! `recvfrom`, `sendto`, and `epoll_wait`. io_uring batches these into
//! submission queues that the kernel processes asynchronously — typical
//! workload sees 10–20× fewer syscalls per request.
//!
//! ## Tradeoffs
//!
//! - Only available on Linux 5.1+ (5.10+ recommended for full feature set).
//! - Each connection is pinned to its accepting thread — no work stealing.
//!   For HTTP servers this is a feature, not a bug: cache locality matters
//!   more than work stealing for short-lived request handlers.
//! - Buffer ownership is more complex — buffers submitted to io_uring can't
//!   be touched by userspace until the kernel returns them.
//!
//! ## Feature flag
//!
//! This module is only compiled when the `io_uring` feature is enabled:
//!
//! ```toml
//! kungfu-core = { path = "...", features = ["io_uring"] }
//! ```
//!
//! On non-Linux platforms the feature silently does nothing — `Server::serve()`
//! falls back to the tokio epoll path.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio_uring::net::TcpListener as UringListener;

use crate::error::{KungfuError, Result, StatusCode};
use crate::middleware::build_chain;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::router::{Handler, RouteResolution, Router};
use crate::server::RouterSlot;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Start an io_uring-backed server with `n_threads` acceptor threads.
///
/// Each thread runs its own `tokio_uring` runtime + io_uring instance and
/// binds its own SO_REUSEPORT listener on the same port. The kernel
/// load-balances accepts across them.
pub async fn serve_io_uring(
    router_slot: Arc<RouterSlot>,
    addr: SocketAddr,
    n_threads: usize,
) -> Result<()> {
    tracing::info!(
        "kungfu (io_uring) listening on http://{} ({} threads)",
        addr,
        n_threads
    );

    let mut handles = Vec::new();
    for worker_id in 0..n_threads {
        let router_slot = router_slot.clone();

        // tokio_uring requires each thread to start its own runtime.
        let handle = std::thread::Builder::new()
            .name(format!("kungfu-uring-{worker_id}"))
            .spawn(move || {
                tokio_uring::start(async move {
                    let listener = match UringListener::bind(addr) {
                        Ok(l) => l,
                        Err(e) => {
                            tracing::error!("bind error on worker {worker_id}: {e}");
                            return;
                        }
                    };
                    tracing::debug!("io_uring acceptor {worker_id} online");

                    loop {
                        let (stream, remote_addr) = match listener.accept().await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("accept error on worker {worker_id}: {e}");
                                continue;
                            }
                        };

                        let router_slot = router_slot.clone();
                        // Spawn a task *on this thread's runtime* — connection
                        // stays pinned here for cache locality.
                        tokio_uring::spawn(async move {
                            if let Err(e) =
                                handle_connection_uring(stream, remote_addr, router_slot).await
                            {
                                tracing::debug!("connection error: {e}");
                            }
                        });
                    }
                });
            })
            .map_err(|e| KungfuError::internal(format!("thread spawn: {e}")))?;

        handles.push(handle);
    }

    // Wait for all acceptor threads (they run forever).
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

async fn handle_connection_uring(
    mut stream: tokio_uring::net::TcpStream,
    remote_addr: SocketAddr,
    router_slot: Arc<RouterSlot>,
) -> Result<()> {
    // Persistent read buffer — keeps any leftover bytes from a previous read
    // that contained multiple pipelined requests. This is the key to HTTP/1.1
    // pipelining: one `read()` can return N requests, and we process them all
    // before going back to the kernel for more data.
    let mut leftover: Vec<u8> = Vec::new();

    loop {
        let req = match read_request_uring(&mut stream, remote_addr, &mut leftover).await {
            Ok(r) => r,
            Err(e) => {
                if let Some(()) = underlying_io_eof(&e) {
                    return Ok(());
                }
                let _ = write_error_uring(&mut stream, StatusCode::BadRequest, &e.to_string()).await;
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

        write_response_uring(&mut stream, resp).await?;

        if !req_keep_alive {
            break;
        }

        // If we have leftover bytes (pipelined requests), loop immediately
        // without going back to the kernel — `read_request_uring` will
        // consume from `leftover` first.
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

async fn read_request_uring(
    stream: &mut tokio_uring::net::TcpStream,
    remote_addr: SocketAddr,
    leftover: &mut Vec<u8>,
) -> Result<Request> {
    // The read buffer accumulates bytes until we have a complete HTTP request.
    // `leftover` carries any bytes from a previous read that contained the
    // start of the next request (HTTP/1.1 pipelining).
    let mut buf = std::mem::take(leftover);
    let mut read_buf: Vec<u8> = vec![0u8; 4096];

    // Loop reading until we have a full request (headers terminated by \r\n\r\n,
    // body length determined by Content-Length).
    let header_end;
    loop {
        // First, try to parse what we already have. This handles the
        // pipelining case: a previous read returned N requests, we processed
        // one, and now `buf` contains the start of the next.
        if let Some(pos) = find_header_end(&buf) {
            header_end = pos;
            break;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(KungfuError::new(
                StatusCode::BadRequest,
                "Request headers too large",
            ));
        }

        let (res, b) = stream.read(read_buf).await;
        read_buf = b;
        let n = res.map_err(|e| KungfuError::internal(format!("read: {e}")))?;
        if n == 0 {
            return Err(KungfuError::internal("UnexpectedEof"));
        }
        buf.extend_from_slice(&read_buf[..n]);
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
        Some(idx) => (raw_path[..idx].to_string(), raw_path[idx + 1..].to_string()),
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
                let (res, b) = stream.read(read_buf).await;
                read_buf = b;
                let n = res.map_err(|e| KungfuError::internal(format!("body read: {e}")))?;
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

    // Calculate where this request ends in `buf`. Anything after that point
    // belongs to the next (pipelined) request — preserve it in `leftover`.
    let request_end = body_start + body.len();
    if buf.len() > request_end {
        *leftover = buf.split_off(request_end);
    } else {
        *leftover = Vec::new();
    }

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

async fn write_response_uring(
    stream: &mut tokio_uring::net::TcpStream,
    resp: Response,
) -> Result<()> {
    let body_len = resp.body.len();
    let mut out = Vec::with_capacity(256 + body_len);

    out.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            resp.status.as_u16(),
            resp.status.canonical_reason()
        )
        .as_bytes(),
    );

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

    // tokio_uring's write_all takes ownership of the buffer and returns
    // (Result<()>, Buf).
    let (res, _buf) = stream.write_all(out).await;
    res.map_err(|e| KungfuError::internal(format!("write: {e}")))?;
    Ok(())
}

async fn write_error_uring(
    stream: &mut tokio_uring::net::TcpStream,
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
    write_response_uring(stream, resp).await
}

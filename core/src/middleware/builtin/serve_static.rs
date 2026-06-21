//! Static file serving middleware.
//!
//! Serves files from a directory on disk. Falls through to `next` if the
//! requested path doesn't match a file.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::middleware_builtin::serve_static;
//!
//! Kungfu::new()
//!     .use_middleware(serve_static("./public"))
//!     .handle_get("/api/health", |_req, res| res.text("ok"))
//!     .run("0.0.0.0:3000")
//! ```
//!
//! A request to `GET /style.css` will serve `./public/style.css` if it exists.
//! A request to `GET /api/health` falls through to the route handler.

use std::path::PathBuf;
use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use crate::response::Response;

/// Create a static-file-serving middleware rooted at `root`.
pub fn serve_static(root: impl Into<PathBuf>) -> Middleware {
    let root = Arc::new(root.into());
    Arc::new(move |req: Request, next: Next| {
        let root = root.clone();
        Box::pin(async move {
            // Only intercept GET and HEAD requests.
            if req.method != crate::Method::Get && req.method != crate::Method::Head {
                return next(req).await;
            }

            // Sanitize the path — reject any `..` to prevent path traversal.
            let path = &req.path;
            if path.contains("..") {
                return Response::new().error(crate::KungfuError::bad_request(
                    "Path traversal not allowed",
                ));
            }

            let rel_path = path.trim_start_matches('/');
            let full_path = root.join(rel_path);

            // If it's a directory, try to serve index.html inside it.
            let target = if full_path.is_dir() {
                full_path.join("index.html")
            } else {
                full_path.clone()
            };

            if target.is_file() {
                match tokio::fs::read(&target).await {
                    Ok(bytes) => {
                        let mut resp = Response::new().send(bytes);
                        if let Some(ct) = guess_content_type(&target) {
                            resp.set_header("content-type", ct);
                        }
                        // Set cache headers for static assets.
                        resp.set_header("cache-control", "public, max-age=3600");
                        resp
                    }
                    Err(e) => {
                        tracing::warn!("static file read error: {e}");
                        next(req).await
                    }
                }
            } else {
                // No file at this path — fall through to the router.
                next(req).await
            }
        })
    })
}

/// Map a file extension to a MIME type. Covers the common web types; falls
/// back to `application/octet-stream` for unknown extensions.
fn guess_content_type(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let ct = match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        _ => return None,
    };
    Some(ct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_common_content_types() {
        assert_eq!(
            guess_content_type(std::path::Path::new("style.css")),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(
            guess_content_type(std::path::Path::new("app.js")),
            Some("application/javascript; charset=utf-8")
        );
        assert_eq!(
            guess_content_type(std::path::Path::new("logo.png")),
            Some("image/png")
        );
        assert_eq!(
            guess_content_type(std::path::Path::new("data.json")),
            Some("application/json; charset=utf-8")
        );
        assert_eq!(guess_content_type(std::path::Path::new("unknown.xyz")), None);
    }
}

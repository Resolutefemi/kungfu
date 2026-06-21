//! Unified error model for the Kungfu core.
//!
//! Every error carries the four-tuple the framework promises in its public contract:
//! `code`, `message`, `detail`, `suggestion`. The same shape is serialised to JSON
//! when an error reaches the response boundary, so client code in *any* language
//! sees the same structure.

use std::fmt;
use thiserror::Error;

/// HTTP status code wrapper. We use our own enum so the C ABI can mirror it later
/// without depending on a third-party crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    NoContent = 204,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    Conflict = 409,
    UnprocessableEntity = 422,
    TooManyRequests = 429,
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
}

impl StatusCode {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn canonical_reason(self) -> &'static str {
        match self {
            StatusCode::Ok => "OK",
            StatusCode::Created => "Created",
            StatusCode::NoContent => "No Content",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::Conflict => "Conflict",
            StatusCode::UnprocessableEntity => "Unprocessable Entity",
            StatusCode::TooManyRequests => "Too Many Requests",
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::NotImplemented => "Not Implemented",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
        }
    }
}

impl From<u16> for StatusCode {
    fn from(code: u16) -> Self {
        match code {
            200 => Self::Ok,
            201 => Self::Created,
            204 => Self::NoContent,
            400 => Self::BadRequest,
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            405 => Self::MethodNotAllowed,
            409 => Self::Conflict,
            422 => Self::UnprocessableEntity,
            429 => Self::TooManyRequests,
            500 => Self::InternalServerError,
            501 => Self::NotImplemented,
            502 => Self::BadGateway,
            503 => Self::ServiceUnavailable,
            _ => Self::InternalServerError,
        }
    }
}

/// The Kungfu error contract. Every error that crosses a handler boundary
/// must be representable as this shape.
#[derive(Debug, Clone)]
pub struct KungfuError {
    pub code: StatusCode,
    pub message: String,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
}

impl KungfuError {
    pub fn new(code: StatusCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            suggestion: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Serialise to the on-the-wire JSON shape that all language bindings agree on.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "code": self.code.as_u16(),
                "message": self.message,
                "detail": self.detail,
                "suggestion": self.suggestion,
            }
        })
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BadRequest, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NotFound, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::InternalServerError, msg)
    }

    pub fn method_not_allowed(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::MethodNotAllowed, msg)
    }

    pub fn too_many_requests(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::TooManyRequests, msg)
    }
}

impl fmt::Display for KungfuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_u16(), self.message)?;
        if let Some(d) = &self.detail {
            write!(f, " ({})", d)?;
        }
        Ok(())
    }
}

impl std::error::Error for KungfuError {}

impl From<std::io::Error> for KungfuError {
    fn from(e: std::io::Error) -> Self {
        Self::new(StatusCode::InternalServerError, e.to_string())
    }
}

impl From<serde_json::Error> for KungfuError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(StatusCode::BadRequest, "Invalid JSON body")
            .with_detail(e.to_string())
            .with_suggestion("Ensure the request body is valid JSON and the Content-Type is application/json.")
    }
}

#[cfg(feature = "simd")]
impl From<simd_json::Error> for KungfuError {
    fn from(e: simd_json::Error) -> Self {
        Self::new(StatusCode::BadRequest, "Invalid JSON body")
            .with_detail(e.to_string())
            .with_suggestion("Ensure the request body is valid JSON and the Content-Type is application/json.")
    }
}

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP parse error: {0}")]
    HttpParse(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Router error: {0}")]
    Router(String),

    #[error("Kungfu error: {0}")]
    Kungfu(#[from] KungfuError),
}

pub type Result<T> = std::result::Result<T, KungfuError>;

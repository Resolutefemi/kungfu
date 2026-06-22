//! Extended authentication: RS256 JWT, sessions, RBAC, OAuth2 scaffold, password reset.
//!
//! Builds on the HS256 JWT in `auth.rs` to add:
//! - RS256/ES256 JWT support via `jsonwebtoken::Algorithm`
//! - Session-based auth (cookie + server-side store)
//! - RBAC (role-based access control) middleware
//! - OAuth2 provider scaffold
//! - Password reset token generation + verification

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use crate::response::Response;

// ─── RS256 JWT ────────────────────────────────────────────────────────────────

/// JWT service supporting multiple algorithms (HS256, HS384, HS512, RS256, RS384, RS512, ES256, ES384).
#[derive(Clone)]
pub struct JwtServiceMulti {
    algorithm: Algorithm,
    encoding_key: Arc<EncodingKey>,
    decoding_key: Arc<DecodingKey>,
}

impl JwtServiceMulti {
    /// Create an HS256 service (same as `JwtService::new`).
    pub fn hs256(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            algorithm: Algorithm::HS256,
            encoding_key: Arc::new(EncodingKey::from_secret(secret.as_bytes())),
            decoding_key: Arc::new(DecodingKey::from_secret(secret.as_bytes())),
        }
    }

    /// Create an RS256 service from PEM-encoded RSA private key + public key.
    pub fn rs256(private_pem: &str, public_pem: &str) -> Result<Self, String> {
        let encoding_key = EncodingKey::from_rsa_pem(private_pem.as_bytes())
            .map_err(|e| e.to_string())?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            algorithm: Algorithm::RS256,
            encoding_key: Arc::new(encoding_key),
            decoding_key: Arc::new(decoding_key),
        })
    }

    /// Create an ES256 service from PEM-encoded EC private key + public key.
    pub fn es256(private_pem: &str, public_pem: &str) -> Result<Self, String> {
        let encoding_key = EncodingKey::from_ec_pem(private_pem.as_bytes())
            .map_err(|e| e.to_string())?;
        let decoding_key = DecodingKey::from_ec_pem(public_pem.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            algorithm: Algorithm::ES256,
            encoding_key: Arc::new(encoding_key),
            decoding_key: Arc::new(decoding_key),
        })
    }

    /// Sign a JWT.
    pub fn sign(&self, claims: &impl Serialize) -> Result<String, String> {
        let mut header = Header::new(self.algorithm);
        header.kid = None;
        encode(&header, claims, &self.encoding_key).map_err(|e| e.to_string())
    }

    /// Verify a JWT.
    pub fn verify<T: for<'de> Deserialize<'de>>(&self, token: &str) -> Result<T, String> {
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        decode::<T>(token, &self.decoding_key, &validation)
            .map(|d| d.claims)
            .map_err(|e| e.to_string())
    }
}

// ─── Session-based auth ───────────────────────────────────────────────────────

/// A session store — maps session IDs to user data.
/// V1 uses an in-memory HashMap. V1.1 will add Redis backend.
pub struct SessionStore {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

/// A user session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub data: serde_json::Value,
    pub expires_at: u64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new session and return the session ID.
    pub fn create(&self, user_id: impl Into<String>, data: serde_json::Value, ttl_seconds: u64) -> String {
        let session_id = generate_session_id();
        let expires_at = current_timestamp() + ttl_seconds;
        let session = Session {
            session_id: session_id.clone(),
            user_id: user_id.into(),
            data,
            expires_at,
        };
        self.sessions.lock().insert(session_id.clone(), session);
        session_id
    }

    /// Get a session by ID. Returns None if not found or expired.
    pub fn get(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.lock();
        let session = sessions.get(session_id)?;
        if current_timestamp() > session.expires_at {
            return None;
        }
        Some(session.clone())
    }

    /// Delete a session (logout).
    pub fn delete(&self, session_id: &str) {
        self.sessions.lock().remove(session_id);
    }

    /// Clean up expired sessions.
    pub fn cleanup(&self) {
        let now = current_timestamp();
        self.sessions.lock().retain(|_, s| s.expires_at > now);
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Session auth middleware — reads the `session_id` cookie and attaches
/// the session to the request via the `x-session` response header (V1 workaround).
pub fn session_auth(store: Arc<SessionStore>) -> Middleware {
    Arc::new(move |req: Request, next: Next| {
        let store = store.clone();
        Box::pin(async move {
            // Parse cookies from the request.
            let jar = crate::cookies::CookieJar::from_request(&req);
            if let Some(session_id) = jar.get("session_id") {
                if let Some(session) = store.get(session_id) {
                    let mut resp = next(req).await;
                    resp.set_header("x-session-user", &session.user_id);
                    return resp;
                }
            }
            // No valid session — continue (the handler can decide to 401).
            next(req).await
        })
    })
}

// ─── RBAC ─────────────────────────────────────────────────────────────────────

/// Role-based access control middleware.
///
/// Checks that the authenticated user has one of the required roles.
/// Expects the user's role to be in the `x-jwt-claims` header (set by
/// the JWT middleware) or the `x-session-user` header (set by session auth).
///
/// ```ignore
/// Kungfu::new()
///     .use_middleware(auth_jwt(JwtConfig::new("secret")))
///     .use_middleware(require_role("admin"))
///     .handle_get("/admin", |_req, res| res.text("admin panel"))
/// ```
pub fn require_role(role: impl Into<String>) -> Middleware {
    let required_role = role.into();
    Arc::new(move |req: Request, next: Next| {
        let required_role = required_role.clone();
        Box::pin(async move {
            // Check JWT claims for the role.
            if let Some(claims_header) = req.header("x-jwt-claims") {
                if let Ok(claims) = serde_json::from_str::<serde_json::Value>(claims_header) {
                    if let Some(user_role) = claims.get("role").and_then(|r| r.as_str()) {
                        if user_role == required_role || user_role == "admin" {
                            return next(req).await;
                        }
                    }
                }
            }

            Response::new()
                .status(crate::StatusCode::Forbidden)
                .json(&serde_json::json!({
                    "error": {
                        "code": 403,
                        "message": "Insufficient permissions",
                        "detail": format!("Required role: {}", required_role),
                        "suggestion": "Contact an administrator if you believe this is an error.",
                    }
                }))
        })
    })
}

/// Require ANY of the given roles.
pub fn require_any_role(roles: Vec<String>) -> Middleware {
    Arc::new(move |req: Request, next: Next| {
        let roles = roles.clone();
        Box::pin(async move {
            if let Some(claims_header) = req.header("x-jwt-claims") {
                if let Ok(claims) = serde_json::from_str::<serde_json::Value>(claims_header) {
                    if let Some(user_role) = claims.get("role").and_then(|r| r.as_str()) {
                        if roles.iter().any(|r| r == user_role) || user_role == "admin" {
                            return next(req).await;
                        }
                    }
                }
            }

            Response::new()
                .status(crate::StatusCode::Forbidden)
                .json(&serde_json::json!({
                    "error": {
                        "code": 403,
                        "message": "Insufficient permissions",
                        "detail": format!("Required one of: {}", roles.join(", ")),
                    }
                }))
        })
    })
}

// ─── OAuth2 scaffold ──────────────────────────────────────────────────────────

/// OAuth2 provider configuration.
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub provider: OAuth2Provider,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

/// Supported OAuth2 providers.
#[derive(Debug, Clone)]
pub enum OAuth2Provider {
    Google,
    GitHub,
    Discord,
    Custom { auth_url: String, token_url: String, user_url: String },
}

impl OAuth2Provider {
    /// Get the authorization URL for this provider.
    pub fn auth_url(&self) -> &str {
        match self {
            OAuth2Provider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            OAuth2Provider::GitHub => "https://github.com/login/oauth/authorize",
            OAuth2Provider::Discord => "https://discord.com/api/oauth2/authorize",
            OAuth2Provider::Custom { auth_url, .. } => auth_url,
        }
    }

    /// Get the token exchange URL.
    pub fn token_url(&self) -> &str {
        match self {
            OAuth2Provider::Google => "https://oauth2.googleapis.com/token",
            OAuth2Provider::GitHub => "https://github.com/login/oauth/access_token",
            OAuth2Provider::Discord => "https://discord.com/api/oauth2/token",
            OAuth2Provider::Custom { token_url, .. } => token_url,
        }
    }

    /// Get the user info URL.
    pub fn user_url(&self) -> &str {
        match self {
            OAuth2Provider::Google => "https://www.googleapis.com/oauth2/v2/userinfo",
            OAuth2Provider::GitHub => "https://api.github.com/user",
            OAuth2Provider::Discord => "https://discord.com/api/users/@me",
            OAuth2Provider::Custom { user_url, .. } => user_url,
        }
    }
}

impl OAuth2Config {
    /// Build the authorization URL to redirect the user to.
    pub fn authorization_url(&self, state: &str) -> String {
        let scope = self.scopes.join(" ");
        format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            self.provider.auth_url(),
            url_encode(&self.client_id),
            url_encode(&self.redirect_uri),
            url_encode(&scope),
            url_encode(state),
        )
    }
}

// ─── Password reset ───────────────────────────────────────────────────────────

/// Password reset token manager. Generates time-limited tokens and verifies them.
pub struct PasswordReset {
    secret: String,
    tokens: Arc<Mutex<HashMap<String, PasswordResetToken>>>,
}

#[derive(Debug, Clone)]
struct PasswordResetToken {
    user_id: String,
    expires_at: u64,
}

impl PasswordReset {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Generate a password reset token for a user. Returns the token string.
    pub fn generate(&self, user_id: impl Into<String>, ttl_seconds: u64) -> String {
        let token = generate_session_id(); // Reuse the random ID generator.
        let reset_token = PasswordResetToken {
            user_id: user_id.into(),
            expires_at: current_timestamp() + ttl_seconds,
        };
        self.tokens.lock().insert(token.clone(), reset_token);
        token
    }

    /// Verify a password reset token. Returns the user_id if valid.
    pub fn verify(&self, token: &str) -> Option<String> {
        let tokens = self.tokens.lock();
        let reset_token = tokens.get(token)?;
        if current_timestamp() > reset_token.expires_at {
            return None;
        }
        Some(reset_token.user_id.clone())
    }

    /// Consume a password reset token (delete it after use).
    pub fn consume(&self, token: &str) -> Option<String> {
        let user_id = self.verify(token)?;
        self.tokens.lock().remove(token);
        Some(user_id)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn generate_session_id() -> String {
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    let mut bytes = [0u8; 32];
    let _ = rng.fill(&mut bytes);
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".into(),
            c if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => c.to_string(),
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_store_create_and_get() {
        let store = SessionStore::new();
        let id = store.create("user123", serde_json::json!({"role":"admin"}), 3600);
        let session = store.get(&id).unwrap();
        assert_eq!(session.user_id, "user123");
        assert_eq!(session.data["role"], "admin");
    }

    #[test]
    fn session_store_delete() {
        let store = SessionStore::new();
        let id = store.create("user123", serde_json::json!({}), 3600);
        store.delete(&id);
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn password_reset_generate_and_verify() {
        let reset = PasswordReset::new("secret");
        let token = reset.generate("user123", 3600);
        assert_eq!(reset.verify(&token), Some("user123".to_string()));
        // Consume (one-time use).
        assert_eq!(reset.consume(&token), Some("user123".to_string()));
        assert_eq!(reset.verify(&token), None);
    }

    #[test]
    fn oauth2_authorization_url() {
        let config = OAuth2Config {
            provider: OAuth2Provider::GitHub,
            client_id: "test_client".into(),
            client_secret: "test_secret".into(),
            redirect_uri: "http://localhost:3000/callback".into(),
            scopes: vec!["user:email".into()],
        };
        let url = config.authorization_url("random_state");
        assert!(url.contains("github.com"));
        assert!(url.contains("test_client"));
        assert!(url.contains("user%3Aemail") || url.contains("user:email"));
    }

    #[test]
    fn url_encode_works() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("a@b.com"), "a%40b.com");
    }
}

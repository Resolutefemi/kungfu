# Cookies & Sessions

> ⏱️ 5 minutes

Kungfu provides built-in cookie support via the `Cookie`, `CookieJar`, and
`SameSite` types. Use cookies for session IDs, theme preferences, "remember
me" tokens, and other small pieces of state stored on the client.

## Reading cookies

```rust
use kungfu::cookies::CookieJar;

Kungfu::new()
    .handle_get("/dashboard", |req, res| {
        let jar = CookieJar::from_request(&req);
        match jar.get("session_id") {
            Some(id) => res.text(format!("Welcome back, session={id}")),
            None => res.status(401).text("Not logged in"),
        }
    })
```

`CookieJar::from_request` parses the `Cookie` header automatically.

## Setting cookies

```rust
use kungfu::cookies::{Cookie, CookieJar};

Kungfu::new()
    .handle_post("/login", |_req, res| {
        let mut jar = CookieJar::new();
        jar.set(
            Cookie::new("session_id", "abc123")
                .path("/")
                .http_only()
                .secure()
                .max_age(3600)  // 1 hour
                .same_site_strict(),
        );
        // Apply the jar to the response.
        let mut response = res.text("logged in");
        jar.apply_to_response(&mut response);
        response
    })
```

## Cookie attributes

| Attribute | Method | Example |
|---|---|---|
| Path | `.path("/")` | Cookie is sent for requests under `/` |
| Domain | `.domain("example.com")` | Cookie is sent for this domain |
| Max-Age | `.max_age(3600)` | Cookie expires after 3600 seconds |
| HttpOnly | `.http_only()` | JavaScript can't read the cookie (XSS protection) |
| Secure | `.secure()` | Cookie only sent over HTTPS |
| SameSite=Strict | `.same_site_strict()` | Cookie never sent on cross-site requests |
| SameSite=Lax | `.same_site_lax()` | Cookie sent on top-level cross-site navigations (default) |
| SameSite=None | `.same_site_none()` | Cookie always sent (requires `Secure`) |

## Sessions

Kungfu doesn't ship a session store in V1, but you can build one on top of
cookies. A common pattern is to store a session ID in a cookie and look up
the session data in a database:

```rust
use kungfu::cookies::CookieJar;
use kungfu_orm::{Db, Model};
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize)]
#[table(name = "sessions")]
struct Session {
    #[field(primary)]
    id: String,
    user_id: i64,
    expires_at: i64,
}

async fn get_session(req: &kungfu::Request, db: &Db) -> Option<Session> {
    let jar = CookieJar::from_request(req);
    let session_id = jar.get("session_id")?;
    Session::find()
        .where_eq("id", session_id)
        .one(db)
        .await
        .ok()
}
```

## Security best practices

- Always use `HttpOnly` for session cookies — prevents XSS from stealing them
- Always use `Secure` in production — cookies only sent over HTTPS
- Use `SameSite=Strict` for session cookies — prevents CSRF
- Use `Max-Age` (not `Expires`) — easier to reason about
- Don't store sensitive data in cookies — they're visible to the client

## Next steps

Continue to [Static Files](./06-static-files.md) to learn how to serve
files from disk + use the built-in CSS engine.

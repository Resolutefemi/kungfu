# Request & Response

> ⏱️ 6 minutes

## The Request object

Every handler receives a `Request`:

```rust
pub struct Request {
    pub method: Method,       // GET, POST, ...
    pub path: String,         // "/users/42"
    pub query_string: String, // "active=true"
    pub query: HashMap<String, String>,
    pub headers: Vec<(String, String)>,  // lowercased keys
    pub params: HashMap<String, String>, // route params (e.g. :id)
    pub body: bytes::Bytes,
    pub remote_addr: Option<SocketAddr>,
    pub version: String,      // "HTTP/1.1"
}
```

### Reading headers

```rust
let content_type = req.header("content-type");
let auth = req.header("authorization");
```

`req.header(key)` is case-insensitive.

### Reading the body

Parse the body as JSON into a typed struct:

```rust
#[derive(serde::Deserialize)]
struct CreateUser {
    email: String,
    password: String,
}

Kungfu::new()
    .json_post("/users", |body: CreateUser| {
        // body is already parsed — no boilerplate.
        User { id: 1, email: body.email }
    })
```

Or parse it manually:

```rust
.handle_post("/users", |req, res| {
    let body: CreateUser = match req.json() {
        Ok(b) => b,
        Err(e) => return res.error(e),
    };
    res.json(&User { id: 1, email: body.email })
})
```

### Reading form data

```rust
.handle_post("/login", |req, res| {
    let form = match req.form() {
        Ok(f) => f,
        Err(e) => return res.error(e),
    };
    let email = form.get("email").map(|s| s.as_str()).unwrap_or("");
    let password = form.get("password").map(|s| s.as_str()).unwrap_or("");
    // ... check credentials ...
    res.text("logged in")
})
```

### Reading query params

```rust
.handle_get("/search", |req, res| {
    let q = req.query("q").unwrap_or("");
    let page: u32 = req.query("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    res.json(&serde_json::json!({"q": q, "page": page}))
})
```

### Reading route params

```rust
.handle_get("/users/:id", |req, res| {
    let id = req.param("id").unwrap_or("unknown");
    res.text(format!("User {}", id))
})
```

## The Response object

There are two ways to build a response: the chainable `ResponseBuilder`
(simplest) or the `Response` type directly (more control).

### `ResponseBuilder` (recommended)

```rust
Kungfu::new()
    .handle_get("/hello", |_req, res| res.text("world"))
    .handle_get("/html", |_req, res| res.html("<h1>hi</h1>"))
    .handle_get("/json", |_req, res| res.json(&serde_json::json!({"ok":true})))
    .handle_get("/teapot", |_req, res| {
        res.status(418).header("x-tea", "earl grey").text("I'm a teapot")
    })
```

The chain ends with a terminal method (`text`, `html`, `json`, `send`)
that returns the final `Response`.

### `Response` directly

```rust
.handle_get("/custom", |_req, _res| {
    kungfu::Response::new()
        .status(kungfu::StatusCode::Created)
        .header("x-custom", "yes")
        .json(&serde_json::json!({"created":true}))
})
```

### Cached responses (hot path)

For endpoints that return the same JSON every time, pre-serialise once
at startup and reuse the bytes:

```rust
use bytes::Bytes;

let cached_body: Bytes = Bytes::from(
    serde_json::to_vec(&serde_json::json!({"message":"world"})).unwrap()
);
let cached_for_handler = cached_body.clone();

Kungfu::new()
    .handle_get("/hello", move |_req, _res| {
        // Bytes::clone() is an atomic Arc increment — ~5ns.
        kungfu::Response::new().json_bytes(cached_for_handler.clone())
    })
```

This is the fastest possible response path in Kungfu.

## Errors

Use `Response::error()` to return an error with the framework's unified
shape (`code`, `message`, `detail`, `suggestion`):

```rust
.handle_get("/not-found", |_req, _res| {
    kungfu::Response::new().error(
        kungfu::KungfuError::not_found("User not found")
            .with_detail("The user has been deleted")
            .with_suggestion("Try GET /users to list all users"),
    )
})
```

```bash
$ curl -i http://localhost:3000/not-found
HTTP/1.1 404 Not Found
content-type: application/json
...

{"error":{"code":404,"message":"User not found","detail":"...","suggestion":"..."}}
```

The 404/405/429 responses are pre-serialised at startup, so they cost
essentially nothing — no JSON encoding on the hot path.

## Next steps

Continue to [Cookies & Sessions](./05-cookies-sessions.md) to learn
about cookie-based authentication.

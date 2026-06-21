# Routing

> ⏱️ 10 minutes

Kungfu's router is a trie — path lookup is O(path depth), which means
routing stays fast even with thousands of registered routes.

## Static paths

The simplest route is a static path:

```rust
use kungfu::prelude::*;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(
        Kungfu::new()
            .handle_get("/", |_req, res| res.text("home"))
            .handle_get("/about", |_req, res| res.text("about"))
            .handle_get("/contact", |_req, res| res.text("contact"))
            .run("0.0.0.0:3000"),
    ).unwrap();
}
```

## Path parameters

Use `:name` to capture a single segment:

```rust
Kungfu::new()
    .handle_get("/users/:id", |req, res| {
        let id = req.param("id").unwrap_or("unknown");
        res.text(format!("User {}", id))
    })
```

```bash
$ curl http://localhost:3000/users/42
User 42
```

Multiple params work too:

```rust
.handle_get("/posts/:post_id/comments/:comment_id", |req, res| {
    let post_id = req.param("post_id").unwrap_or("");
    let comment_id = req.param("comment_id").unwrap_or("");
    res.text(format!("Post {} comment {}", post_id, comment_id))
})
```

## Wildcards

Use `*name` to capture the rest of the path (including `/`):

```rust
.handle_get("/assets/*path", |req, res| {
    let path = req.param("path").unwrap_or("");
    res.text(format!("Serving {}", path))
})
```

```bash
$ curl http://localhost:3000/assets/css/app.css
Serving css/app.css
```

## Query strings

Query parameters are parsed automatically and available via `req.query("key")`:

```rust
.handle_get("/search", |req, res| {
    let q = req.query("q").unwrap_or("");
    let limit: usize = req.query("limit").and_then(|s| s.parse().ok()).unwrap_or(10);
    res.json(&serde_json::json!({
        "query": q,
        "limit": limit,
        "results": [],
    }))
})
```

```bash
$ curl 'http://localhost:3000/search?q=rust&limit=5'
{"query":"rust","limit":5,"results":[]}
```

## HTTP methods

Kungfu has separate methods for each HTTP verb:

| Method | Builder fn |
|---|---|
| GET | `.handle_get(path, h)` or `.json_get(path, h)` |
| POST | `.handle_post(path, h)` or `.json_post(path, h)` |
| PUT | `.handle_put(path, h)` |
| DELETE | `.handle_delete(path, h)` |
| PATCH | `.handle_patch(path, h)` |

If a path is registered for GET but the client sends POST, the framework
returns 405 Method Not Allowed automatically.

## The `json_get` / `json_post` shortcut

For endpoints that just return JSON, use `json_get` / `json_post` to skip
the `Response::new().json(...)` boilerplate:

```rust
Kungfu::new()
    // Equivalent to .handle_get("/health", |_req, res| res.json(&...))
    .json_get("/health", || serde_json::json!({"status":"ok"}))
    // Parse the body into a typed struct automatically.
    .json_post("/users", |body: CreateUser| User {
        id: 1,
        email: body.email,
    })
```

## Route metadata (for OpenAPI)

If you want to add summaries, tags, or schemas to the OpenAPI spec,
use the `add_with_meta` API:

```rust
use kungfu::{Kungfu, Method, RouteMeta, Handler};

let handler: Handler = std::sync::Arc::new(|_req| {
    Box::pin(async { kungfu::Response::new().text("hi") })
});

Kungfu::new()
    .add_with_meta(
        RouteMeta {
            path: "/hello".into(),
            method: Method::Get,
            summary: Some("Say hello".into()),
            tags: vec!["greeting".into()],
            ..Default::default()
        },
        handler,
    )
```

The summary and tags will appear in the Swagger UI at `/docs`.

## Next steps

Continue to [Middleware](./03-middleware.md) to learn how to add
custom logic that runs before/after every request.

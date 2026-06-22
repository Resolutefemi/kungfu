# Kungfu.js API Reference

Auto-generated from rustdoc. To generate locally:

```bash
cargo doc --workspace --no-deps --open
```

## Core crate (`kungfu-core`)

### Server
- `Server::new(router, addr)` — create a server
- `Server::with_acceptor_threads(n)` — set SO_REUSEPORT acceptor count
- `Server::serve()` — start serving (blocks)

### Router
- `Router::new()` — create a router
- `router.get/post/put/delete/patch(path, handler)` — register routes
- `router.ws(path, handler)` — register WebSocket route
- `router.use_middleware(mw)` — add middleware
- `router.add_with_meta(meta, handler)` — register with OpenAPI metadata

### Request
- `req.param("id")` — route parameter
- `req.query("q")` — query parameter
- `req.header("content-type")` — request header
- `req.json::<T>()` — parse JSON body
- `req.form()` — parse form body
- `req.multipart()` — parse multipart form
- `req.json_value()` — parse JSON as `serde_json::Value`

### Response
- `Response::new().json(&value)` — JSON response
- `Response::new().text("hello")` — text response
- `Response::new().html("<h1>hi</h1>")` — HTML response
- `Response::new().json_bytes(cached_bytes)` — cached JSON (fastest path)
- `Response::new().error(KungfuError)` — error response
- `Response::html_escape(s)` — XSS-safe HTML escaping

### Middleware (built-in)
- `security_headers()` — HSTS, CSP, X-Frame-Options, etc.
- `cors()` / `cors_with(config)` — CORS with preflight
- `rate_limiter()` / `rate_limiter_with(config)` — leaky-bucket per IP+path
- `logger()` — structured request logging
- `serve_static("./public")` — static file serving
- `etag()` — ETag + conditional GET
- `gzip()` — gzip compression
- `validate_json(path, method, schema)` — JSON Schema validation
- `auth_jwt(config)` — JWT authentication
- `session_auth(store)` — session-based authentication
- `require_role("admin")` — RBAC role check
- `require_any_role(vec!["admin","editor"])` — any of roles

### Auth
- `JwtService::new(secret)` — HS256 JWT sign/verify
- `JwtServiceMulti::rs256(priv, pub)` — RS256 JWT
- `JwtServiceMulti::es256(priv, pub)` — ES256 JWT
- `SessionStore::new()` — in-memory session store
- `PasswordReset::new(secret)` — password reset tokens
- `OAuth2Config` — OAuth2 provider config (Google/GitHub/Discord)

### ORM (`kungfu-orm`)
- `Db::mock()` — in-memory database
- `Db::connect(&config)` — connect to Postgres/MySQL/SQLite
- `Model::insert(&self, &db)` — insert a row
- `Model::all(&db)` — fetch all rows
- `Model::find().where_eq("col", val).one(&db)` — query with WHERE
- `Model::find_by_pk(pk, &db)` — find by primary key
- `Model::update_by_pk(&db, pk, sets)` — update by PK
- `Model::delete_by_pk(pk, &db)` — delete by PK
- `Model::count(&db)` — count rows
- `Db::transaction(|tx| async { ... })` — database transaction
- `Db::query_raw(sql, params)` — raw SQL (for JOINs)
- `Db::aggregate(sql, params)` — COUNT/SUM/AVG
- `hash_password(plain)` / `verify_password(plain, hash)` — Argon2id

### CSS Engine (`kungfu-css`)
- `compile_classes("flex p-4 text-red-500")` — compile class string to CSS
- `compile_directory("./src")` — scan source files + emit CSS bundle

### Frontend (`kungfu-frontend`)
- `parse_kungfu_file(content, path)` — parse .kungfu file
- `render_page(file, ctx, template, data)` — render SSR page
- `render_kungfu_file(path, req_json, ctx)` — render via Node.js subprocess
- `register_pages(router, "src/pages")` — auto-register .kungfu routes
- `generate_typescript(routes)` — generate routes.d.ts
- `DevMode::new(paths, types_path)` — dev mode controller

### TLS
- `TlsConfig::from_files("cert.pem", "key.pem")` — HTTPS configuration
- `config.to_rustls_config()` — convert to rustls ServerConfig

### Performance
- `Server::with_acceptor_threads(n)` — SO_REUSEPORT multi-acceptor
- Build with `--features io_uring` for zero-copy I/O
- Build with `--features simd` for SIMD JSON
- `perf::enable_connection_pinning()` — pin connections to threads

### Other
- `WebSocket` — RFC 6455 frame parser + encoder
- `Queue::new(n)` — background job queue
- `Plugin` trait — plugin system
- `Cookie / CookieJar` — cookie management
- `Headers` — SmallVec-backed header storage

## CLI (`kungfu-cli`)
- `kungfu new <name>` — scaffold a new project
- `kungfu demo` — run the demo server
- `kungfu start [--watch]` — run with optional hot reload
- `kungfu build` — release build
- `kungfu migrate` — generate SQL migrations
- `kungfu generate admin` — generate admin dashboard
- `kungfu deploy [port]` — generate Dockerfile + docker-compose + systemd

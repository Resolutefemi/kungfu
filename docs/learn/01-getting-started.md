# Getting Started with Kungfu.js

> ⏱️ 5 minutes

This tutorial walks you through installing Kungfu and writing your first
app. By the end, you'll have a running HTTP server that responds to
`GET /hello` with JSON.

## Prerequisites

Kungfu requires:

- **Rust 1.96+** (for the Rust API + core)
- **Node.js 18+** (for the JS/TS binding + frontend)
- **Python 3.8+** (for the Python binding, optional)
- **Go 1.21+** (for the Go binding, optional)

Install Rust from https://rustup.rs if you don't have it.

## Install

Clone the repo and build the demo:

```bash
git clone https://github.com/Resolutefemi/kungfu.git
cd kungfu
cargo build --workspace --release
```

For maximum performance on Linux 5.1+ with AVX2:

```bash
cargo build --workspace --release --features "kungfu-core/io_uring kungfu-core/simd"
```

## Your first Kungfu app

Create `hello.rs`:

```rust
use kungfu::prelude::*;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(
        Kungfu::new()
            .handle_get("/hello", |_req, res| res.text("world"))
            .run("0.0.0.0:3000"),
    )
    .unwrap();
}
```

Run it:

```bash
cargo run -p kungfu --example simple
```

Test it:

```bash
$ curl http://localhost:3000/hello
world
```

🎉 You have a running Kungfu server!

## What's happening here?

1. `Kungfu::new()` constructs a new application builder.
2. `.handle_get("/hello", closure)` registers a GET handler at `/hello`.
   The closure takes a `Request` and a `ResponseBuilder` and returns a
   `Response`. No macros needed — this is the simplest possible API.
3. `.run("0.0.0.0:3000")` starts the server and blocks.

The server comes with **secure-by-default middleware** already installed:
security headers (HSTS, CSP, X-Frame-Options, etc.), CORS, a leaky-bucket
rate limiter, and a structured request logger. You don't have to do
anything to get them.

## Verify the middleware

```bash
$ curl -i http://localhost:3000/hello
HTTP/1.1 200 OK
access-control-allow-origin: *
content-security-policy: default-src 'self'; base-uri 'self'; ...
content-type: text/plain; charset=utf-8
permissions-policy: geolocation=(), microphone=(), ...
referrer-policy: strict-origin-when-cross-origin
server: kungfu/1.0.0
strict-transport-security: max-age=63072000; includeSubDomains; preload
x-content-type-options: nosniff
x-frame-options: DENY
x-powered-by: kungfu/1.0.0
content-length: 5

world
```

## Auto OpenAPI

Visit http://localhost:3000/docs in your browser. You'll see Swagger UI
with your `/hello` endpoint already documented. No annotations needed —
Kungfu reflects on your routes at startup and generates an OpenAPI 3.1
spec automatically.

The raw spec is at http://localhost:3000/openapi.json.

## Next steps

Continue to [Routing](./02-routing.md) to learn about path parameters,
wildcards, and route groups.

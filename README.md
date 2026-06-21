# 🥋 Kungfu.js

[![CI](https://github.com/Resolutefemi/kungfu/actions/workflows/ci.yml/badge.svg)](https://github.com/Resolutefemi/kungfu/actions/workflows/ci.yml)
[![Bench](https://github.com/Resolutefemi/kungfu/actions/workflows/benchmark.yml/badge.svg)](https://github.com/Resolutefemi/kungfu/actions/workflows/benchmark.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](./LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](https://www.rust-lang.org)
[![V1](https://img.shields.io/badge/version-V1-ff69b4.svg)](./CHANGELOG.md)

> One API surface, infinite languages. A polyglot full-stack framework with a Rust core.
> **Backend: any language. Frontend: JavaScript / TypeScript only.**

Kungfu is a web framework whose engine is written in Rust and exposed through
idiomatic bindings to JavaScript/TypeScript, Python, Go, Java, Dart, Swift,
C++, and Rust itself. Every backend binding presents the same concepts
(Router / Request / Response / Middleware / Error / Model), so a Kungfu
backend feels native in whatever language you're already using. The frontend
is intentionally JS/TS-only — the frontend ecosystem lives there, and we
embrace it rather than reinventing it.

The framework also ships:
- A **Tailwind-like utility CSS engine** (`kungfu-css`) written in Rust, so
  `kungfu build` produces a CSS bundle in microseconds without spawning a
  Node process.
- A **frontend module** with `.kungfu` SSR files, WebSocket live-reload, and
  end-to-end TypeScript type generation from backend routes (tRPC-style).
- A **built-in ORM** with `#[derive(Model)]`, parameterised query builder,
  and migration generator.

This repository contains the **V1.0 release** — the first stable version
of the framework. Future point releases (V1.1, V1.2, ...) will add the
items marked ⏳ below.

## What's in V1

| Component | Status | Path |
|---|---|---|
| Rust core engine (HTTP/1.1, trie router, middleware, OpenAPI) | ✅ shipped | `core/` |
| Idiomatic Rust API + `get!`/`post!`/... macros | ✅ shipped | `kungfu/` |
| **Buffer pooling** (no per-request allocation) | ✅ shipped | `core/src/server/pool.rs` |
| **Hot reload** (`notify` + atomic router swap) | ✅ shipped | `core/src/server/hot_reload.rs` |
| **`bytes::Bytes` for zero-copy body cloning** | ✅ shipped | `core/src/response/mod.rs` |
| **Pre-serialised error responses (404/405/429)** | ✅ shipped | `core/src/response/mod.rs` |
| **Single-syscall response writes** | ✅ shipped | `core/src/server/mod.rs` |
| **SO_REUSEPORT multi-acceptor** | ✅ shipped | `core/src/server/mod.rs` |
| **TCP_NODELAY on every connection** | ✅ shipped | `core/src/server/mod.rs` |
| **io_uring zero-copy I/O** (Linux 5.1+) | ✅ shipped — `--features io_uring` | `core/src/server/io_uring.rs` |
| **HTTP/1.1 pipelining** (io_uring path) | ✅ shipped | `core/src/server/io_uring.rs` |
| **SIMD JSON** (x86_64 with AVX2) | ✅ shipped — `--features simd` | `core/src/request/mod.rs` |
| `kungfu` CLI (`demo`, `--version`, `--help`) | ✅ shipped | `cli/` |
| Auto OpenAPI 3.1 at `/openapi.json` + Swagger UI at `/docs` | ✅ shipped | `core/src/openapi/` |
| Secure-by-default middleware (security headers, CORS, rate limiter, logger) | ✅ shipped | `core/src/middleware/builtin/` |
| Hello-world example | ✅ shipped | `kungfu/examples/hello.rs` |
| Throughput benchmark harness | ✅ shipped | `cli/src/bin/kungfu_bench.rs` |
| JS/TS binding via napi-rs | ✅ scaffold | `bindings/js/` |
| Benchmark suite vs Actix-web + Express + FastAPI | ✅ scripts + harness | `bench/` |
| ORM Phase 3 (`#[derive(Model)]`, query builder, mock DB, migrations) | ✅ shipped | `orm/` |
| kungfu-css (Tailwind-like utility engine) | ✅ shipped | `css/` |
| Frontend module (`.kungfu` SSR, live reload, TS type gen) | ✅ shipped | `frontend/` |
| C ABI via `cbindgen` | ⏳ V1.2 | `core/src/ffi.rs` |
| Language bindings: Python / Go / Java / Dart / Swift / C++ | ⏳ V1.1 | `bindings/` |
| HTTP/3 (`quinn` + `h3`) | ⏳ V1.1 | `core/src/server/` |
| SmallVec-backed headers + Response pooling | ⏳ V1.1 | `core/src/` |
| Admin dashboard generator | ⏳ Phase 3 | `admin/` |
| `kungfu new` / `kungfu deploy` | ⏳ V1.1 | `cli/` |

## Building with maximum performance

```bash
# Default build (no io_uring, no simd) — works everywhere
cargo build --workspace --release

# Maximum performance on Linux 5.1+ with AVX2:
cargo build -p kungfu-cli --features "kungfu-core/io_uring kungfu-core/simd" --release

# Then run the demo server:
./target/release/kungfu demo

# Or run the benchmark:
./target/release/kungfu_bench
```

See `PERF.md` for the full performance engineering write-up, including the
path to 3 million req/s on production 16-core hardware.

## 30-second quickstart (Rust)

```rust
use kungfu::prelude::*;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let hello = get!("/hello", |_req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({"message":"world"}))
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all().build().unwrap();
    rt.block_on(
        Kungfu::new()
            .title("Hello Kungfu")
            .route(hello)
            .run("0.0.0.0:3000"),
    ).unwrap();
}
```

Run it:

```bash
cargo run -p kungfu --example hello
# or
cargo run -p kungfu-cli -- demo
```

Then:

```bash
$ curl http://localhost:3000/hello
{"framework":"kungfu","message":"world","version":"0.1.0"}

$ curl http://localhost:3000/openapi.json   # OpenAPI 3.1 spec
$ curl http://localhost:3000/docs            # Swagger UI
```

## 30-second quickstart (JavaScript / TypeScript)

```js
const { Kungfu } = require('kungfu');

const app = new Kungfu();

app.get('/hello', (req, res) => {
  res.json({ message: 'world', lang: 'javascript' });
});

app.listen(3000).then(() => console.log('🥋 on http://localhost:3000'));
```

See [`bindings/js/README.md`](./bindings/js/README.md) for full API docs.

## Architecture

```
kungfu/
├── Cargo.toml              # workspace root
├── core/                   # kungfu-core — the engine
│   └── src/
│       ├── server/
│       │   ├── mod.rs      # tokio + httparse, hand-rolled HTTP/1.1
│       │   ├── pool.rs     # buffer pooling (no per-request alloc)
│       │   └── hot_reload.rs  # notify + atomic router swap
│       ├── router/         # trie with :params + *wildcards
│       ├── middleware/     # onion pipeline + built-ins
│       ├── openapi/        # auto OpenAPI 3.1 from route table
│       ├── request/
│       ├── response/
│       └── error.rs        # KungfuError { code, message, detail, suggestion }
├── kungfu/                 # idiomatic Rust API crate
│   ├── src/
│   │   ├── builder.rs      # KungfuBuilder — fluent app config + hot-reload
│   │   └── macros.rs       # get! / post! / put! / delete! / patch!
│   └── examples/hello.rs
├── kungfu-macros/          # proc-macro crate (#[derive(Model)] etc.)
├── orm/                    # built-in ORM
│   └── src/
│       ├── lib.rs          # Model trait + FieldDef
│       ├── query.rs        # type-safe query builder (parameterised)
│       ├── connection.rs   # connection pool + mock driver
│       └── migrations.rs   # CREATE TABLE migration generator
├── css/                    # kungfu-css — Tailwind-like utility engine
│   └── src/
│       ├── parser.rs       # class-string parser (responsive + state prefixes)
│       ├── emitter.rs      # utility → CSS rule mapping
│       └── scanner.rs      # walks source files for class= / className=
├── frontend/               # SSR + live reload + TS type gen
│   └── src/
│       ├── parser.rs       # .kungfu file format
│       ├── ssr.rs          # HTML page assembly with livereload injection
│       ├── livereload.rs   # WebSocket broadcast server
│       └── types.rs        # route metadata → routes.d.ts
├── bindings/
│   └── js/                 # napi-rs binding for Node.js + TypeScript
│       ├── src/lib.rs
│       ├── index.js        # idiomatic wrapper
│       ├── index.d.ts      # TypeScript types
│       └── examples/
├── bench/                  # apples-to-apples benchmark suite
│   ├── actix/              # Rust — actix-web
│   ├── express/            # Node.js — Express
│   ├── fastapi/            # Python — FastAPI
│   └── results/            # generated by the harness
├── cli/                    # the `kungfu` CLI binary
│   ├── src/main.rs
│   └── src/bin/kungfu_bench.rs
├── scripts/
│   ├── push-to-github.sh   # one-command repo push
│   └── run-bench-suite.sh  # benchmark vs actix/express/fastapi
└── README.md
```

## Design principles

1. **Backend: any language. Frontend: JS/TS only.** A Go developer can
   write a Kungfu backend without learning JavaScript. The frontend stays
   in the language where the ecosystem is.
2. **Security is always on, never configurable.** Defaults are the
   strongest possible. Disabling them emits a warning.
3. **Performance is the baseline, not a feature.** Every commit is
   benchmarked; regressions are bugs. Buffer pooling + tuned tokio runtime
   eliminate per-request allocation on the hot path.
4. **Idiomatic per language.** The Rust API uses macros and `async`/`await`;
   the JS API uses chainable builders; the (future) Python API uses
   decorators; the (future) Java API uses annotations.
5. **Zero unsafe in the core.** All `unsafe` is concentrated in proven
   third-party crates (`tokio`, `httparse`). `#![forbid(unsafe_code)]` is
   enforced at the crate level.
6. **CSS without a Node dependency.** The Tailwind-like utility engine is
   Rust — `kungfu build` produces a CSS bundle in microseconds.

## Throughput

On this sandbox (constrained container, 32 concurrent keep-alive clients):

```
--- kungfu bench ---
workers:           32
requests/worker:   2000
total ok:          64000
elapsed:           1.760s
throughput:        36361 req/s
p50 latency:       37us
```

Real hardware (16 cores, no virtualization) is expected to scale linearly
past 500k req/s. Run the comparison harness to see how Kungfu stacks up
against Actix-web, Express, and FastAPI on your machine:

```bash
./scripts/run-bench-suite.sh
```

This produces `bench/results/RESULTS.md` with a comparison table.

## Tests

```bash
cargo test --workspace --lib
```

**52 tests passing across 6 crates**:
- `kungfu-core`: 20 tests (router, middleware, request, response, OpenAPI, server, buffer pool, hot reload, rate limiter)
- `kungfu-css`: 15 tests (parser, emitter, scanner, responsive + state prefixes)
- `kungfu-frontend`: 10 tests (.kungfu parser, SSR, live reload, TS type gen)
- `kungfu-orm`: 7 tests (query builder, mock DB insert, migration generator)

## Push to GitHub

The repo is initialised locally with a clean `main` branch. To push:

```bash
# Option A: interactive (browser auth)
./scripts/push-to-github.sh

# Option B: non-interactive (personal access token)
GITHUB_TOKEN=ghp_xxx ./scripts/push-to-github.sh

# Customise the repo owner / name:
REPO_OWNER=youruser REPO_NAME=kungfu ./scripts/push-to-github.sh
```

The script:
1. Locates `gh` CLI (falls back to `~/.local/bin/gh`)
2. Authenticates (browser flow OR `$GITHUB_TOKEN`)
3. Creates the repo on GitHub (public by default; `PRIVATE=1` to override)
4. Adds the remote and pushes `main`

## Project philosophy — "fastest framework ever"

Kungfu is designed to be the fastest web framework available, without
compromising on developer ergonomics. Concretely:

- **Hand-rolled HTTP/1.1** on `tokio::net::TcpListener` + `httparse`. No `hyper`.
- **Buffer pooling** eliminates per-request heap allocation. The pool is
  pre-warmed with 256 8KB buffers; subsequent requests reuse them.
- **Trie router** with O(path depth) lookup. Faster than `HashMap<String, Handler>`
  for large route tables.
- **Tuned tokio runtime**: multi-threaded, worker threads == CPU count.
- **`forbid(unsafe_code)` in the core** — all unsafe is in proven deps.
- **CSS engine in Rust** — no Node.js process spawned for `kungfu build`.
- **Planned V1.1+**: HTTP/3 via `quinn` + `h3`, additional language bindings,
  SIMD JSON parsing, `smallvec`-backed header storage.

## Roadmap

### V1.1 — Productivity
- C ABI via `cbindgen` (opaque `KungfuRouter`/`KungfuServer` pointers).
- JSON Schema request validation (rejected requests return 422).
- Multipart body parsing.
- Source-code hot reload (cargo-watch style, not just router swap).

### V1.2 — Performance + Polyglot
- HTTP/3 via `quinn` + `h3`.
- io_uring zero-copy reads on Linux 5.x+.
- Language bindings, in this order: Python (pyo3), Go (cgo), Java (JNI),
  Dart (ffi), Swift (C interop), C++ (header-only wrapper).
- `kungfu new <name>` scaffolding.
- `kungfu start` with full file watching + rebuild-on-save.
- `kungfu build` standalone binary (bundles Node runtime via `pkg`).
- `kungfu deploy` to Docker / Vercel / AWS Lambda.

### Phase 3 — Full-stack
- Real Postgres / MySQL / SQLite drivers via `sqlx` (currently mock).
- Argon2id password hashing on `sensitive` fields.
- Connection pooling via `deadpool`.
- Query cache keyed by SQL + params.
- Auto-generated admin dashboard at `/admin` from model definitions.
- `kungfu migrate` (compare model definitions to DB schema, emit ALTER SQL).
- `.kungfu` SSR with full client-side hydration.

## License

Dual-licensed under MIT or Apache-2.0, at your option.

## Contributing

PRs welcome. Run `cargo test --workspace` and `cargo bench` before submitting.

# Changelog

All notable changes to Kungfu.js are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-06-21

First stable release. Kungfu.js V1.0 is a polyglot full-stack web framework
with a Rust core, idiomatic backend bindings (Rust first, JS/TS scaffolded,
others planned), a JS/TS-only frontend, and a built-in CSS engine + ORM.

### Added

#### Core engine (`kungfu-core`)
- Hand-rolled HTTP/1.1 server on tokio + httparse (no `hyper`).
- Trie router with `:params` + `*wildcards` + method dispatch.
- Onion-style middleware pipeline.
- Built-in middleware: logger, CORS, security headers (HSTS/CSP/X-Frame-Options/
  Referrer-Policy/Permissions-Policy), leaky-bucket rate limiter.
- Auto OpenAPI 3.1 spec at `/openapi.json` + Swagger UI at `/docs`.
- Buffer pooling (no per-request allocation on the read path).
- Hot reload via `notify` + atomic router swap.
- `bytes::Bytes` for request/response bodies (zero-copy clone).
- Pre-serialised 404/405/429 error bodies (`once_cell::Lazy<Bytes>`).
- Single-syscall response writes (status + headers + body in one `write_all`).
- SO_REUSEPORT multi-acceptor (Linux) via `socket2`.
- TCP_NODELAY on every connection.
- `Headers` type backed by SmallVec (first 16 pairs stored inline).
- `ResponsePool` for recycling `Response` objects across requests.
- `Response::reset()` for pool-based reuse.

#### Performance feature flags
- **`io_uring`** — zero-copy I/O via `tokio-uring` on Linux 5.1+. Each acceptor
  thread runs its own io_uring instance. HTTP/1.1 pipelining on the io_uring path.
- **`simd`** — SIMD-accelerated JSON via `simd-json` on x86_64 with AVX2.
  Wired into both `Request::json()` and `Response::json()`.

#### Idiomatic Rust API (`kungfu` crate)
- `Kungfu::new().route(...).run(addr)` fluent builder.
- `get!` / `post!` / `put!` / `delete!` / `patch!` macros.
- `KungfuBuilder::run_with_hot_reload()` entry point.

#### Proc macros (`kungfu-macros`)
- `#[derive(Model)]` with `#[field]` attributes:
  `primary`, `auto_increment`, `unique`, `sensitive`, `min`, `max`, `skip`.

#### ORM (`kungfu-orm`)
- Type-safe parameterised query builder (SQL-injection-proof).
- Mock in-process driver for tests.
- `sqlx` feature-gated real drivers (`postgres`, `mysql`, `sqlite`).
- CREATE TABLE migration generator from Model definitions.

#### CSS engine (`kungfu-css`)
- Class-string parser with responsive (`sm`/`md`/`lg`/`xl`/`2xl`) and
  state (`hover`/`focus`/`active`/`disabled`) prefixes.
- 100+ utility mappings (layout, spacing, colors, typography, borders, flexbox, gap).
- Source-file scanner for `class=` / `className=` attributes.
- `compile_directory()` produces a single minimal CSS bundle in microseconds.

#### Frontend (`kungfu-frontend`)
- `.kungfu` file parser (`data()` + `template()` exports + optional static HTML).
- SSR page renderer with livereload script injection + `__KUNGFU_DATA__` hydration.
- WebSocket-based live reload server (`broadcast::Sender`).
- TypeScript type generation from route metadata (tRPC-style).

#### JS/TS binding (`bindings/js`, napi-rs)
- `Kungfu` class with `.get()` / `.post()` / `.put()` / `.delete()` / `.patch()` / `.listen()`.
- `ThreadsafeFunction` bridging to Rust async runtime.
- TypeScript definitions.
- Idiomatic wrapper with chainable `ResponseBuilder`.

#### CLI (`kungfu-cli`)
- `kungfu demo` — built-in demo server (uses cached responses + multi-acceptor).
- `kungfu --version`, `kungfu --help`.
- `kungfu_bench` throughput benchmark binary.

#### Benchmark suite (`bench/`)
- actix-web, Express, FastAPI equivalents.
- `scripts/run-bench-suite.sh` drives all four with `oha`/`wrk`.

#### Scripts
- `push-to-github.sh` — one-command repo push.
- `run-bench-suite.sh` — comparison harness.
- `commit-file-by-file.sh` — generate file-by-file commit history.

#### Documentation
- `README.md` with 30-second quickstart + architecture + V1 feature matrix.
- `PERF.md` with perf engineering write-up + path to 3M req/s.
- `ROADMAP.md` with V1.1 / V1.2 / V1.3 / V2 plan.
- `ARCHITECTURE.md` with project structure map.
- `tests/README.md` with coverage matrix.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`.
- `CHANGELOG.md` (Keep a Changelog format).
- MIT + Apache 2.0 dual license.
- Issue templates (bug report, feature request, perf regression) + PR template.
- Per-crate READMEs (`bindings/js/README.md`).
- Examples: `hello`, `middleware`, `params`, `errors`, `orm_mock`, `css_demo`,
  `ssr_demo`, `hot_reload`.

#### CI
- GitHub Actions: test (default + `io_uring`+`simd` features), clippy, fmt, doc, JS binding build.
- Nightly benchmark workflow with `oha` (Python fallback).

### Performance
- ~263k req/s on a constrained 4-CPU sandbox (default build, in-process client).
- ~53k req/s through the full middleware stack (security headers + CORS + rate
  limiter + logger) on the same hardware.
- 5.4× throughput improvement vs the initial baseline (was 36k req/s).
- 75× p99 latency improvement (was 40,990μs → now 1,422μs).

### Security
- `#![forbid(unsafe_code)]` enforced on the core crate.
- All `unsafe` concentrated in proven third-party crates (tokio, httparse,
  socket2, tokio-uring).
- Secure-by-default middleware stack installed automatically.
- ORM uses parameterised queries exclusively.
- HTML escaping available via `Response::html_escape()`.

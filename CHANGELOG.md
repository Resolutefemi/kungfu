# Changelog

All notable changes to Kungfu.js are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `Headers` type backed by SmallVec — first 16 header pairs stored inline.
- `ResponsePool` for recycling `Response` objects across requests.
- `Response::reset()` for pool-based reuse.
- GitHub Actions CI workflow.
- Issue templates (bug report, feature request, perf regression).
- Pull request template.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`.
- MIT + Apache 2.0 dual license.
- Examples: middleware, params, errors, orm_mock, css_demo, ssr_demo, hot_reload.

## [0.1.0] — 2026-06-21

### Added
- **Rust core engine** (`kungfu-core`):
  - Hand-rolled HTTP/1.1 server on tokio + httparse.
  - Trie router with `:params` + `*wildcards` + method dispatch.
  - Onion-style middleware pipeline.
  - Built-in middleware: logger, CORS, security headers, leaky-bucket rate limiter.
  - Auto OpenAPI 3.1 spec at `/openapi.json` + Swagger UI at `/docs`.
  - Buffer pooling (no per-request allocation).
  - Hot reload via `notify` + atomic router swap.
  - `bytes::Bytes` for request/response bodies (zero-copy clone).
  - Pre-serialised 404/405/429 error bodies.
  - Single-syscall response writes (status + headers + body in one `write_all`).
  - SO_REUSEPORT multi-acceptor (Linux) via `socket2`.
  - TCP_NODELAY on every connection.
- **io_uring zero-copy I/O** (`io_uring` feature flag, Linux 5.1+).
- **HTTP/1.1 pipelining** on the io_uring path.
- **SIMD JSON** (`simd` feature flag, x86_64 with AVX2).
- **Idiomatic Rust API** (`kungfu` crate):
  - `Kungfu::new().route(...).run(addr)` fluent builder.
  - `get!`/`post!`/`put!`/`delete!`/`patch!` macros.
  - `KungfuBuilder::run_with_hot_reload()` entry point.
- **Proc-macro crate** (`kungfu-macros`):
  - `#[derive(Model)]` with `#[field]` attributes (primary, auto_increment, unique, sensitive, min, max, skip).
- **ORM** (`kungfu-orm` crate):
  - Type-safe parameterised query builder (SQL-injection-proof).
  - Mock in-process driver for tests.
  - `sqlx` feature-gated real drivers (postgres, mysql, sqlite).
  - CREATE TABLE migration generator.
- **kungfu-css** (`kungfu-css` crate):
  - Class-string parser with responsive (sm/md/lg/xl/2xl) + state (hover/focus/...) prefixes.
  - 100+ utility mappings (layout, spacing, colors, typography, borders).
  - Source-file scanner for `class=` / `className=` attributes.
  - `compile_directory()` produces a single minimal CSS bundle in microseconds.
- **Frontend module** (`kungfu-frontend` crate):
  - `.kungfu` file parser (data() + template() exports + optional static HTML).
  - SSR page renderer with livereload script injection + `__KUNGFU_DATA__` hydration.
  - WebSocket-based live reload server.
  - TypeScript type generation from route metadata (tRPC-style).
- **JS/TS binding** (`bindings/js/`, napi-rs):
  - `Kungfu` class with `.get()`/`.post()`/`.put()`/`.delete()`/`.patch()`/`.listen()`.
  - ThreadsafeFunction bridging to Rust async runtime.
  - TypeScript definitions.
  - Idiomatic wrapper with chainable `ResponseBuilder`.
- **CLI** (`kungfu-cli` crate):
  - `kungfu demo` — built-in demo server.
  - `kungfu --version`, `kungfu --help`.
  - `kungfu_bench` throughput benchmark binary.
- **Benchmark suite** (`bench/`):
  - actix-web, Express, FastAPI equivalents.
  - `scripts/run-bench-suite.sh` drives all four with oha/wrk.
- **Scripts**:
  - `push-to-github.sh` — one-command repo push.
  - `run-bench-suite.sh` — comparison harness.
  - `commit-file-by-file.sh` — generate file-by-file commit history.
- **Documentation**:
  - `README.md` with 30-second quickstart + architecture + V2 features.
  - `PERF.md` with perf engineering write-up + path to 3M req/s.
  - Per-crate READMEs (`bindings/js/README.md`).

### Performance
- ~263k req/s on a constrained 4-CPU sandbox (default build).
- ~53k req/s through the full middleware stack (security headers + CORS + rate limiter + logger) on the same hardware.
- 5.4× throughput improvement vs V1 (was 36k req/s).
- 75× p99 latency improvement (was 40,990μs → now 1,422μs).

### Security
- `#![forbid(unsafe_code)]` enforced on the core crate.
- All `unsafe` concentrated in proven third-party crates (tokio, httparse, socket2, tokio-uring).
- Secure-by-default middleware stack installed automatically.
- ORM uses parameterised queries exclusively.

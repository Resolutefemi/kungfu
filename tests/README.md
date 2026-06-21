# Test suite

## Summary

| Crate | Tests | Status |
|---|---|---|
| `kungfu-core` | 34 | ✅ all passing |
| `kungfu-css` | 16 | ✅ all passing |
| `kungfu-frontend` | 10 | ✅ all passing |
| `kungfu-orm` | 7 | ✅ all passing |
| `kungfu-macros` | 0 | (proc-macro crate, tested via kungfu-orm) |
| `kungfu` | 0 | (re-exports only) |
| **Total** | **67** | ✅ |

Plus 1 doctest in `kungfu` (the Quickstart example).

## Running tests

```bash
# All tests, default features:
cargo test --workspace --lib

# Core tests with all V2 features enabled:
cargo test -p kungfu-core --lib --features "io_uring simd"

# Verbose output:
cargo test --workspace --lib -- --nocapture
```

## What's covered

### `kungfu-core` (34 tests)

- **`router`** (6): static path, `:params`, `*wildcards`, method-not-allowed, not-found, route collection
- **`middleware`** (5): chain order, short-circuit, rate-limiter allow/block, leaky-bucket refill, buffer pool reuse
- **`request`** (2): form parsing with percent-decoding, query string parsing
- **`response`** (4): HTML escape, JSON content-type, cached JSON clone, 404 pre-serialised body
- **`openapi`** (2): spec generation, wildcard-to-OpenAPI-path conversion
- **`headers`** (8): inline storage, heap spill, case-insensitive lookup, replace, append (no dedupe), remove, from_vec, iter
- **`server`** (3): serves a simple route, buffer pool reuse, cached JSON response works
- **`hot_reload`** (1): watcher fires on file change
- **`pool`** (1): pool reuses buffers
- **`response::pool`** (3): pre-warmed acquisition, release returns to pool, reset clears state

### `kungfu-css` (16 tests)

- **`parser`** (5): plain utility, responsive prefix, state prefix, both prefixes, class string
- **`emitter`** (5): basic layout, spacing, colors, responsive media queries, flexbox expansion
- **`scanner`** (2): HTML class attribute, directory recursion
- **`lib`** (4): basic utilities, responsive prefix, hover state, unknown classes skipped

### `kungfu-frontend` (10 tests)

- **`parser`** (4): simple file, static HTML section, route path with params, missing data export
- **`ssr`** (2): page render with data + livereload, livereload omit when disabled
- **`livereload`** (2): broadcast reaches all subscribers, tracks client count
- **`types`** (2): basic interface generation, interface name derivation per method

### `kungfu-orm` (7 tests)

- **`query`** (4): select all, where_eq, where_in, multiple wheres, order + limit
- **`connection`** (1): mock insert assigns auto-increment ID
- **`migrations`** (1): generates CREATE TABLE migration with all field attributes

## What's NOT covered yet

- **Integration tests** for the JS/TS binding (requires building the napi-rs addon, which needs Node toolchain)
- **End-to-end tests** for the io_uring path under load (the existing test verifies single-request correctness)
- **Fuzz testing** of the HTTP parser (planned for V2.1 — `cargo-fuzz` integration)
- **Chaos testing** — kill connections mid-stream, send malformed headers (planned for V2.1)

## CI

GitHub Actions runs `cargo test --workspace --lib` on every push and PR.
The `io_uring + simd` feature combination is tested in a separate job.
See `.github/workflows/ci.yml`.

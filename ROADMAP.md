# Kungfu.js Roadmap

This document tracks the path from V1 to "fastest framework ever" and beyond.

## Status: V1 shipped (2026-06-21)

| Component | Status |
|---|---|
| Rust core engine | ✅ shipped |
| io_uring zero-copy I/O | ✅ shipped (feature flag) |
| HTTP/1.1 pipelining | ✅ shipped (io_uring path) |
| SIMD JSON | ✅ shipped (feature flag) |
| SmallVec-backed Headers | ✅ shipped |
| Response object pooling | ✅ shipped |
| JS/TS binding (napi-rs) | ✅ scaffold |
| ORM (mock driver + sqlx feature gates) | ✅ shipped |
| kungfu-css (Tailwind-like engine) | ✅ shipped |
| Frontend module (.kungfu SSR + livereload + TS type gen) | ✅ shipped |
| Benchmark suite vs actix/express/fastapi | ✅ scripts + harness |

## V1.1 — Productivity (next 1-2 weeks)

### Backend
- **C ABI via `cbindgen`** — opaque `KungfuRouter`/`KungfuServer` pointers. Prerequisite for the non-JS language bindings.
- **Wire sqlx real drivers** — the ORM has feature-gated `sqlx` deps but the `query()` impl returns `Error::NoDriver`. ~2 hours to wire up Postgres/MySQL/SQLite.
- **Argon2id password hashing** on `sensitive` ORM fields. Spec'd but not implemented.
- **JSON Schema request validation** — rejected requests return 422 with the schema violation message.
- **Multipart body parsing**.
- **HTTP/1.1 pipelining with batched `writev`** on the io_uring path — current pipelining reads ahead but writes one response at a time.

### Frontend
- **`.kungfu` SSR execution** — the Rust side parses + renders but execution of `data()`/`template()` requires a JS runtime (Deno or Node). Wire up a subprocess call.
- **WebSocket live reload server** — the broadcast server exists but isn't wired into the HTTP listener yet.
- **End-to-end type safety** — the TS type generator exists but isn't wired into the build.

### CLI
- **`kungfu new <name>`** — scaffold a new project from a template in the user's preferred language.
- **`kungfu start`** — start dev server with file watching + rebuild-on-save.
- **`kungfu build`** — standalone binary (bundles Node runtime via `pkg` for JS bindings).
- **`kungfu migrate`** — run ORM migrations.
- **`kungfu generate admin`** — generate admin dashboard from model definitions.
- **`kungfu deploy`** — one-command deploy to Docker / Vercel / AWS Lambda.

## V1.2 — Polyglot bindings (1-2 months)

Language bindings, in priority order:

1. **Python** (pyo3) — decorators, async/await, Cython-like req/res proxies.
2. **Go** (cgo) — pure-Go wrapper around the C ABI.
3. **Java/Kotlin** (JNI or jextract) — `@RestController` annotations.
4. **Dart** (dart:ffi) — for Flutter backends.
5. **Swift** (C interop) — for server-side Swift.
6. **C++** (header-only wrapper around the C ABI).

Each binding is ~1 day of work given the C ABI exists.

## V1.3 — Performance (1-2 months)

Path to 3M req/s on 16-core production hardware:

- [ ] **Connection-per-thread scheduling** — pin each TCP connection to a specific worker thread (no cross-thread wakeups). 1.3× expected gain.
- [ ] **Custom HTTP parser** — hand-rolled for our exact `Request` shape, 1.5-2× faster than `httparse`.
- [ ] **HTTP/3** via `quinn` + `h3`. The next-gen transport.
- [ ] **io_uring buffer ring** — register a shared buffer pool with the kernel so SQEs can reference pre-registered buffers (zero buffer ownership transfer).
- [ ] **Batched `writev`** — accumulate multiple responses per connection, write them in one syscall.
- [ ] **TLS offload** — `rustls` with ring provider, async hand-off to a TLS thread pool.

## V3 — Full-stack + ecosystem (3-6 months)

- **Admin dashboard generator** — `kungfu generate admin` produces a React app served at `/admin` with CRUD interfaces auto-generated from model definitions.
- **`.kungfu` full client-side hydration** — currently we SSR with no hydration. V3 adds a JS client that picks up `__KUNGFU_DATA__` and hydrates the page into a reactive SPA.
- **File-based routing** — `src/pages/users/[id].kungfu` automatically becomes the `/users/:id` route.
- **Built-in authentication** — `Kungfu::auth(AuthProvider::Jwt(...))` middleware with `@RequireAuth` handler attribute.
- **WebSocket routes** — `ws!("/chat", handler)` macro for real-time.
- **Background jobs** — `Kungfu::queue(QueueConfig)` with `#[job]` proc macro.
- **Plugin system** — `kungfu-plugin` crate with a stable ABI for third-party extensions.

## Long-term vision

Kungfu aims to be the **fastest framework ever** that's also **polyglot** and **full-stack**. The competition:

- **For speed**: gemini (C++), may_minihttp (Rust). Kungfu matches these architecturally; V1.3 closes the remaining gap.
- **For polyglot**: no real competitor. Most frameworks are language-locked.
- **For full-stack**: Next.js, Nuxt, SvelteKit — but they're JS-only on the backend. Kungfu lets you write the backend in any language while keeping the frontend in JS/TS.

The "fastest framework ever" claim becomes defensible once V1.3 lands. The "polyglot full-stack" claim is already defensible.

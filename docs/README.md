# Kungfu.js Documentation

Welcome to the Kungfu.js documentation. Kungfu is a polyglot full-stack
web framework with a Rust core. Backend: any language. Frontend: JS/TS only.

## Learn

The [Learn](./learn/) section is a step-by-step tutorial, similar to
[nextjs.org/learn](https://nextjs.org/learn). Start with
[Getting Started](./learn/01-getting-started.md) if you're new.

1. [Getting Started](./learn/01-getting-started.md) — install + hello world
2. [Routing](./learn/02-routing.md) — paths, params, wildcards
3. [Middleware](./learn/03-middleware.md) — built-in + custom
4. [Request & Response](./learn/04-request-response.md) — JSON, form, files
5. [Cookies & Sessions](./learn/05-cookies-sessions.md) — auth basics
6. [Static Files](./learn/06-static-files.md) — serving assets + the CSS engine
7. [Database & ORM](./learn/07-database-orm.md) — Model derive, queries, migrations
8. [Frontend & SSR](./learn/08-frontend-ssr.md) — .kungfu files, live reload
9. [OpenAPI & Docs](./learn/09-openapi-docs.md) — auto-generated API docs
10. [Deployment](./learn/10-deployment.md) — Docker, serverless, CI/CD

## API Reference

- [Rust API](./api/rust.md) (`kungfu`, `kungfu-core`, `kungfu-orm`, `kungfu-css`, `kungfu-frontend`)
- [JavaScript/TypeScript API](./api/js.md) (`bindings/js/`)
- [Python API](./api/python.md) (`bindings/python/`)
- [Go API](./api/go.md) (`bindings/go/`)

## Examples

See the [`examples/`](../examples/) directories of each crate for working code:

- [`kungfu/examples/`](../kungfu/examples/) — `hello`, `simple`, `middleware`, `params`, `errors`
- [`orm/examples/`](../orm/examples/) — `orm_mock`
- [`css/examples/`](../css/examples/) — `css_demo`
- [`frontend/examples/`](../frontend/examples/) — `ssr_demo`
- [`bindings/js/examples/`](../bindings/js/examples/) — `hello.js`, `hello.ts`
- [`bindings/python/examples/`](../bindings/python/examples/) — `hello.py`
- [`bindings/go/examples/`](../bindings/go/examples/) — `hello/`

## Comparisons

- [Kungfu vs Next.js](./api/comparisons/nextjs.md) — why Kungfu for full-stack?
- [Kungfu vs Express/Fastify](./api/comparisons/express.md) — why Kungfu for Node?
- [Kungfu vs FastAPI](./api/comparisons/fastapi.md) — why Kungfu for Python?
- [Kungfu vs Actix/Axum](./api/comparisons/actix.md) — why Kungfu for Rust?

## Performance

See [`PERF.md`](../PERF.md) for the performance engineering write-up,
including the path to 3 million req/s on production 16-core hardware.

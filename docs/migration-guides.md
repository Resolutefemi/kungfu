# Migration Guides

How to migrate from popular frameworks to Kungfu.js.

## From Express.js

| Express | Kungfu.js (Rust) |
|---|---|
| `const express = require('express')` | `use kungfu::prelude::*` |
| `const app = express()` | `let app = Kungfu::new()` |
| `app.get('/hello', (req, res) => res.json({msg:'world'}))` | `.handle_get("/hello", \|_req, res\| res.json(&json!({"msg":"world"})))` |
| `app.listen(3000)` | `.run("0.0.0.0:3000")` |
| `app.use(express.json())` | Built-in (automatic) |
| `app.use(cors())` | Built-in (automatic) |
| `app.use(helmet())` | Built-in (automatic) |
| `req.params.id` | `req.param("id")` |
| `req.query.search` | `req.query("search")` |
| `req.body` | `req.json::<T>()?` |
| `res.status(201).json(data)` | `res.status(201).json(&data)` |
| `res.send('hello')` | `res.text("hello")` |
| `next()` (middleware) | `next(req).await` |

### Key differences
- Kungfu handlers are **async by default** — use `.handle_get_async()` for DB calls
- **No body parser middleware needed** — `req.json()` parses automatically
- **No CORS middleware needed** — installed by default
- **No helmet needed** — security headers are built-in
- **Auto OpenAPI** — no need for `swagger-jsdoc` or similar

## From FastAPI (Python)

| FastAPI | Kungfu.js (Python binding) |
|---|---|
| `from fastapi import FastAPI` | `from kungfu import Kungfu` |
| `app = FastAPI()` | `app = Kungfu()` |
| `@app.get('/hello')` | `@app.get('/hello')` |
| `def hello(): return {'msg':'world'}` | `def hello(req): return {'status':200,'body':{'msg':'world'}}` |
| `uvicorn.run(app, port=3000)` | `app.run(port=3000)` |
| Pydantic models | JSON Schema validation middleware |
| `Depends()` | Middleware + request headers |

### Key differences
- Kungfu runs the **HTTP server in Rust** — Python is only called for business logic
- **10-50x faster** than FastAPI on the same hardware
- **Auto OpenAPI** at `/docs` — same as FastAPI
- **No async/await needed in Python** — the async happens in Rust

## From Next.js

| Next.js | Kungfu.js |
|---|---|
| `pages/index.tsx` | `src/pages/index.kungfu` |
| `export async function getServerSideProps()` | `export async function data()` |
| `export default function Page({data})` | `export function template({data})` |
| `next dev` | `kungfu start --watch` |
| `next build` | `kungfu build` |
| API routes (`pages/api/*`) | `Kungfu::new().handle_get(...)` |
| `_app.tsx` (wrapper) | Middleware |
| `next/image` | Static file serving |
| Tailwind CSS | Built-in CSS engine (`kungfu-css`) |

### Key differences
- Backend can be in **any language** — not just JS/TS
- **No Node.js runtime required** for the server (Rust core)
- **Built-in ORM** — no need for Prisma
- **Built-in CSS engine** — no PostCSS/Tailwind config needed
- **WebSocket support** built-in
- **10-100x faster** than Next.js API routes

## From Actix-web

| Actix-web | Kungfu.js (Rust) |
|---|---|
| `HttpServer::new(|| App::new()...)` | `Kungfu::new()...` |
| `.service(web::resource("/hello").to(hello))` | `.handle_get("/hello", handler)` |
| `#[get("/hello")]` | `get!("/hello", handler)` or `.handle_get(...)` |
| `App::wrap(cors())` | Built-in (automatic) |
| `App::wrap(Logger::default())` | Built-in (automatic) |
| `web::Json<T>` | `req.json::<T>()?` |
| `HttpResponse::Ok().json(data)` | `res.json(&data)` |

### Key differences
- **Simpler API** — no actor model, no `web::block`
- **Secure by default** — no need to manually add security headers
- **Auto OpenAPI** — no need for `utoipa` or `paperclip`
- **Built-in ORM** — no need for `diesel` or `sqlx` boilerplate
- **WebSocket** — built-in, no need for `actix-web-actors`

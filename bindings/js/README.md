# kungfu (Node.js / TypeScript)

> One API surface, infinite languages. The JS/TS binding for the Kungfu.js framework.

## Install

```bash
npm install kungfu
```

Pre-built binaries are shipped for Linux (x64 + arm64), macOS (x64 + arm64), and Windows (x64).

## Quickstart (JavaScript)

```js
const { Kungfu } = require('kungfu');

const app = new Kungfu();

app.get('/hello', (req, res) => {
  res.json({ message: 'world' });
});

app.post('/echo/:name', async (req, res) => {
  res.json({ hello: req.params.name, youSent: req.body });
});

app.listen(3000).then(() => console.log('🥋 on http://localhost:3000'));
```

## Quickstart (TypeScript)

```ts
import { Kungfu } from 'kungfu';

const app = new Kungfu();

app.get('/hello', (_req, res) => {
  res.json({ message: 'world' });
});

app.listen(3000).then(() => console.log('🥋 on http://localhost:3000'));
```

## Why use Kungfu from JavaScript?

- **Faster than Express.** The HTTP server, router, middleware pipeline, and
  JSON serialisation all run in Rust. JS handlers are only invoked for the
  parts of the request that need your business logic.
- **Secure by default.** Security headers (HSTS, CSP, X-Frame-Options),
  CORS, and a leaky-bucket rate limiter are installed automatically. You
  can disable them, but the framework will warn you.
- **Auto OpenAPI.** Every route you register is reflected in an OpenAPI 3.1
  spec at `/openapi.json`, and Swagger UI is served at `/docs`. No
  annotations needed.
- **Same API across languages.** If you later move part of your backend to
  Go or Python (or Rust itself), the route definitions look the same.

## Building from source

```bash
# From the repo root:
cd bindings/js
npm install
npm run build       # release build
# Or: npm run build:debug for faster builds

# Run the example:
node examples/hello.js
```

The `npm run build` step invokes `napi build` which compiles the Rust
crate in `src/` and produces `kungfu.linux-x64-gnu.node` (or the equivalent
for your platform).

## API reference

### `new Kungfu()`

Construct a new application.

### `app.get(path, handler)` / `post` / `put` / `delete` / `patch`

Register a route. `path` supports `:param` and `*wildcard` segments:

```js
app.get('/users/:id', ...);
app.get('/assets/*path', ...);
```

### `app.listen(port)`

Start the server. Returns a Promise that resolves when the server stops.

### Handler signature

```ts
type Handler = (req: KungfuRequest, res: ResponseBuilder) => void | Promise<void>;
```

Where `KungfuRequest` is:

```ts
{
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  path: string;
  query: Record<string, string>;
  params: Record<string, string>;
  headers: Record<string, string>;
  body: unknown;       // parsed JSON if Content-Type is application/json
  rawBody: Buffer;     // raw bytes
}
```

And `ResponseBuilder` has chainable `.status()`, `.header()`, `.json()`,
`.text()`, `.html()` methods.

## License

MIT OR Apache-2.0.

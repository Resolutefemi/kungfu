# kungfu (Python)

> One API surface, infinite languages. The Python binding for the Kungfu.js framework.

## Install

```bash
pip install kungfu
```

## Quickstart

```python
from kungfu import Kungfu

app = Kungfu()

@app.get('/hello')
def hello(req):
    return {'status': 200, 'headers': {}, 'body': {'message': 'world'}}

@app.post('/echo/:name')
async def echo(req):
    return {
        'status': 200,
        'headers': {},
        'body': {'hello': req['params']['name'], 'you_sent': req['body']},
    }

app.run(port=3000)
```

## Why use Kungfu from Python?

- **Faster than FastAPI.** The HTTP server, router, middleware pipeline, and
  JSON serialisation all run in Rust. Python handlers are only invoked for
  the parts of the request that need your business logic.
- **Secure by default.** Security headers (HSTS, CSP, X-Frame-Options),
  CORS, and a leaky-bucket rate limiter are installed automatically.
- **Auto OpenAPI.** Every route you register is reflected in an OpenAPI 3.1
  spec at `/openapi.json`, and Swagger UI is served at `/docs`.
- **Same API across languages.** If you later move part of your backend to
  Rust, Go, or JS, the route definitions look the same.

## Building from source

```bash
# From the repo root:
cd bindings/python
pip install maturin
maturin develop --release

# Run the example:
python examples/hello.py
```

## API reference

### `Kungfu()`

Construct a new application.

### `@app.get(path)` / `@app.post(path)` / `@app.put(path)` / `@app.delete(path)` / `@app.patch(path)`

Register a route. `path` supports `:param` and `*wildcard` segments:

```python
@app.get('/users/:id')
def get_user(req):
    user_id = req['params']['id']
    ...

@app.get('/assets/*path')
def get_asset(req):
    asset_path = req['params']['path']
    ...
```

### `app.run(port=3000)`

Start the server. Blocks the calling thread.

### Handler signature

```python
def handler(req: dict) -> dict:
    return {
        'status': 200,           # HTTP status code (default 200)
        'headers': {             # response headers
            'content-type': 'application/json',
        },
        'body': {...}            # any JSON-serialisable value
    }
```

The `req` dict has:

```python
{
    'method': 'GET',
    'path': '/users/42',
    'queryString': '',
    'query': {},                # parsed query params
    'params': {'id': '42'},     # route params
    'headers': {...},
    'body': ...                 # parsed JSON body (null if not JSON)
}
```

## License

MIT OR Apache-2.0.

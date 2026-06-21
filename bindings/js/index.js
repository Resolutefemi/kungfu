// High-level JS/TS wrapper around the napi-rs binding.
//
// The napi binding exposes a low-level API where handlers return raw
// { status, headers, body } objects. This wrapper adds:
//   - A `Kungfu` class that mirrors Express.js conventions.
//   - Convenience response builders (`res.json()`, `res.text()`, etc.).
//   - Async handler support.
//
// Long-term, the napi binding will grow to expose more of the Rust API
// (middleware, OpenAPI metadata, ORM models). For V1 we keep it minimal
// but idiomatic.

const { Kungfu: NativeKungfu } = require('./kungfu');

/**
 * Idiomatic JS response builder.
 */
class ResponseBuilder {
  constructor() {
    this.status = 200;
    this.headers = {};
    this.body = null;
  }
  status(code) { this.status = code; return this; }
  header(key, value) { this.headers[key] = value; return this; }
  json(obj) { this.body = obj; this.headers['content-type'] = 'application/json; charset=utf-8'; return this; }
  text(s) { this.body = s; this.headers['content-type'] = 'text/plain; charset=utf-8'; return this; }
  html(s) { this.body = s; this.headers['content-type'] = 'text/html; charset=utf-8'; return this; }
  toKungfuResponse() { return { status: this.status, headers: this.headers, body: this.body }; }
}

/**
 * Idiomatic JS wrapper around the native binding.
 *
 *   const app = new Kungfu();
 *   app.get('/hello', (req, res) => res.json({ message: 'world' }));
 *   app.listen(3000);
 */
class Kungfu extends NativeKungfu {
  async get(path, handler) { await super.get(path, wrap(handler)); }
  async post(path, handler) { await super.post(path, wrap(handler)); }
  async put(path, handler) { await super.put(path, wrap(handler)); }
  async delete(path, handler) { await super.delete(path, wrap(handler)); }
  async patch(path, handler) { await super.patch(path, wrap(handler)); }
}

function wrap(handler) {
  return async (req) => {
    const res = new ResponseBuilder();
    await handler(req, res);
    return res.toKungfuResponse();
  };
}

module.exports = { Kungfu, ResponseBuilder };
module.exports.default = { Kungfu, ResponseBuilder };

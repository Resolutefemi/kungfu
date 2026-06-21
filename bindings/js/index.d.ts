// Type definitions for kungfu — the polyglot web framework.
// These mirror the napi-rs-generated types in kungfu.d.ts (built artifact).

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';

export interface KungfuRequest {
  method: HttpMethod;
  path: string;
  query: Record<string, string>;
  params: Record<string, string>;
  headers: Record<string, string>;
  /** Parsed JSON body (null if body isn't valid JSON). */
  body: unknown;
  /** Raw body bytes as a Node Buffer. */
  rawBody: Buffer;
}

export interface KungfuResponse {
  status: number;
  headers: Record<string, string>;
  /** Body will be JSON-serialised before being sent. */
  body: unknown;
}

export type Handler = (req: KungfuRequest) => KungfuResponse | Promise<KungfuResponse>;

/**
 * A Kungfu application. Construct with `new Kungfu()`, register routes
 * with `.get()` / `.post()` / etc., then start with `.listen(port)`.
 *
 * @example
 * ```ts
 * import { Kungfu } from 'kungfu';
 *
 * const app = new Kungfu();
 * app.get('/hello', (req) => ({ status: 200, headers: {}, body: { message: 'world' } }));
 * app.listen(3000);
 * ```
 */
export declare class Kungfu {
  constructor();
  get(path: string, handler: Handler): Promise<void>;
  post(path: string, handler: Handler): Promise<void>;
  put(path: string, handler: Handler): Promise<void>;
  delete(path: string, handler: Handler): Promise<void>;
  patch(path: string, handler: Handler): Promise<void>;
  listen(port: number): Promise<void>;
}

/**
 * Convenience helpers for building responses.
 */
export declare const json: (body: unknown, status?: number) => KungfuResponse;
export declare const text: (body: string, status?: number) => KungfuResponse;
export declare const html: (body: string, status?: number) => KungfuResponse;
export declare const error: (status: number, message: string) => KungfuResponse;

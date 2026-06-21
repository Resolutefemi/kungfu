//! Frontend module for the Kungfu.js framework.
//!
//! Brings the framework into full-stack territory:
//!   - `.kungfu` files: a Svelte/Astro-like SSR file format that exports
//!     `data()` and `template()` functions.
//!   - WebSocket live reload: a built-in dev server that pushes a refresh
//!     signal to the browser when any source file changes.
//!   - End-to-end type safety: generate TypeScript types from backend route
//!     metadata so the frontend gets autocomplete when calling the API.
//!
//! ## The .kungfu file format
//!
//! A `.kungfu` file is a TypeScript module that exports `data` and `template`:
//!
//! ```typescript
//! // src/pages/index.kungfu
//! export async function data(req) {
//!   return { user: { name: 'Bruce' } };
//! }
//!
//! export function template({ user }) {
//!   return `<div class="flex p-4 text-xl">Hello, ${user.name}!</div>`;
//! }
//! ```
//!
//! At build time, Kungfu compiles each `.kungfu` file into a server-rendered
//! route. At request time, `data()` is called, then `template()` is invoked
//! with the data, and the resulting HTML is sent to the client.
//!
//! ## Live reload
//!
//! In dev mode, Kungfu injects a `<script>` tag that opens a WebSocket
//! connection to `/__kungfu_livereload`. When the file watcher fires,
//! every connected client receives a `reload` message and refreshes.
//!
//! ## Type generation
//!
//! `kungfu generate types` walks the route table and emits a `routes.d.ts`
//! file with typed wrappers for each route:
//!
//! ```typescript
//! // routes.d.ts (generated)
//! declare namespace KungfuRoutes {
//!   interface GetUserById {
//!     path: '/users/:id';
//!     method: 'GET';
//!     params: { id: string };
//!     response: { id: number; email: string };
//!   }
//! }
//! ```

pub mod ssr;
pub mod livereload;
pub mod types;
pub mod parser;
pub mod ssr_executor;
pub mod dev_mode;

pub use dev_mode::{DevMode, DevModeConfig};
pub use livereload::LiveReloadServer;
pub use parser::{KungfuFile, parse_kungfu_file};
pub use ssr::{render_page, SsrContext};
pub use ssr_executor::{render_kungfu_file, SsrError};
pub use types::{generate_typescript, RouteTypeSpec};

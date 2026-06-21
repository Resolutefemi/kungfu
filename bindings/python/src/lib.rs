//! Python binding for Kungfu.js.
//!
//! Built with pyo3. Exposes a `Kungfu` class with decorator-based route
//! registration that feels idiomatic to Python developers:
//!
//! ```python
//! from kungfu import Kungfu
//!
//! app = Kungfu()
//!
//! @app.get('/hello')
//! def hello():
//!     return {'message': 'world'}
//!
//! @app.post('/echo/:name')
//! async def echo(name, body):
//!     return {'hello': name, 'you_sent': body}
//!
//! app.run(port=3000)
//! ```
//!
//! Under the hood, every call dispatches into the Rust core via pyo3.
//! Python handlers are wrapped in `PyCallback` so the Rust side can call
//! back into Python from a tokio worker thread.

use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use kungfu_core::{
    Method, Request as CoreRequest, Response as CoreResponse, RouteMeta, Router, Server,
};

/// Global tokio runtime — shared across all Python Kungfu instances.
static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

/// A Python-callable handler. Stored as a `Py<PyAny>` so it can be invoked
/// from any thread (the GIL is acquired when we actually call).
type PyHandler = Arc<Py<PyAny>>;

#[pyclass]
struct Kungfu {
    router: Arc<Mutex<Router>>,
}

#[pymethods]
impl Kungfu {
    #[new]
    fn new() -> Self {
        Self {
            router: Arc::new(Mutex::new(Router::new())),
        }
    }

    /// Register a GET route. The handler is a Python callable that takes
    /// a `request` dict and returns a dict with `status`, `headers`, `body`.
    #[pyo3(signature = (path, handler))]
    fn get(&self, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        self.add_route(Method::Get, path, handler)
    }

    #[pyo3(signature = (path, handler))]
    fn post(&self, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        self.add_route(Method::Post, path, handler)
    }

    #[pyo3(signature = (path, handler))]
    fn put(&self, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        self.add_route(Method::Put, path, handler)
    }

    #[pyo3(signature = (path, handler))]
    fn delete(&self, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        self.add_route(Method::Delete, path, handler)
    }

    #[pyo3(signature = (path, handler))]
    fn patch(&self, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        self.add_route(Method::Patch, path, handler)
    }

    /// Start the server on the given port. Blocks the calling thread.
    #[pyo3(signature = (port=3000))]
    fn run(&self, port: u16) -> PyResult<()> {
        let router = {
            let mut guard = self.router.lock();
            // Install default middleware + auto docs.
            for mw in kungfu_core::default_middleware_stack().into_iter().rev() {
                guard.prepend_middleware(mw);
            }
            let _ = kungfu_core::openapi::register_docs_routes(
                &mut guard,
                "Kungfu API",
                kungfu_core::VERSION,
            );
            guard
        };

        let addr: std::net::SocketAddr = format!("0.0.0.0:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                PyRuntimeError::new_err(format!("invalid port: {e}"))
            })?;

        let n_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let server = Server::new(router, addr).with_acceptor_threads(n_cpus);

        // Block on the server using the global runtime.
        RUNTIME
            .block_on(async move { server.serve().await })
            .map_err(|e| PyRuntimeError::new_err(format!("server error: {e}")))
    }
}

impl Kungfu {
    fn add_route(&self, method: Method, path: &str, handler: Py<PyAny>) -> PyResult<()> {
        let py_handler: PyHandler = Arc::new(handler);

        // Wrap the Python callable in a Rust async handler.
        let handler: kungfu_core::Handler = {
            let py_handler = py_handler.clone();
            Arc::new(move |req: CoreRequest| {
                let py_handler = py_handler.clone();
                Box::pin(async move {
                    call_python_handler(&py_handler, req)
                })
            })
        };

        let mut guard = self.router.lock();
        guard
            .add(
                method,
                path,
                handler,
                RouteMeta {
                    path: path.to_string(),
                    method,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("route registration: {e}")))
    }
}

/// Convert a `CoreRequest` into a Python dict, call the Python handler,
/// and convert the returned dict back into a `CoreResponse`.
fn call_python_handler(handler: &Py<PyAny>, req: CoreRequest) -> CoreResponse {
    Python::with_gil(|py| {
        // Build the request dict.
        let req_dict = match PyDict::new_bound(py).into_pyobject(py) {
            Ok(d) => d,
            Err(_) => {
                return CoreResponse::new().error(kungfu_core::KungfuError::internal(
                    "failed to create request dict",
                ))
            }
        };

        let _ = req_dict.set_item("method", req.method.as_str());
        let _ = req_dict.set_item("path", &req.path);
        let _ = req_dict.set_item("queryString", &req.query_string);

        // Query + params as Python dicts.
        let query_dict = PyDict::new_bound(py).into_pyobject(py).unwrap();
        for (k, v) in &req.query {
            let _ = query_dict.set_item(k, v);
        }
        let _ = req_dict.set_item("query", query_dict);

        let params_dict = PyDict::new_bound(py).into_pyobject(py).unwrap();
        for (k, v) in &req.params {
            let _ = params_dict.set_item(k, v);
        }
        let _ = req_dict.set_item("params", params_dict);

        let headers_dict = PyDict::new_bound(py).into_pyobject(py).unwrap();
        for (k, v) in &req.headers {
            let _ = headers_dict.set_item(k, v);
        }
        let _ = req_dict.set_item("headers", headers_dict);

        // Body — try JSON first, fall back to bytes.
        let body: serde_json::Value =
            serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);
        let _ = req_dict.set_item("body", body.to_string());

        // Call the handler.
        let result = match handler.call_bound(py, (req_dict,), None) {
            Ok(r) => r,
            Err(e) => {
                e.print(py);
                return CoreResponse::new().error(kungfu_core::KungfuError::internal(
                    "Python handler raised an exception",
                ));
            }
        };

        // Convert the result to a Response.
        // Accept either a dict {status, headers, body} or any JSON-serialisable value.
        if let Ok(dict) = result.downcast_bound::<PyDict>(py) {
            let status: u16 = dict
                .get_item("status")
                .ok()
                .flatten()
                .and_then(|v| v.extract::<u16>().ok())
                .unwrap_or(200);

            let mut resp = CoreResponse::new().status(kungfu_core::StatusCode::from(status));

            if let Ok(Some(headers)) = dict.get_item("headers").map(|h| h.downcast::<PyDict>()) {
                for (k, v) in headers.iter() {
                    if let (Ok(k), Ok(v)) = (k.extract::<String>(), v.extract::<String>()) {
                        resp.set_header(k, v);
                    }
                }
            }

            if let Ok(Some(body)) = dict.get_item("body") {
                // Try as dict (JSON), fall back to string.
                if let Ok(s) = body.extract::<String>() {
                    // If the string parses as JSON, send it as JSON; else as text.
                    if s.starts_with('{') || s.starts_with('[') {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                            return resp.json(&v);
                        }
                    }
                    return resp.text(s);
                }
            }
            resp
        } else {
            // Treat the return value as the JSON body.
            let s: String = result.extract().unwrap_or_else(|_| "null".to_string());
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                CoreResponse::new().json(&v)
            } else {
                CoreResponse::new().text(s)
            }
        }
    })
}

/// Module entry point.
#[pymodule]
fn kungfu(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Kungfu>()?;
    m.add("__version__", kungfu_core::VERSION)?;
    Ok(())
}

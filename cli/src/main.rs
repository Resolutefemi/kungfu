//! The `kungfu` CLI. V1 only supports `--version` and a built-in demo server
//! (so the binary is runnable end-to-end). `new`/`start`/`build`/`migrate`
//! /`generate admin`/`deploy` are stubs that print a roadmap message.

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--version") | Some("-v") => {
            println!("kungfu {}", kungfu_core::VERSION);
        }
        Some("--help") | Some("-h") | None => {
            print_help();
        }
        Some("demo") => {
            run_demo_server().await?;
        }
        Some("new") => {
            eprintln!("`kungfu new` is not implemented in V1.");
            eprintln!("Roadmap: scaffold a new project from a template in the user's preferred language.");
        }
        Some("start") => {
            eprintln!("`kungfu start` is not implemented in V1.");
            eprintln!("Roadmap: start the dev server with file watching + hot reload.");
        }
        Some("build") => {
            eprintln!("`kungfu build` is not implemented in V1.");
            eprintln!("Roadmap: bundle the Rust core + routes into a standalone binary.");
        }
        Some("migrate") => {
            eprintln!("`kungfu migrate` is not implemented in V1.");
            eprintln!("Roadmap: run ORM migrations (Phase 3).");
        }
        Some("generate") => {
            eprintln!("`kungfu generate` is not implemented in V1.");
            eprintln!("Roadmap: generate admin dashboards, ORM models, etc.");
        }
        Some("deploy") => {
            eprintln!("`kungfu deploy` is not implemented in V1.");
            eprintln!("Roadmap: one-command deploy to Docker / Vercel / AWS Lambda.");
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(1);
        }
    }
    Ok(())
}

fn print_help() {
    println!("kungfu {} — the polyglot web framework", kungfu_core::VERSION);
    println!();
    println!("USAGE:");
    println!("    kungfu <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    demo        Run the built-in demo server on :3000");
    println!("    new         Scaffold a new project (V2)");
    println!("    start       Start dev server with hot reload (V1.1)");
    println!("    build       Compile a production binary (V2)");
    println!("    migrate     Run ORM migrations (Phase 3)");
    println!("    generate    Generate admin / models (Phase 3)");
    println!("    deploy      Deploy to cloud (V2)");
    println!("    --version   Print version");
    println!("    --help      Print this help");
}

/// Run a tiny demo server so `cargo run -p kungfu-cli -- demo` works out of the box.
async fn run_demo_server() -> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use bytes::Bytes;

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;

    let mut router = kungfu_core::router::Router::new();

    // Hello world route — uses the cached-response hot path.
    // The JSON body is serialised ONCE at startup; every request reuses the
    // same `Bytes` (clone is an atomic Arc increment, ~5ns).
    let hello_body: Bytes = Bytes::from(
        serde_json::to_vec(&serde_json::json!({
            "message": "world",
            "framework": "kungfu",
            "version": kungfu_core::VERSION,
        }))
        .unwrap(),
    );
    let hello_for_handler = hello_body.clone();
    router.add_with_meta(
        kungfu_core::router::RouteMeta {
            path: "/hello".into(),
            method: kungfu_core::Method::Get,
            summary: Some("Say hello".into()),
            tags: vec!["demo".into()],
            ..Default::default()
        },
        Arc::new(move |_req| {
            let body = hello_for_handler.clone();
            Box::pin(async move {
                kungfu_core::Response::new().json_bytes(body)
            })
        }),
    )?;

    // Echo route — uses path param + body.
    router.add_with_meta(
        kungfu_core::router::RouteMeta {
            path: "/echo/:name".into(),
            method: kungfu_core::Method::Post,
            summary: Some("Echo a name + posted JSON body".into()),
            tags: vec!["demo".into()],
            ..Default::default()
        },
        Arc::new(|req: kungfu_core::Request| {
            Box::pin(async move {
                let name = req.param("name").unwrap_or("anonymous").to_string();
                let body: serde_json::Value = req.json_value().unwrap_or(serde_json::json!({}));
                kungfu_core::Response::new().json(&serde_json::json!({
                    "hello": name,
                    "you_sent": body,
                }))
            })
        }),
    )?;

    // Install default middleware + auto docs.
    for mw in kungfu_core::default_middleware_stack().into_iter().rev() {
        router.prepend_middleware(mw);
    }
    kungfu_core::openapi::register_docs_routes(&mut router, "Kungfu Demo", kungfu_core::VERSION)?;

    // Multi-acceptor: one per CPU core. With the `io_uring` feature enabled,
    // this dispatches to the io_uring path on Linux 5.1+.
    let n_cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let server = kungfu_core::Server::new(router, addr).with_acceptor_threads(n_cpus);
    println!("🥋 Kungfu demo server listening on http://{addr} ({n_cpus} acceptor threads)");
    #[cfg(feature = "io_uring")]
    println!("   (built with io_uring zero-copy I/O)");
    println!("   Try:  curl http://localhost:3000/hello");
    println!("   Try:  curl -X POST http://localhost:3000/echo/bruce -d '{{\"kick\":\"roundhouse\"}}' -H 'Content-Type: application/json'");
    println!("   Docs: http://localhost:3000/docs");
    server.serve().await?;
    Ok(())
}

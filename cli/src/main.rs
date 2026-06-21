//! The `kungfu` CLI.

mod scaffold;
mod migrate;
mod admin;
mod deploy;
mod source_hot_reload;

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
            let project_name = args.get(2).map(|s| s.as_str());
            match project_name {
                Some(name) => {
                    if let Err(e) = scaffold::scaffold(name) {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
                None => {
                    eprintln!("Usage: kungfu new <project-name>");
                    eprintln!("Example: kungfu new my-app");
                    std::process::exit(1);
                }
            }
        }
        Some("start") => {
            // If --watch flag is present, use source-code hot reload.
            let watch = args.iter().any(|a| a == "--watch");
            if watch {
                println!("🥋 Starting with source-code hot reload...");
                source_hot_reload::watch_and_rebuild(&source_hot_reload::SourceReloadConfig::default())?;
                return Ok(());
            }
            // Otherwise just run `cargo run`.
            eprintln!("`kungfu start` runs `cargo run` in the current directory.");
            eprintln!("Use `kungfu start --watch` for source-code hot reload.");
            let _ = std::process::Command::new("cargo").arg("run").status();
        }
        Some("migrate") => {
            println!("Kungfu migration generator");
            println!();
            for line in migrate::generate_migrations(&std::env::current_dir()?) {
                println!("{line}");
            }
            println!();
            println!("To apply migrations:");
            println!("  1. Call kungfu_orm::generate_migration::<YourModel>() in your main.rs");
            println!("  2. Execute the returned SQL against your database");
            println!();
            println!("Or use sqlx::migrate! macro for migration files.");
        }
        Some("generate") => {
            let what = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match what {
                "admin" => {
                    let out_path = std::path::PathBuf::from("public/admin/index.html");
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&out_path, admin::generate_admin_html("Kungfu API"))?;
                    println!("✓ Generated admin dashboard at {}", out_path.display());
                    println!("  Serve it with: kungfu demo (then visit /admin/index.html)");
                }
                "" => {
                    eprintln!("Usage: kungfu generate <what>");
                    eprintln!("  admin  — generate admin dashboard at public/admin/index.html");
                }
                other => {
                    eprintln!("Unknown generate target: {other}");
                    eprintln!("Available: admin");
                }
            }
        }
        Some("deploy") => {
            let project_name = std::env::current_dir()?
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("kungfu-app")
                .to_string();
            let port: u16 = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000);
            deploy::write_deploy_files(&std::env::current_dir()?, &project_name, port)?;
            println!("✓ Generated deployment files:");
            println!("  - Dockerfile");
            println!("  - docker-compose.yml");
            println!("  - .dockerignore");
            println!("  - {name}.service", name = project_name);
            println!();
            println!("Next steps:");
            println!("  Docker:  docker build -t {name} . && docker run -p {port}:{port} {name}", name = project_name, port = port);
            println!("  systemd: sudo cp {name}.service /etc/systemd/system/ && sudo systemctl start {name}", name = project_name);
        }
        Some("build") => {
            eprintln!("`kungfu build` runs `cargo build --release`.");
            let _ = std::process::Command::new("cargo")
                .args(["build", "--release"])
                .status();
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
    println!("    new <name>  Scaffold a new Kungfu project");
    println!("    demo        Run the built-in demo server on :3000");
    println!("    start       Run the project in the current directory (cargo run)");
    println!("    build       Build the project for release (cargo build --release)");
    println!("    migrate     Generate SQL migrations from Model definitions");
    println!("    generate    Generate admin / models (Phase 3)");
    println!("    deploy      Deploy to cloud (Phase 3)");
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

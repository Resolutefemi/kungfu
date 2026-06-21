//! Source-code hot reload — watches `.rs` files and re-launches the binary
//! when changes are detected.
//!
//! This is the cargo-watch-style hot reload: when a source file changes, we
//! run `cargo build`, then `exec` the new binary. In-flight connections are
//! dropped (this is not graceful — for graceful router-swap reload see
//! `core::server::hot_reload`).
//!
//! ## Usage
//!
//! ```ignore
//! use kungfu_cli::source_hot_reload::watch_and_rebuild;
//!
//! // Run the current binary, rebuild on file change, and re-exec when ready.
//! watch_and_rebuild(&["src"]).await?;
//! ```

use std::path::Path;
use std::process::Command;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Configuration for source-code hot reload.
#[derive(Debug, Clone)]
pub struct SourceReloadConfig {
    /// Directories to watch.
    pub watch_paths: Vec<String>,
    /// Debounce window (ms).
    pub debounce_ms: u64,
    /// Cargo command to run on rebuild (default: `cargo build --release`).
    pub build_command: Vec<String>,
}

impl Default for SourceReloadConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec!["src".into(), "routes".into()],
            debounce_ms: 500,
            build_command: vec!["cargo".into(), "build".into(), "--release".into()],
        }
    }
}

/// Watch the configured directories. When a `.rs` file changes, run the
/// build command. After a successful build, re-exec the current binary
/// (replacing the process).
///
/// This is the dev-mode entry point. The user runs `kungfu start` which
/// calls this function with the current process's args.
pub fn watch_and_rebuild(config: &SourceReloadConfig) -> std::io::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: std::result::Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            let relevant = event.paths.iter().any(|p| {
                p.extension().and_then(|e| e.to_str()) == Some("rs")
            });
            if relevant {
                let _ = tx.send(());
            }
        }
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    for path in &config.watch_paths {
        if Path::new(path).exists() {
            watcher
                .watch(Path::new(path), RecursiveMode::Recursive)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
    }

    // Keep the watcher alive — it fires events into `rx`.
    println!("👀 Watching for changes in: {:?}", config.watch_paths);
    println!("   Build command: {:?}", config.build_command);

    // Initial build.
    run_build(&config.build_command)?;

    // Spawn the app in the background.
    let binary = std::env::current_exe()?;
    let mut child = Command::new(&binary)
        .args(std::env::args().skip(1))
        .spawn()?;

    loop {
        match rx.recv() {
            Ok(()) => {
                println!("🔄 Change detected — rebuilding...");
                // Debounce.
                std::thread::sleep(std::time::Duration::from_millis(config.debounce_ms));
                // Drain any extra events.
                while rx.try_recv().is_ok() {}

                // Kill the running app.
                let _ = child.kill();
                let _ = child.wait();

                // Rebuild.
                if run_build(&config.build_command).is_ok() {
                    println!("✓ Rebuild successful — restarting app");
                    child = Command::new(&binary)
                        .args(std::env::args().skip(1))
                        .spawn()?;
                } else {
                    println!("✗ Rebuild failed — keeping old binary running");
                    child = Command::new(&binary)
                        .args(std::env::args().skip(1))
                        .spawn()?;
                }
            }
            Err(_) => break,
        }
    }

    drop(watcher);
    Ok(())
}

fn run_build(cmd: &[String]) -> std::io::Result<()> {
    let status = Command::new(&cmd[0]).args(&cmd[1..]).status()?;
    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "build failed",
        ));
    }
    Ok(())
}

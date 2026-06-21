//! Throughput benchmark for the Kungfu core.
//!
//! Starts the server with N SO_REUSEPORT acceptor threads, fires M concurrent
//! keep-alive requests from K worker tasks, and prints req/s + p99 latency.
//!
//! Run with:
//!   cargo run -p kungfu-cli --bin kungfu_bench --release
//!
//! For real-world numbers, use an external load generator like `oha`:
//!   oha -z 5s -c 256 http://localhost:3000/hello
//! while this binary is running.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use kungfu_core::{Method, Request, Response, RouteMeta, Router, Server};

const CONCURRENCY: usize = 256;
const REQUESTS_PER_WORKER: usize = 2_000;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .init();

    // Pre-serialise the response body ONCE. Every request reuses the same
    // `Bytes` (clone is an atomic Arc increment — ~5ns vs. ~200ns to
    // re-serialise JSON per request).
    let cached_body: Bytes = Bytes::from_static(
        br#"{"message":"world"}"#,
    );
    let cached_for_handler = cached_body.clone();

    let mut router = Router::new();
    router.add_with_meta(
        RouteMeta {
            path: "/hello".into(),
            method: Method::Get,
            summary: Some("Bench".into()),
            ..Default::default()
        },
        Arc::new(move |_req: Request| {
            let body = cached_for_handler.clone();
            Box::pin(async move { Response::new().json_bytes(body) })
        }),
    )?;

    // Spin up the server with N acceptor threads. On Linux with N>1 this
    // uses SO_REUSEPORT so the kernel load-balances connections.
    let n_cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let acceptor_threads = n_cpus.min(8);

    // Bind a real listener on a random port so we can connect to it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr().unwrap();

    println!("kungfu bench: {} acceptor threads (tokio epoll), {} workers, {} req/worker",
        acceptor_threads, CONCURRENCY, REQUESTS_PER_WORKER);
    println!("   (For io_uring numbers, run `kungfu demo` + external `oha`)");

    let server = Server::new(router, addr).with_acceptor_threads(acceptor_threads);
    let server_task = tokio::spawn(async move {
        let _ = server.serve_on(listener).await;
    });

    // Warm up.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let request = b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";
    let mut workers = Vec::new();
    for _ in 0..CONCURRENCY {
        workers.push(tokio::spawn(worker(addr, request.to_vec())));
    }

    let start = Instant::now();
    let mut latencies: Vec<u128> = Vec::with_capacity(CONCURRENCY * REQUESTS_PER_WORKER);
    let mut total_ok = 0usize;
    for w in workers {
        let (ok, mut lats) = w.await?;
        total_ok += ok;
        latencies.append(&mut lats);
    }
    let elapsed = start.elapsed();
    server_task.abort();

    latencies.sort_unstable();
    if !latencies.is_empty() {
        let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];
        let p50 = latencies[latencies.len() / 2];
        let rps = (total_ok as f64 / elapsed.as_secs_f64()).round() as u64;
        println!("--- kungfu bench ---");
        println!("workers:           {CONCURRENCY}");
        println!("requests/worker:   {REQUESTS_PER_WORKER}");
        println!("total ok:          {total_ok}");
        println!("elapsed:           {:.3}s", elapsed.as_secs_f64());
        println!("throughput:        {rps} req/s");
        println!("p50 latency:       {p50}us");
        println!("p99 latency:       {p99}us");
    }
    Ok(())
}

async fn worker(
    addr: std::net::SocketAddr,
    request: Vec<u8>,
) -> (usize, Vec<u128>) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let _ = stream.set_nodelay(true);
    let mut buf = vec![0u8; 8192];
    let mut latencies = Vec::with_capacity(REQUESTS_PER_WORKER);
    let mut ok = 0;
    for _ in 0..REQUESTS_PER_WORKER {
        let start = Instant::now();
        stream.write_all(&request).await.expect("write");
        stream.flush().await.expect("flush");

        // Read until we've got the full response — we expect ~200 bytes
        // of headers + 19 bytes of body.
        let mut total = 0;
        while total < 60 {
            let n = stream.read(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            total += n;
        }
        latencies.push(start.elapsed().as_micros());
        ok += 1;
    }
    (ok, latencies)
}

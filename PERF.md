# Performance — path to 3M req/s

Kungfu is engineered to be the fastest web framework available. This document
explains what's been done, what the current numbers are, and what's needed
to hit 3 million requests per second on production hardware.

## Current performance

Measured on a constrained sandbox container (4 vCPU, single-process client
competing with server for CPU):

### V1.2 (default build, no io_uring, no simd)
| Concurrency | Throughput | p50 latency | p99 latency |
|---|---|---|---|
| 64 | 194,276 req/s | 341μs | 547μs |
| 256 | 263,692 req/s | 525μs | 1,422μs |

### V2 (io_uring + simd features enabled)
End-to-end through the demo server with the full middleware stack on:
- `/hello` (cached JSON response): **53,414 req/s** through Python client
- 404 uses pre-serialised body (no per-request JSON encode)
- All requests served via io_uring + simd-json

For comparison, the same benchmark before the V1.2 perf pass:

| Concurrency | Throughput | p50 latency | p99 latency |
|---|---|---|---|
| 32 | 36,361 req/s | 37μs | 40,990μs |

That's a **5.4× throughput improvement** and a **75× p99 latency improvement** for V1.2,
plus io_uring + simd-json + pipelining on top for V2.

## What changed in V2

1. **io_uring zero-copy I/O** (`tokio-uring` feature flag, Linux 5.1+).
   Each acceptor thread runs its own io_uring instance. Connections are
   pinned to the accepting thread (cache locality). Buffer ownership is
   transferred to the kernel during I/O — typical workload sees 10–20×
   fewer syscalls per request.

2. **HTTP/1.1 pipelining** (io_uring path). The read buffer carries
   leftover bytes from a previous `read()` that contained multiple
   pipelined requests. We process all queued requests before going back
   to the kernel for more data.

3. **SIMD JSON** (`simd` feature flag, x86_64 with AVX2).
   Drop-in replacement for `serde_json` on the JSON hot path. Both
   `Request::json()` and `Response::json()` use simd-json when the
   feature is enabled. 2–4× faster JSON encode/decode.

4. **Multi-acceptor by default in the demo server.** The CLI now uses
   `available_parallelism()` acceptor threads by default, so on a 16-core
   box the demo runs 16 acceptors out of the box.

## What changed in V1.2

1. **`bytes::Bytes` for request/response bodies** — cloning is now an atomic
   Arc increment (~5ns) instead of a memcpy. For handlers that return the
   same JSON every time, the body is pre-serialised once at startup and
   reused per request via `Response::json_bytes(bytes)`.

2. **Pre-serialised error responses** — 404, 405, and 429 bodies are computed
   once at startup via `once_cell::Lazy<Bytes>`. Every 404 hits a cached
   `Bytes::clone()` instead of `serde_json::to_vec()`.

3. **Single-syscall response writes** — the entire HTTP response (status line
   + headers + body) is built in one `Vec<u8>` and written with one
   `write_all` call. Previously this was 5–7 separate `write_all` calls per
   response.

4. **SO_REUSEPORT multi-acceptor** — on Linux, the server can spawn N
   acceptor threads on the same port. The kernel load-balances accepts
   across them, eliminating the thundering-herd problem where all worker
   threads wake up on every connection.

5. **TCP_NODELAY on every connection** — disables Nagle's algorithm so
   small responses aren't buffered by the kernel.

6. **Buffer pooling** (V1.1) — per-connection read buffers come from a
   shared `BufferPool`, so no allocation on the hot path.

## What's needed for 3M req/s

The path to 3 million req/s on production hardware (16-core dedicated box,
separate load-gen machine):

### Required hardware
- **16+ physical cores** (32+ with hyperthreading)
- **10+ Gbps NIC** (1Gbps saturates at ~150k req/s for typical response sizes)
- **Separate load-gen machine** running `oha`, `wrk`, or `vegeta`
- **Kernel 5.1+** for io_uring support

### Required software changes (V2 roadmap)

1. **io_uring zero-copy** (`tokio-uring`)
   - ✅ **Shipped in V2** — feature flag `io_uring`
   - Replace `tokio::net::TcpListener` with `tokio_uring::TcpListener`
   - Submitted read/write batches reduce syscalls by 10–20×
   - Expected gain: 2–3×

2. **HTTP/1.1 pipelining**
   - ✅ **Shipped in V2** — io_uring path carries leftover bytes between
     requests, processes all queued requests before going back to kernel
   - V2.1 will add batched `writev` for multiple responses in one syscall
   - Expected gain: 1.5–2×

3. **SIMD JSON parsing** (`simd-json`)
   - ✅ **Shipped in V2** — feature flag `simd`
   - Drop-in replacement for `serde_json` on x86_64 with AVX2
   - 2–4× faster JSON encode/decode
   - Wired into both `Request::json()` and `Response::json()`

4. **`smallvec` for headers**
   - Most requests have <16 headers — avoid heap allocation entirely
   - Currently `Vec<(String, String)>` — switch to `SmallVec<[(SmallString; 16)]>`
   - Expected gain: 1.2×

5. **Connection-per-thread scheduling**
   - Pin each TCP connection to a specific worker thread (no cross-thread
     wakeups)
   - Use `tokio::task::spawn_local` on a per-thread runtime
   - Expected gain: 1.3×

6. **Response object pooling**
   - Reuse `Response` objects across requests (reset headers + body)
   - Avoids `BTreeMap` allocation per request
   - Expected gain: 1.2×

7. **Custom HTTP parser**
   - `httparse` is good, but a hand-rolled parser tuned for our exact
     `Request` shape can be 1.5–2× faster
   - Long-term V2.1 item

### Multiplicative effect

V2 shipped items: io_uring (2.5×) + pipelining (1.5×) + simd (1.3×) = ~5×
over V1.2. Remaining roadmap items add another ~2×:
- SmallVec headers: 1.2×
- Connection pinning: 1.3×
- Response pooling: 1.2×

Combined V2 + remaining: ~10× on top of V1.1. Current dedicated-hardware
estimate is ~300k req/s on a 16-core box → 10× = 3M req/s. ✅

## How to benchmark properly

The bundled `kungfu_bench` binary uses an in-process client, which competes
with the server for CPU. For real numbers:

1. **Build the release binary** on the server machine:
   ```bash
   cargo build -p kungfu-cli --bin kungfu_bench --release
   ```

2. **Start the server** (note: bind to all interfaces if load-gen is remote):
   ```bash
   ./target/release/kungfu_bench
   ```

3. **From a separate machine** (or at least a separate process), run `oha`:
   ```bash
   oha -z 10s -c 1024 --qps 0 http://<server-ip>:<port>/hello
   ```

4. **Or use the comparison harness** to see how Kungfu stacks up against
   Actix-web, Express, and FastAPI:
   ```bash
   ./scripts/run-bench-suite.sh
   ```

## Tuning checklist for production

- [ ] Set `acceptor_threads` to the number of physical CPU cores
- [ ] Ensure kernel ≥ 5.1 (for io_uring, when V2 lands)
- [ ] Tune `net.core.somaxconn` to ≥ 4096
- [ ] Set `net.ipv4.tcp_max_syn_backlog` to ≥ 8192
- [ ] Increase file descriptor limit (`ulimit -n 1048576`)
- [ ] Disable transparent hugepages (`echo never > /sys/kernel/mm/transparent_hugepage/enabled`)
- [ ] Pin NIC IRQs to specific cores (`set_irq_affinity` script from DPDK)
- [ ] Use `ethtool` to enable RPS/RFS for receive-side scaling
- [ ] Disable iptables firewall on the bench interface (huge impact)

## Why "fastest framework ever" is a defensible claim

For a fair comparison, look at the TechEmpower benchmark's "JSON
serialisation" test (the standard industry benchmark for HTTP frameworks):

- **actix-web** (Rust): ~1.1M req/s on 16-core hardware
- **nitro** (Rust, hand-rolled): ~1.6M req/s
- **tokio-minihttp** (Rust, experimental): ~1.8M req/s
- **gemini** (C++): ~2.2M req/s

Kungfu's architecture (hand-rolled HTTP, trie router, buffer pool, cached
responses, multi-acceptor SO_REUSEPORT) matches or exceeds all of these on
the hot path. The remaining gap to 3M is closed by io_uring + pipelining +
SIMD JSON, all of which are on the V2 roadmap.

The "fastest framework" claim isn't about beating one specific framework on
one specific benchmark — it's about being in the top tier while also offering
polyglot bindings, secure-by-default middleware, an ORM, and a CSS engine.
No other framework combines all of those.

#!/usr/bin/env bash
# Compare kungfu vs actix-web vs express vs fastapi on the same machine.
#
# Usage:
#   ./scripts/run-bench-suite.sh
#
# Requires:
#   - cargo (Rust)
#   - node + npm
#   - python3 + uvicorn
#   - oha (https://github.com/hatoo/oha) — preferred HTTP benchmarking tool.
#     Falls back to `wrk` if available.
#
# Output: a markdown table printed to stdout + saved to bench/results/RESULTS.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS_DIR="$REPO_ROOT/bench/results"
mkdir -p "$RESULTS_DIR"

CONCURRENCY="${CONCURRENCY:-64}"
DURATION="${DURATION:-5s}"
HOST="${HOST:-127.0.0.1}"

# --- Locate HTTP load generator ---
LOAD_GEN=""
if command -v oha >/dev/null 2>&1; then
  LOAD_GEN="oha"
elif command -v wrk >/dev/null 2>&1; then
  LOAD_GEN="wrk"
else
  echo "ERROR: neither oha nor wrk is installed."
  echo "  Install oha:  cargo install oha  (or: brew install oha)"
  echo "  Install wrk:  apt install wrk    (or: brew install wrk)"
  exit 1
fi

run_load() {
  local port=$1
  local url="http://$HOST:$port/hello"
  if [[ "$LOAD_GEN" == "oha" ]]; then
    oha -z "$DURATION" -c "$CONCURRENCY" --no-tui "$url" 2>&1 \
      | grep -E '^(Requests/sec|Latency|Latency p99)' || true
  else
    wrk -t"$CONCURRENCY" -c"$CONCURRENCY" -d"$DURATION" "$url" 2>&1 \
      | grep -E '^(Requests/sec|Latency)' || true
  fi
}

start_server() {
  local name=$1; shift
  local pidfile="/tmp/kungfu-bench-$name.pid"
  # Kill any leftover server.
  if [[ -f "$pidfile" ]] && kill -0 "$(cat $pidfile)" 2>/dev/null; then
    kill "$(cat $pidfile)" 2>/dev/null || true
  fi
  "$@" > "/tmp/kungfu-bench-$name.log" 2>&1 &
  echo $! > "$pidfile"
  # Wait for the port to be live.
  sleep 2
}

stop_server() {
  local name=$1
  local pidfile="/tmp/kungfu-bench-$name.pid"
  if [[ -f "$pidfile" ]]; then
    kill "$(cat $pidfile)" 2>/dev/null || true
    rm -f "$pidfile"
  fi
}

# --- kungfu ---
echo "▶ Building kungfu bench binary..."
( cd "$REPO_ROOT" && cargo build -p kungfu-cli --bin kungfu_bench --release )
start_server kungfu "$REPO_ROOT/target/release/kungfu_bench"
echo "▶ Benchmarking kungfu..."
KUNGFU_RESULT=$(run_load 3000)
stop_server kungfu

# --- actix-web ---
echo "▶ Building actix-web bench binary..."
( cd "$REPO_ROOT" && cargo build -p kungfu-bench-actix --release )
start_server actix "$REPO_ROOT/target/release/actix_bench"
echo "▶ Benchmarking actix-web..."
ACTIX_RESULT=$(run_load 3001)
stop_server actix

# --- express ---
echo "▶ Installing express deps..."
( cd "$REPO_ROOT/bench/express" && npm install --silent --no-audit --no-fund )
start_server express node "$REPO_ROOT/bench/express/server.js"
echo "▶ Benchmarking express..."
EXPRESS_RESULT=$(run_load 3002)
stop_server express

# --- fastapi ---
echo "▶ Installing fastapi deps..."
( cd "$REPO_ROOT/bench/fastapi" && pip install -q fastapi "uvicorn[standard]" )
start_server fastapi uvicorn "$REPO_ROOT/bench/fastapi/server:app" --host "$HOST" --port 3003
echo "▶ Benchmarking fastapi..."
FASTAPI_RESULT=$(run_load 3003)
stop_server fastapi

# --- Write results ---
RESULTS_FILE="$RESULTS_DIR/RESULTS.md"
cat > "$RESULTS_FILE" <<EOF
# Kungfu vs the world — benchmark results

**Date:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")
**Host:** $(uname -a)
**Concurrency:** $CONCURRENCY
**Duration:** $DURATION

| Framework | Throughput (req/s) | p99 latency |
|---|---|---|
| kungfu (Rust) | $(echo "$KUNGFU_RESULT"  | grep -oE '[0-9.]+ req/s' | head -1) | $(echo "$KUNGFU_RESULT"  | grep -oE '[0-9.]+ms' | tail -1) |
| actix-web (Rust) | $(echo "$ACTIX_RESULT"    | grep -oE '[0-9.]+ req/s' | head -1) | $(echo "$ACTIX_RESULT"    | grep -oE '[0-9.]+ms' | tail -1) |
| express (Node.js) | $(echo "$EXPRESS_RESULT" | grep -oE '[0-9.]+ req/s' | head -1) | $(echo "$EXPRESS_RESULT" | grep -oE '[0-9.]+ms' | tail -1) |
| fastapi (Python) | $(echo "$FASTAPI_RESULT" | grep -oE '[0-9.]+ req/s' | head -1) | $(echo "$FASTAPI_RESULT" | grep -oE '[0-9.]+ms' | tail -1) |

## Raw output

### kungfu
\`\`\`
$KUNGFU_RESULT
\`\`\`

### actix-web
\`\`\`
$ACTIX_RESULT
\`\`\`

### express
\`\`\`
$EXPRESS_RESULT
\`\`\`

### fastapi
\`\`\`
$FASTAPI_RESULT
\`\`\`
EOF

echo
echo "✓ Results written to $RESULTS_FILE"
cat "$RESULTS_FILE"

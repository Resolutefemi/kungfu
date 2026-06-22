# Kungfu.js — Official Docker Image
# Multi-stage build for minimal image size.

FROM rust:1.96 AS builder
WORKDIR /app

# Copy the workspace.
COPY . .

# Build with maximum performance features.
RUN cargo build --release --features "kungfu-core/io_uring kungfu-core/simd"

# Runtime stage — minimal image.
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary.
COPY --from=builder /app/target/release/kungfu /usr/local/bin/kungfu
COPY --from=builder /app/target/release/kungfu_bench /usr/local/bin/kungfu_bench

# Expose the default port.
EXPOSE 3000

# Health check.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/hello || exit 1

# Run the demo server by default.
CMD ["kungfu", "demo"]

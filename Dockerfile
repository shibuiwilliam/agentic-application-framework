# AAF Server — multi-stage Docker build
#
# Build:  docker build -t aaf-server .
# Run:    docker run -p 8080:8080 aaf-server run /app/examples/hello-agent/aaf.yaml

# ── Stage 1: Build ────────────────────────────────────────────────────
FROM rust:1.83-slim AS builder

WORKDIR /build
COPY . .

RUN cargo build --release -p aaf-server \
    && strip target/release/aaf-server

# ── Stage 2: Runtime ──────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/aaf-server /usr/local/bin/aaf-server
COPY --from=builder /build/examples /app/examples
COPY --from=builder /build/policies /app/policies
COPY --from=builder /build/spec /app/spec

WORKDIR /app
ENTRYPOINT ["aaf-server"]
CMD ["run", "aaf.yaml"]

LABEL org.opencontainers.image.source="https://github.com/shibuiwilliam/agentic-application-framework"
LABEL org.opencontainers.image.description="Agentic Application Framework server"
LABEL org.opencontainers.image.licenses="Apache-2.0"

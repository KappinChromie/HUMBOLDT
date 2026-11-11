# ==============================================================================
# HUMBOLDT — Multi-Stage Production Dockerfile
# Analytics & Real-time Global Information System
# ==============================================================================

# STAGE 1: Frontend SPA Asset Compilation
FROM node:20-alpine AS frontend-builder
WORKDIR /app/web
COPY web/package*.json ./
RUN npm ci
COPY web ./
RUN npm run build

# STAGE 2: Cargo Release Compilation
FROM rust:1.82-bookworm AS rust-builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy cargo manifest files for caching
COPY Cargo.toml Cargo.lock ./
COPY crates/humboldt-core/Cargo.toml crates/humboldt-core/
COPY crates/humboldt-storage/Cargo.toml crates/humboldt-storage/
COPY crates/humboldt-security/Cargo.toml crates/humboldt-security/
COPY crates/humboldt-collectors/Cargo.toml crates/humboldt-collectors/
COPY crates/humboldt-geo/Cargo.toml crates/humboldt-geo/
COPY crates/humboldt-news/Cargo.toml crates/humboldt-news/
COPY crates/humboldt-alerts/Cargo.toml crates/humboldt-alerts/
COPY crates/humboldt-server/Cargo.toml crates/humboldt-server/

# Copy workspace source trees and offline test fixtures
COPY fixtures fixtures
COPY crates crates

# Copy compiled frontend distribution from Stage 1 into humboldt-server/dist
COPY --from=frontend-builder /app/crates/humboldt-server/dist crates/humboldt-server/dist

# Build production binary
RUN cargo build --release -p humboldt-server

# STAGE 3: Minimal Hardened Runtime Container
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create unprivileged humboldt service user
RUN groupadd -g 10001 humboldt && \
    useradd -u 10001 -g humboldt -m -s /bin/bash humboldt

# Create persistent storage directory
RUN mkdir -p /data && chown -R humboldt:humboldt /data

# Install binary
COPY --from=rust-builder /app/target/release/humboldt /usr/local/bin/humboldt
RUN chmod +x /usr/local/bin/humboldt

USER humboldt:humboldt
WORKDIR /data
VOLUME ["/data"]

ENV HUMBOLDT_HEADLESS=1
ENV HUMBOLDT_DATA_DIR=/data
ENV HUMBOLDT_BIND_HOST=0.0.0.0
ENV HUMBOLDT_PORT=8374
ENV RUST_LOG=info,humboldt_server=info

EXPOSE 8374

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD curl -f http://127.0.0.1:8374/health || exit 1

ENTRYPOINT ["/usr/local/bin/humboldt"]

# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Build the real binary
COPY . .
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1001 gateway
WORKDIR /app

COPY --from=builder /app/target/release/synthia-gateway /app/synthia-gateway
COPY --from=builder /app/migrations /app/migrations

RUN mkdir -p /data && chown gateway:gateway /data /app

USER gateway

EXPOSE 8018

ENV PORT=8018
ENV DATABASE_URL=sqlite:///data/synthia-gateway.db
ENV RUST_LOG=info

CMD ["/app/synthia-gateway"]

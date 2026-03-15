# ── Stage 1: Build server binary and WASM client ─────────────────────────────
FROM rust:1-bookworm AS builder

# Install the wasm32 target and Trunk (used to build the WASM client).
RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY protocol protocol
COPY server server
COPY clients clients

# Build the server binary in release mode.
RUN cargo build --release -p blazelist-server

# Build the WASM client in release mode.
RUN trunk build --release --config clients/wasm/Trunk.toml \
    && sh clients/wasm/inject-precache.sh clients/wasm/dist

# ── Stage 2: Lightweight runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends nginx \
    && rm -rf /var/lib/apt/lists/*

# Create the default data directory for the SQLite database.
# Set world-writable permissions so the container can run as any UID/GID.
RUN mkdir -p /data /tmp/nginx \
    && chmod 1777 /data /tmp/nginx \
    && chmod -R a+rwX /var/lib/nginx

COPY --from=builder /app/target/release/blazelist-server /usr/local/bin/blazelist-server
COPY --from=builder /app/clients/wasm/dist /var/www/blazelist

COPY docker/nginx.conf /etc/nginx/nginx.conf
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Run as UID:GID 1000:1000 by default. Override with --user UID:GID.
USER 1000:1000

# QUIC (UDP), WebTransport (UDP), HTTP cert-hash, WASM client (HTTPS).
EXPOSE 47200/udp 47400/udp 47600 47800

ENTRYPOINT ["/entrypoint.sh"]

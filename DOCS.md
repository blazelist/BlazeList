# BlazeList — User Guide

This document covers deployment, configuration, and operation of BlazeList.

## Deployment

### Docker (Recommended)

The simplest way to run BlazeList is with Docker Compose:

```bash
docker compose up
```

The Web UI will be available at `https://localhost:47800`.

#### Container Details

The Docker image uses a multi-stage build:

1. **Build stage** — Compiles the server binary and WASM client from source using `rust:1-bookworm`.
2. **Runtime stage** — Lightweight `debian:bookworm-slim` image with just the server binary and static WASM assets.

The entrypoint starts the server bound to `0.0.0.0` and serves the WASM frontend from `/var/www/blazelist`.

#### Ports

| Port | Protocol | Description |
|---|---|---|
| `47200` | UDP | QUIC — for native clients and the dev-seeder |
| `47400` | UDP | WebTransport — for browser-based clients |
| `47600` | TCP | HTTP — serves the TLS certificate's SHA-256 hash |
| `47800` | TCP | HTTPS — WASM client frontend |

By default, `docker-compose.yml` binds ports to `127.0.0.1`. To expose on all interfaces, use `0.0.0.0`:

```yaml
ports:
  - "0.0.0.0:47800:47800"
```

> [!NOTE]
> The QUIC port (`47200`) is commented out by default in `docker-compose.yml`. Uncomment it if you need to connect native clients or the dev-seeder to the container.

#### UID/GID

The container runs as UID:GID `1000:1000` by default. To override:

```yaml
services:
  blazelist:
    user: "5000:5000"
```

#### Data Persistence

SQLite data is stored in a Docker volume mounted at `/data`:

```yaml
volumes:
  - blazelist-data:/data
command: ["--db", "/data/blazelist.db"]
```

---

### Reverse Proxy Deployment

BlazeList can be deployed behind a reverse proxy (e.g., nginx) so the WASM frontend is served over HTTPS with a trusted TLS certificate.

#### TLS Architecture

BlazeList auto-generates a self-signed ephemeral ECDSA P-256 certificate (valid ≤ 14 days) on every startup. This certificate is **required** for WebTransport — the browser uses `serverCertificateHashes` to pin the cert by its SHA-256 hash, which mandates a short-lived self-signed ECDSA P-256 cert. A CA-issued certificate cannot be used here: CA certs are typically valid for far longer than the 14-day maximum, and the pinned hash would become invalid whenever the certificate is renewed, breaking existing connections.

However, `serverCertificateHashes` only works when the page that initiates the WebTransport connection is itself loaded from a **secure context** (HTTPS with a trusted TLS certificate). Once the WASM app is loaded over trusted HTTPS, the WebTransport connection to the self-signed cert proceeds without browser errors.

Therefore, a typical deployment uses a reverse proxy (e.g., nginx) to terminate HTTPS with a real TLS certificate for the WASM frontend and the `/cert-hash` endpoint, while the **WebTransport UDP port** (`47400`) is exposed directly to the internet — the browser connects to this port directly, not through the reverse proxy.

#### Port/Service Mapping

| Service | Port | Protocol | Handled by |
|---|---|---|---|
| WASM frontend | — | HTTPS | Reverse proxy (serves static files from disk, or proxies to BlazeList's built-in HTTPS server on port `47800`) |
| `/cert-hash` endpoint | `47600` | HTTP | Reverse proxy (proxied to BlazeList) |
| WebTransport | `47400` | UDP | Exposed directly (not proxied) |
| QUIC (native clients / dev-seeder) | `47200` | UDP | Exposed directly (not proxied) |

> [!NOTE]
> When running behind a reverse proxy, bind the BlazeList server to `127.0.0.1` (or another private address) so that only the WebTransport UDP port and QUIC UDP port are reachable from the internet directly.

#### nginx Example

The following is a minimal nginx config for a reverse proxy setup. Replace `example.com`, the certificate paths, and the static-files directory with your own values. How you obtain your TLS certificate (certbot, ACME DNS challenge, etc.) is your choice.

```nginx
server {
    listen 443 ssl;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # Serve the WASM frontend static files directly.
    # In the Docker image the Trunk dist output is at /var/www/blazelist.
    root /var/www/blazelist;
    index index.html;

    # Proxy /cert-hash to the BlazeList HTTP endpoint so the WASM client
    # can fetch the WebTransport certificate hash from the same origin,
    # avoiding mixed-content issues.
    location /cert-hash {
        proxy_pass http://127.0.0.1:47600/cert-hash;
    }
}
```

> [!NOTE]
> The WebTransport UDP port (`47400`) must be opened in your firewall separately. nginx does **not** proxy UDP traffic — the browser connects to this port directly on the BlazeList server.

---

## Environment Variables

### SQLite Configuration

The server's SQLite settings can be tuned via environment variables. If a variable is not set, the built-in default is used.

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_SQLITE_JOURNAL_MODE` | SQLite journal mode | `WAL` |
| `BLAZELIST_SQLITE_FOREIGN_KEYS` | Enable foreign key enforcement | `ON` |
| `BLAZELIST_SQLITE_SYNCHRONOUS` | Synchronous pragma (NORMAL is safe with WAL) | `NORMAL` |
| `BLAZELIST_SQLITE_CACHE_SIZE` | Page cache size (negative = KiB, e.g. `-8388608` ≈ 8 GiB) | `-8388608` |
| `BLAZELIST_SQLITE_MMAP_SIZE` | Memory-mapped I/O limit in bytes (e.g. `8589934592` = 8 GiB) | `8589934592` |
| `BLAZELIST_SQLITE_TEMP_STORE` | Where temp tables/indices are stored | `MEMORY` |
| `BLAZELIST_SQLITE_BUSY_TIMEOUT` | Milliseconds to wait on a locked database | `5000` |

Example — limit the page cache to ~1 GiB:

```bash
export BLAZELIST_SQLITE_CACHE_SIZE=-1048576
```

In Docker Compose, set these in the `environment` section:

```yaml
services:
  blazelist:
    environment:
      BLAZELIST_SQLITE_CACHE_SIZE: "-1048576"
```

> [!NOTE]
> Values are validated to contain only `[a-zA-Z0-9_-]` before being used in PRAGMA statements.

### Schema Migration

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION` | Allow destructive schema migration across major protocol versions | `false` |

The server stores the protocol version in its SQLite database when the database is first created. On startup, it compares the stored version against the binary's protocol version:

- **Same major version** — server starts normally.
- **Different major version** — server refuses to start unless the migration flag is enabled.

> [!CAUTION]
> Automatic migration is **not yet implemented**. Enabling this flag today will cause the server to exit with a "not yet implemented" error. It exists so the upgrade path is explicit and opt-in once migration logic is added.

The `docker-compose.yml` includes this variable defaulting to `"false"`.

# BlazeList — User Guide

This document covers deployment, configuration, and operation of BlazeList.

## Quick Start

```bash
docker compose up
```

The Web UI will be available at `https://localhost:47800`.

> [!NOTE]
> The server uses a self-signed TLS certificate. Your browser will show a security warning on first visit — accept it to proceed. For production deployments, use a [reverse proxy](#reverse-proxy) with a trusted certificate.

## Deployment

### Docker

#### Ports

| Port | Protocol | Description |
|---|---|---|
| `47200` | UDP | QUIC — native clients and dev-seeder |
| `47400` | UDP | WebTransport — browser clients |
| `47600` | TCP | HTTP — internal cert-hash and config endpoints |
| `47800` | TCP | HTTPS — Web UI |

By default, `docker-compose.yml` binds ports to `127.0.0.1`. To expose on all interfaces:

```yaml
ports:
  - "0.0.0.0:47800:47800"
```

> [!NOTE]
> The QUIC port (`47200`) is commented out by default. Uncomment it to connect native clients or the dev-seeder.

#### Data Persistence

SQLite data is stored in a Docker volume mounted at `/data`:

```yaml
volumes:
  - blazelist-data:/data
command: ["--db", "/data/blazelist.db"]
```

#### UID/GID

The container runs as UID:GID `1000:1000` by default. To override:

```yaml
services:
  blazelist:
    user: "5000:5000"
```

---

### Reverse Proxy

For production deployments, use a reverse proxy (e.g., nginx) to serve the Web UI over HTTPS with a trusted TLS certificate.

The WebTransport port (`47400`) must be exposed directly to clients — reverse proxies cannot handle UDP/QUIC traffic. The browser connects to this port using the server's self-signed certificate, pinned by its SHA-256 hash (fetched via `/cert-hash`).

#### nginx Example

```nginx
server {
    listen 443 ssl;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # Serve the WASM frontend static files.
    root /var/www/blazelist;
    index index.html;

    # Proxy /cert-hash and /config to the BlazeList HTTP endpoint.
    # The WASM client fetches both from the same origin.
    location /cert-hash {
        proxy_pass http://127.0.0.1:47600/cert-hash;
    }

    location /config {
        proxy_pass http://127.0.0.1:47600/config;
    }
}
```

> [!NOTE]
> Open the WebTransport UDP port (`47400`) in your firewall. The browser connects directly to this port.

---

## Environment Variables

All environment variables are optional. Built-in defaults are used when not set.

### Client Default Settings

These override default values for WASM client settings. Served via the `/config` endpoint and applied on first load. Once a user changes a setting in the browser, their local preference takes priority.

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_DEFAULT_AUTO_SAVE` | Auto-save cards while editing | `false` |
| `BLAZELIST_DEFAULT_AUTO_SAVE_DELAY` | Auto-save delay in seconds | `5` |
| `BLAZELIST_DEFAULT_AUTO_SYNC` | Periodic sync check with server | `true` |
| `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL` | Periodic sync check interval in seconds | `10` |
| `BLAZELIST_DEFAULT_DEBOUNCE_ENABLED` | Enable push debounce (instant push when disabled) | `false` |
| `BLAZELIST_DEFAULT_DEBOUNCE_DELAY` | Push debounce delay in seconds | `5` |
| `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` | Enable keyboard shortcuts | `true` |
| `BLAZELIST_DEFAULT_SHOW_PREVIEW` | Show markdown preview when editing | `false` |
| `BLAZELIST_DEFAULT_SEARCH_TAGS` | Include tag names in search | `true` |
| `BLAZELIST_DEFAULT_UI_SCALE` | UI scale percentage | `100` |
| `BLAZELIST_DEFAULT_UI_DENSITY` | UI density mode (`compact` or `cozy`) | `compact` |
| `BLAZELIST_DEFAULT_TOUCH_SWIPE` | Enable touch swipe gestures on cards | `false` |

Boolean values are compared against `"true"` (case-sensitive). Numeric values must be valid unsigned integers.

Example:

```yaml
services:
  blazelist:
    environment:
      BLAZELIST_DEFAULT_AUTO_SYNC: "false"
      BLAZELIST_DEFAULT_AUTO_SAVE_DELAY: "10"
```

### SQLite Tuning

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_SQLITE_JOURNAL_MODE` | Journal mode | `WAL` |
| `BLAZELIST_SQLITE_SYNCHRONOUS` | Synchronous pragma (NORMAL is safe with WAL) | `NORMAL` |
| `BLAZELIST_SQLITE_CACHE_SIZE` | Page cache size (negative = KiB) | `-8388608` (~8 GiB) |
| `BLAZELIST_SQLITE_MMAP_SIZE` | Memory-mapped I/O limit in bytes | `8589934592` (8 GiB) |
| `BLAZELIST_SQLITE_TEMP_STORE` | Temp table/index storage | `MEMORY` |
| `BLAZELIST_SQLITE_BUSY_TIMEOUT` | Lock wait timeout in milliseconds | `5000` |

> [!NOTE]
> Values are validated to contain only `[a-zA-Z0-9_-]` before being used in PRAGMA statements.

### Schema Migration

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION` | Allow schema migration across major protocol versions | `false` |

On startup, the server compares the protocol version stored in the database against the binary's version:

- **Same major version** — starts normally.
- **Stored > current** — refuses to start (downgrade not supported).
- **Stored < current** — refuses to start unless migration is enabled.

---

## Offline Behavior (WASM Client)

The WASM PWA operates **offline-first**:

1. **Instant startup** — Renders immediately from a local cache in the browser's Origin Private File System (OPFS).
2. **Background sync** — Incremental sync over WebTransport fetches changes since the last session. Real-time subscription notifications keep the UI current.
3. **Offline editing** — Cards can be created and edited while offline. Changes are queued locally and pushed automatically when the connection is restored. The sync indicator shows a count of unsynced changes.
4. **Automatic reconnection** — Connection attempts use exponential backoff (5s to 60s). Returning to the app (visibility change) or regaining network connectivity triggers an immediate reconnect, even if a stale connection attempt was in progress.
5. **Automatic recovery** — If the local cache is evicted or corrupt, falls back to a full sync.

### Browser Requirements

- HTTPS and a modern browser.

---

## Keyboard Shortcuts

Press `?` to open the shortcuts panel. Shortcuts are suppressed while typing in inputs and can be disabled entirely in Settings.

Shortcuts can be disabled by default for all clients via the `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` environment variable.

---

## Touch Swipe Gestures

Disabled by default. Enable in Settings or via the `BLAZELIST_DEFAULT_TOUCH_SWIPE` environment variable.

- **Swipe right** — Blaze or extinguish the card.
- **Swipe left** — Set due date to today. If already set to today, sets to tomorrow.
---

## Attachments / File Hosting

BlazeList does not support file attachments. A workaround is to host a file server (e.g., [miniserve](https://github.com/svenstaro/miniserve)) on the same network and reference files in card Markdown:

- **Images** — `![alt text](https://<file-server>/image.png)`
- **Downloads** — `[filename](https://<file-server>/document.pdf)`

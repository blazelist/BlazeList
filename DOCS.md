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

### NixOS

The repo ships a flake exposing `nixosModules.default` (`services.blazelist`):

```nix
{
  inputs.blazelist.url = "github:blazelist/BlazeList";

  outputs = { nixpkgs, blazelist, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        blazelist.nixosModules.default
        {
          services.blazelist = {
            enable = true;
            bind   = "0.0.0.0";
            dbPath = "/var/lib/blazelist/blazelist.db";
          };
        }
      ];
    };
  };
}
```

For pinned, signature-verified builds, use `lib.buildFromCommit` to fetch
and GPG-verify a commit against the in-tree release signing key, then
build it:

```nix
services.blazelist.package = blazelist.lib.${pkgs.system}.buildFromCommit {
  rev  = "<full-commit-hash>";
  hash = "sha256-..."; # from `nix-prefetch-git --rev <rev> <url>`
  # For an unsigned fork, point to its URL and opt out of verification.
  # The resulting store-path gets an `-unverified` suffix:
  # url    = "https://example.org/me/BlazeList.git";
  # verify = false;
};
```

`verify` defaults to `true` — the build fails if the commit isn't signed
by the key at [`release-signing-key.asc`](release-signing-key.asc).

For sibling units (e.g. a `miniserve` sidecar serving user-uploaded
assets next to the main server), reuse the same systemd hardening
attrs the module applies internally:

```nix
systemd.services.my-blazelist-sidecar.serviceConfig =
  blazelist.lib.${pkgs.system}.hardeningSettings // {
    ExecStart = "...";
    ReadWritePaths = [ "/var/lib/my-sidecar" ];
  };
```

The attrset is safe for static-binary network services with the same
threat model (no JIT, only `AF_INET`/`AF_INET6`/`AF_UNIX`/`AF_NETLINK`,
only `@system-service` syscalls). See
[`nix/hardening-settings.nix`](nix/hardening-settings.nix) for the
full caveats.

See [`nix/module.nix`](nix/module.nix) for the full option set.

---

## Environment Variables

All environment variables are optional. Built-in defaults are used when not set.

### Client Default Settings

These override default values for WASM client settings. Served via the `/config` endpoint and applied on first load. Once a user changes a setting in the browser, their local preference takes priority.

| Variable | Description | Default |
|---|---|---|
| `BLAZELIST_DEFAULT_AUTO_SYNC` | Periodic sync check with server | `true` |
| `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS` | Periodic sync check interval in milliseconds | `10000` |
| `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_ENABLED` | Coalesce bursts of card moves into one push | `true` |
| `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS` | Card-move debounce window in milliseconds | `3000` |
| `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` | Enable keyboard shortcuts | `true` |
| `BLAZELIST_DEFAULT_SHOW_PREVIEW` | Show markdown preview when editing | `false` |
| `BLAZELIST_DEFAULT_SEARCH_TAGS` | Include tag names in search | `true` |
| `BLAZELIST_DEFAULT_UI_SCALE` | UI scale percentage | `100` |
| `BLAZELIST_DEFAULT_UI_DENSITY` | UI density mode (`compact` or `cozy`) | `compact` |
| `BLAZELIST_DEFAULT_TOUCH_SWIPE` | Enable touch swipe gestures on cards | `false` |
| `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_CYCLE` | Swipe right trigger distance in px in `cycle` swipe-left mode | `135` |
| `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS` | Swipe right trigger distance in px in `levels` swipe-left mode | `135` |
| `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_CYCLE` | Swipe left trigger distance in px in `cycle` swipe-left mode | `115` |
| `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS` | Swipe left trigger distance in px in `levels` swipe-left mode (also marks the start of the Today zone) | `95` |
| `BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS` | Swipe undo toast dismiss timeout in milliseconds | `4000` |
| `BLAZELIST_DEFAULT_SWIPE_LEFT_MODE` | Swipe-left mode: `levels` (distance picks the action) or `cycle` (each swipe advances) | `levels` |
| `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TODAY_WIDTH` | Levels-mode zone width (px) for the Today action (additive: zones extend outward from `SWIPE_THRESHOLD_LEFT_LEVELS`) | `75` |
| `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH` | Levels-mode zone width (px) for the Tomorrow action | `60` |
| `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_SOON_WIDTH` | Levels-mode zone width (px) for the In-2-days action; beyond it is the open-ended Clear-due region | `55` |
| `BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH` | Clear tag search input after selecting a tag | `true` |
| `BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH` | Enable sidebar width override | `false` |
| `BLAZELIST_DEFAULT_SIDEBAR_WIDTH` | Default sidebar width in px (when override enabled) | `180` |
| `BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH` | Enable detail panel width override | `false` |
| `BLAZELIST_DEFAULT_DETAIL_WIDTH` | Default detail panel width in px (when override enabled) | `0` |
| `BLAZELIST_DEFAULT_RECURSIVE_LINKS` | Recursively expand all transitively linked cards | `true` |
| `BLAZELIST_DEFAULT_SHOW_LIST_LINK_COUNTS` | Show transitive link counts in card list (computed in background) | `false` |
| `BLAZELIST_DEFAULT_SHOW_DUE_TODAY_BUTTON` | Show Today quick-filter button beside due date dropdown | `true` |
| `BLAZELIST_DEFAULT_SHOW_CARD_TIME` | Show the card-list relative-time label ("x ago") on each row | `false` |
| `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_SET` | Extinguish a Blazed card when its due date is set or changed | `true` |
| `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_CLEAR` | Also extinguish a Blazed card when its due date is cleared | `true` |
| `BLAZELIST_DEFAULT_CLEAR_DUE_ON_BLAZE` | Clear a card's due date when blazing it | `true` |
| `BLAZELIST_DEFAULT_DRAG_AND_DROP_ENABLED` | Enable drag-and-drop card reordering in the list (active only when sorted by priority) | `false` |
| `BLAZELIST_DEFAULT_DRAG_AND_DROP_MODE` | Drag activation: `anywhere` (pointerdown anywhere on the card, desktop-friendly) or `handle` (the card's leading number only, mobile-friendly) | `anywhere` |

Boolean values are compared against `"true"` (case-sensitive). Numeric values must be valid unsigned integers.

Example:

```yaml
services:
  blazelist:
    environment:
      BLAZELIST_DEFAULT_AUTO_SYNC: "false"
      BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS: "30000"
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
| `BLAZELIST_SQLITE_CHECKPOINT_INTERVAL` | WAL checkpoint interval in seconds (0 to disable) | `60` |

> [!NOTE]
> Values are validated to contain only `[a-zA-Z0-9_-]` before being used in PRAGMA statements.

### Tag Implications

Each tag carries an `implies` list of direct parent tag IDs. When a card is
pushed, the server checks that its tag set is closed under the transitive
closure of the implication relation — if any implied tag is missing, the
push is rejected with `TagImplicationViolation`. When a tag's `implies`
list changes, any existing cards that would fall out of compliance must be
brought back into compliance in the **same** `PushBatch` as the tag update;
the server rejects the whole batch otherwise. The server never fabricates
card versions on its own.

Cycles in the implication graph are rejected with `TagImplicationCycle`.

The v2 → v3 schema migration adds an `implies BLOB` column to both
`tags` and `tag_versions`. Pre-feature tag hashes continue to verify
after upgrade because `canonical_tag_hash` appends the implies block
only when the list is non-empty.

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
6. **Version-aware caching** — On startup, the client compares the protocol version (client and server) against the stored fingerprint. If either version has changed (e.g. after an update), all local caches are evicted and rebuilt via a full sync. The offline queue is preserved so unsynced edits are not lost.

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

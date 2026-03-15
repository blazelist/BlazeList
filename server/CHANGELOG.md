# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.2.0] - 2026-03-15

### Added

- Periodic WAL checkpointing — writes committed WAL pages back to the main
  database file on a configurable interval (default: 60 s), preventing
  unbounded WAL growth during long-running sessions
- `BLAZELIST_SQLITE_CHECKPOINT_INTERVAL` environment variable (seconds,
  default: `60`, set to `0` to disable)
- Graceful shutdown on SIGINT and SIGTERM — aborts the checkpoint task,
  runs a final WAL checkpoint, and exits cleanly
- `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT` environment variable
- `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT` environment variable
- `BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH` environment variable
- `BLAZELIST_DEFAULT_SIDEBAR_WIDTH` environment variable
- `BLAZELIST_DEFAULT_DETAIL_WIDTH` environment variable
- `BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH` environment variable
- `BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH` environment variable

### Added

- `tracing` dependency for structured checkpoint diagnostics

## [2.1.0] - 2026-03-15

### Added

- `BLAZELIST_DEFAULT_SEARCH_TAGS` environment variable (default: `true`)
- `BLAZELIST_DEFAULT_UI_SCALE` environment variable (default: `100`)
- `BLAZELIST_DEFAULT_UI_DENSITY` environment variable (default: `compact`)
- `BLAZELIST_DEFAULT_TOUCH_SWIPE` environment variable (default: `false`)

### Removed

- `BLAZELIST_DEFAULT_DRAG_DROP` environment variable (drag & drop removed from client)

## [2.0.0] - 2026-03-15

### Added

- Atomic major-to-major SQLite schema migration with startup gating —
  upgrades are executed sequentially (e.g., 0 -> 1 -> 2) in a single
  transaction with full rollback on failure
- `/config` HTTP and HTTPS endpoint serving client default settings as JSON
- `BLAZELIST_DEFAULT_*` environment variables for overriding client defaults:
  `AUTO_SAVE`, `AUTO_SAVE_DELAY`, `SHOW_PREVIEW`, `AUTO_SYNC`,
  `AUTO_SYNC_INTERVAL`, `DEBOUNCE_ENABLED`, `DEBOUNCE_DELAY`
- `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` environment variable for overriding client keyboard shortcuts default

### Changed

- Server now rejects `DeleteTag` when cards still reference the tag, returning
  `OrphanedTagReference` error. Clients must remove the tag from all referencing
  cards before deleting it (use `PushBatch` for atomicity).

## [1.0.0] - 2026-03-07

### Added

- Dual transport layer: QUIC (default port 47200) and WebTransport (default
  port 47400), both sharing the same request handler
- Auto-generated self-signed ECDSA P-256 certificates (14-day validity for
  WebTransport compliance)
- HTTP cert-hash endpoint (default port 47600) exposing SHA-256 certificate
  hash with CORS for WASM clients
- Optional HTTPS static-file server (default port 47800) with SPA routing
  for serving the WASM frontend
- SQLite storage backend with WAL mode and tunable PRAGMAs via environment
  variables (journal mode, cache size, mmap size, synchronous mode, etc.)
- 256-bucket root hash optimization — mutations only recompute the affected
  bucket, then XOR all 256 buckets for the root hash, avoiding O(N) rescans
- Ancestor hash chain validation on every push, preventing concurrent
  mutation conflicts
- Soft deletion: deleted entities preserved for reliable incremental sync
- Real-time subscription via `tokio::sync::broadcast` (capacity 64),
  shared across both transports
- Atomic batch operations with full rollback on any item failure
- Separate reader/writer SQLite connections for concurrent read access
  under WAL mode
- CLI with `--quic-port`, `--wt-port`, `--http-port`, `--https-port`,
  `--bind`, `--db`, and `--static-dir` options
- `docker-compose.yml` with migration env var defaulting to `false`
- Schema version tracking in SQLite — stores protocol version on first run
  and checks compatibility on every subsequent startup
- `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION` environment
  variable for future cross-major-version migration opt-in

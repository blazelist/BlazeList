# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [4.0.0] - 2026-07-05

### Added

- `BLAZELIST_DEFAULT_DRAG_AND_DROP_ENABLED` (default `false`) and
  `BLAZELIST_DEFAULT_DRAG_AND_DROP_MODE` (`anywhere` or `handle`, default
  `anywhere`) — defaults for drag-and-drop card reordering in WASM clients.
- `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_SET`,
  `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_CLEAR`, and
  `BLAZELIST_DEFAULT_CLEAR_DUE_ON_BLAZE` (default for "clear due date when
  blazing").
- `BLAZELIST_DEFAULT_SWIPE_LEFT_MODE` (`levels` or `cycle`) — default
  swipe-left mode.
- `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TODAY_WIDTH`,
  `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH`, and
  `BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_SOON_WIDTH` — additive zone widths (px)
  for levels-mode swipe-left.
- `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS` and
  `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS` — levels-mode swipe trigger
  distances (px), pairing with the renamed `_RIGHT_CYCLE` / `_LEFT_CYCLE`.
- `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS` (default `3000`) and
  `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_ENABLED` (default `true`) — card-move
  debounce defaults, forwarded via `/config` as `priority_debounce_delay_ms` /
  `priority_debounce_enabled`. When disabled, every move pushes immediately.

### Changed

- **Breaking:** rename `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT` →
  `..._RIGHT_CYCLE` and `..._LEFT` → `..._LEFT_CYCLE`, with the `/config` keys
  renamed in lockstep (`swipe_threshold_right_cycle` /
  `swipe_threshold_left_cycle`). Old env vars are ignored.
- **Breaking:** removed push-debounce env vars
  `BLAZELIST_DEFAULT_DEBOUNCE_ENABLED` and `BLAZELIST_DEFAULT_DEBOUNCE_DELAY`
  and their `/config` keys (`debounce_enabled`, `debounce_delay`). Switch to
  `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS` (now in milliseconds); the new
  behavior debounces only card moves, every other edit pushes immediately.
- **Breaking:** rename and rescale `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL`
  (seconds, default `10`) → `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS` (ms,
  default `10000`), with the `/config` key renamed `auto_sync_interval` →
  `auto_sync_interval_ms`. Old name is ignored.

### Removed

- **Breaking:** `BLAZELIST_DEFAULT_AUTO_SAVE` and
  `BLAZELIST_DEFAULT_AUTO_SAVE_DELAY` — the auto-save-while-editing feature is
  removed from the WASM client and its `/config` keys are no longer emitted.

### Fixed

- WebTransport binds IPv4 listen addresses directly instead of via a dual-stack
  IPv6 socket, fixing Linux `127.0.0.1` clients that couldn't reach a server
  bound to an unspecified/loopback address; dual-stack is now used only for IPv6
  loopback/unspecified addresses.
- HTTPS static-file handler no longer serves `index.html` for every asset when
  `--static-dir` is a Nix `symlinkJoin` output; traversal is now rejected by a
  literal `..`-segment check on the cleaned request path, restoring strict-MIME
  and SRI for `.js` / `.css` / `.wasm` / `.webmanifest`.

## [3.0.0] - 2026-05-04

### Added

- Tag-implication invariant enforced on every push (batch and single-item): the
  post-batch tag graph is checked for cycles and dangling `implies` references
  (`TagImpliesUnknown`), and every card is verified closed under the implication
  relation (`TagImplicationViolation`); any violation rolls back the whole
  transaction.
- Schema v2 → v3 migration adds an `implies BLOB NOT NULL DEFAULT X'00'` column
  to `tags` and `tag_versions`; existing rows deserialize as `[]` with no
  backfill, stored tag hashes still verify, and fresh databases declare the
  column identically. Gated by
  `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION`.
- Tag deletion now also rejects deleting a tag still declared as a parent by
  another live tag, with `OrphanedTagImpliesReference`.
- `GetAllCardHistories` and `GetAllTagHistories` bulk history endpoints with
  optional per-entity limit and ID filter.
- `BLAZELIST_DEFAULT_SHOW_DUE_TODAY_BUTTON`, `BLAZELIST_DEFAULT_RECURSIVE_LINKS`,
  `BLAZELIST_DEFAULT_SHOW_LIST_LINK_COUNTS`,
  `BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS`, and
  `BLAZELIST_DEFAULT_SHOW_CARD_TIME` env vars.
- Structured logging via `tokio-tracing` / `tracing-subscriber`, log levels via
  `RUST_LOG` (default `info`); replaces prior `println!` / `eprintln!`
  diagnostics in server and dev-seeder.

### Changed

- **Breaking:** Tied to protocol 3.0.0 — the `Tag` wire format changed,
  `PushError` gained four appended variants, and `canonical_tag_hash` takes an
  additional `implies` argument. Old and new clients/servers are mutually
  incompatible.
- **Breaking:** Storage schema gains an `implies` column on both tag tables (see
  migration above).
- Tag deletion still rejects via `OrphanedTagReference` when live cards
  reference the tag; the batch validator additionally catches post-batch closure
  violations from co-pushed tag updates.

## [2.2.0] - 2026-03-15

### Added

- Periodic WAL checkpointing with structured `tracing` diagnostics — writes
  committed WAL pages back to the main database on a configurable interval
  (default 60 s), preventing unbounded WAL growth.
- `BLAZELIST_SQLITE_CHECKPOINT_INTERVAL` env var (seconds, default `60`, `0`
  disables).
- Graceful shutdown on SIGINT/SIGTERM — aborts the checkpoint task, runs a final
  WAL checkpoint, and exits cleanly.
- `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT`,
  `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT`, `BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH`,
  `BLAZELIST_DEFAULT_SIDEBAR_WIDTH`, `BLAZELIST_DEFAULT_DETAIL_WIDTH`,
  `BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH`, and
  `BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH` env vars.

## [2.1.0] - 2026-03-15

### Added

- `BLAZELIST_DEFAULT_SEARCH_TAGS` (default `true`), `BLAZELIST_DEFAULT_UI_SCALE`
  (default `100`), `BLAZELIST_DEFAULT_UI_DENSITY` (default `compact`), and
  `BLAZELIST_DEFAULT_TOUCH_SWIPE` (default `false`) env vars.

### Removed

- `BLAZELIST_DEFAULT_DRAG_DROP` env var (drag & drop removed from client).

## [2.0.0] - 2026-03-15

### Added

- Atomic major-to-major SQLite schema migration with startup gating — upgrades
  run sequentially (e.g. 0 → 1 → 2) in a single transaction with full rollback
  on failure.
- `/config` HTTP and HTTPS endpoint serving client default settings as JSON.
- `BLAZELIST_DEFAULT_*` env vars for client defaults: `AUTO_SAVE`,
  `AUTO_SAVE_DELAY`, `SHOW_PREVIEW`, `AUTO_SYNC`, `AUTO_SYNC_INTERVAL`,
  `DEBOUNCE_ENABLED`, `DEBOUNCE_DELAY`.
- `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` env var for overriding client keyboard
  shortcuts.

### Changed

- Server now rejects `DeleteTag` when cards still reference the tag, returning
  `OrphanedTagReference`; clients must first remove the tag from all referencing
  cards (use `PushBatch` for atomicity).

## [1.0.0] - 2026-03-07

### Added

- Dual transport: QUIC (default port 47200) and WebTransport (default port
  47400), sharing one request handler.
- Auto-generated self-signed ECDSA P-256 certificates (14-day validity for
  WebTransport compliance).
- HTTP cert-hash endpoint (default port 47600) exposing the SHA-256 certificate
  hash with CORS for WASM clients.
- Optional HTTPS static-file server (default port 47800) with SPA routing for
  the WASM frontend.
- SQLite storage backend in WAL mode with PRAGMAs tunable via env vars (journal
  mode, cache size, mmap size, synchronous mode, etc.).
- 256-bucket root hash: mutations recompute only the affected bucket, then XOR
  all 256 buckets, avoiding O(N) rescans.
- Ancestor hash chain validation on every push, preventing concurrent mutation
  conflicts.
- Soft deletion of entities for reliable incremental sync.
- Real-time subscription shared across both transports (broadcast capacity 64).
- Atomic batch operations with full rollback on any item failure.
- Separate reader/writer SQLite connections for concurrent reads under WAL.
- CLI options: `--quic-port`, `--wt-port`, `--http-port`, `--https-port`,
  `--bind`, `--db`, `--static-dir`.
- `docker-compose.yml` with the migration env var defaulting to `false`.
- Schema version tracking in SQLite — stores protocol version on first run and
  checks compatibility on every subsequent startup.
- `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION` env var for future
  cross-major-version migration opt-in.

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-05-04

### Added

- Tag implication invariant enforcement. A new
  `validate_batch_implications` helper runs inside the existing
  `push_batch` transaction after every per-item write and before
  `recompute_root`. It snapshots the live tag graph + live card tag
  sets, overlays the batch's updates on top, runs iterative cycle
  detection on the post-batch tag graph, and verifies every
  post-batch card is closed under the implication relation. Any
  violation rolls back the whole transaction. Single-item
  `push_card_versions` and `push_tag_versions` route through the same
  validator via synthetic one-element batches, so isolated pushes can't
  bypass the invariant either.
- Schema v2 → v3 migration: adds an `implies BLOB NOT NULL DEFAULT
  X'00'` column to both `tags` and `tag_versions`. The default is the
  postcard encoding of `Vec::<Uuid>::new()` (single zero byte), so
  existing rows deserialize as `[]` without backfill. Because
  `canonical_tag_hash` appends the implies block only when non-empty,
  existing stored tag hashes continue to verify after upgrade. The
  migration is gated by `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION`
  consistent with every prior major bump.
- Fresh-database `init_schema` declares the `implies` column as the last
  column of both `tags` and `tag_versions`, so the column layout is
  identical to what `ALTER TABLE ADD COLUMN` produces on a v2→v3
  upgrade. A regression test
  (`fresh_and_migrated_schemas_have_identical_table_info`) asserts
  `PRAGMA table_info` matches byte-for-byte between the two paths.
- Tag deletion now scans `tags.implies` in addition to `cards.tags`:
  deleting a tag that another live tag still declares as a parent is
  rejected with `OrphanedTagImpliesReference`. Without this check the
  referenced tag would move to `deleted_entities` and every card
  holding the implying tag would permanently trip
  `TagImplicationViolation`.
- The batch tag-implication validator gains a dangling-reference pass:
  any tag whose `implies` list references an unknown or already-deleted
  tag id (post-batch) is rejected with `TagImpliesUnknown`. Runs before
  cycle detection because cycles in a graph with dangling edges are
  meaningless.
- `GetAllCardHistories` and `GetAllTagHistories` bulk history endpoints with
  optional per-entity limit and ID filter
- `BLAZELIST_DEFAULT_SHOW_DUE_TODAY_BUTTON` environment variable
- `BLAZELIST_DEFAULT_RECURSIVE_LINKS` environment variable
- `BLAZELIST_DEFAULT_SHOW_LIST_LINK_COUNTS` environment variable
- `BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS` environment variable
- `BLAZELIST_DEFAULT_SHOW_CARD_TIME` environment variable
- Structured logging via `tokio-tracing` with `tracing-subscriber`
  initialised in `main()`; log levels controllable via `RUST_LOG`
  (default `info`). All `println!`/`eprintln!` diagnostic output in
  server and dev-seeder replaced with structured tracing macros

### Changed

- **Breaking:** Tied to protocol 3.0.0 — wire format for `Tag`
  changed, `PushError` gained four appended variants, and
  `canonical_tag_hash` takes an additional `implies` argument. Old
  clients cannot talk to new servers and vice versa.
- **Breaking:** Storage schema gains an `implies` column on both
  tag tables (see migration note above).
- Tag deletion still rejects via `OrphanedTagReference` when live
  cards reference the tag. The new batch validator additionally
  catches post-batch closure violations introduced by co-pushed
  tag updates.

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

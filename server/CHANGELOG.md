# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-03-15

### Added

- `PushError::OrphanedTagReference` variant — returned when attempting to delete
  a tag that is still referenced by one or more cards.

### Changed

- **Breaking:** `PushError` enum variant ordering changed (new variant inserted
  at position 3). This is a wire format breaking change requiring all clients and
  servers to upgrade together.
- **Breaking:** Card priority changed from `NonNegativeI64` (0..=i64::MAX) to
  full `i64` range, allowing negative priorities.

## [1.0.0] - 2026-03-07

### Added

- Core data models: `Card` (with content, priority, tags, blazed status, due date),
  `Tag` (with title and optional color), `DeletedEntity`, and `RootState`
- Request/response protocol covering card CRUD, tag CRUD, root state queries,
  and incremental sync (`GetChangesSince`)
- `PushBatch` for atomic multi-item mutations (cards, tags, deletions) with
  all-or-nothing rollback semantics
- `Subscribe` request for real-time push notifications on server mutations
- BLAKE3 hash chain verification — each card/tag version carries a hash
  computed from a canonical byte layout plus its ancestor hash
- Length-prefixed postcard binary wire format (4-byte BE length + payload,
  16 MiB maximum message size)
- Version handshake with semver compatibility (major version must match)
- `ChangeSet` type for incremental sync deltas (cards, tags, deletions, root)
- Priority placement algorithm with midpoint + random jitter to avoid
  collisions when multiple clients insert concurrently
- Sequence history tracking (`SequenceHistoryEntry` with per-operation details)
- `CardFilter` enum (All / Blazed / Extinguished) for filtered listing
- Comprehensive error types: `ProtocolError`, `PushError`, `BatchItemError`,
  `WireError`, `HandshakeError`, `HashVerificationError`
- Card and tag version history queries with optional limits

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.1.0] - 2026-07-05

### Added

- `ALPN_PROTOCOL` constant (`b"blazelist/0"`) exported from the protocol crate — the ALPN identifier negotiated during the QUIC/WebTransport TLS handshake; the negotiated value is unchanged from previous releases.

## [3.0.0] - 2026-05-04

### Added

- `Tag::implies` field (direct parent tag IDs forming an implication graph, sorted and deduplicated on ingress). New `Tag::first_with_implies` / `Tag::next_with_implies` constructors take the list explicitly; `Tag::first` / `Tag::next` default to empty / preserve existing. `Tag::implies()` accessor returns a sorted slice of parent IDs.
- New module `tag::graph` with a `TagGraph` helper: `from_tags` / `from_pairs`, `upsert` / `remove`, `closure_of`, `missing_for_card`, `detect_cycle`, plus an `affected_cards_for_change` free function. Shared by the server validator and the client confirm dialog.
- Four `PushError` variants: `TagImplicationViolation { card_id, missing }` (a card, including any in a batch's post-batch state, is missing a transitively-implied tag); `TagImplicationCycle { cycle }` (a tag push or post-batch graph would contain a cycle); `OrphanedTagImpliesReference { tag_id, referencing_tag_ids }` (deleting a tag another live tag still declares in its `implies` list); `TagImpliesUnknown { tag_id, missing }` (a pushed tag's `implies` references ids not live in the post-batch snapshot).
- `GetAllCardHistories` / `GetAllTagHistories` request variants — bulk fetch of version histories with optional per-item limit and ID filter; `AllCardHistories` / `AllTagHistories` response variants carry `HashMap<Uuid, Vec<Card/Tag>>` payloads.

### Changed

- **Breaking:** `Tag` gains a trailing `implies: Vec<Uuid>` field, changing the postcard wire format (postcard rejects struct field additions); all clients and servers must upgrade together.
- **Breaking:** `canonical_tag_hash` takes an extra `implies: &[Uuid]` argument, appended to the canonical bytes **only when non-empty** — a tag with `implies = []` hashes byte-identically to the pre-feature format, so existing on-disk tag hashes still verify.
- **Breaking:** `Tag::from_parts` takes an additional `implies: Vec<Uuid>` argument.
- **Breaking:** `PushError` gains four appended variants at positions 7-10 (`TagImplicationViolation`, `TagImplicationCycle`, `OrphanedTagImpliesReference`, `TagImpliesUnknown`); existing variants keep their positions, but exhaustive matches need new arms.
- **Breaking:** `affected_cards_for_change` now takes only `(next, cards)` — the unused `prev` parameter is removed.
- `Tag.implies` no longer carries `#[serde(default)]` — cross-major wire compatibility relies on the major version bump, and postcard ignores the attribute for trailing positional fields anyway.
- **Breaking:** `VersionResult::Ok` now carries `server_version: Version` so clients can detect server version changes during the handshake.
- **Breaking:** `client_handshake` returns `Version` instead of `()`.

## [2.1.0] - 2026-03-15

### Removed

- `compute_priority` and `priority_percentage` public exports (moved to `blazelist-client-lib::priority`).
- `rand` dependency (no longer needed without priority computation).

## [2.0.0] - 2026-03-15

### Added

- `PushError::OrphanedTagReference` variant — returned when deleting a tag still referenced by one or more cards.

### Changed

- **Breaking:** `PushError` variant ordering changed (new variant inserted at position 3) — a wire-format break; all clients and servers must upgrade together.
- **Breaking:** Card priority widened from `NonNegativeI64` (0..=i64::MAX) to the full `i64` range, allowing negative priorities.

## [1.0.0] - 2026-03-07

### Added

- Core data models: `Card` (content, priority, tags, blazed status, due date), `Tag` (title, optional color), `DeletedEntity`, `RootState`.
- Request/response protocol: card CRUD, tag CRUD, root state queries, and incremental sync (`GetChangesSince`).
- `PushBatch` — atomic multi-item mutations (cards, tags, deletions) with all-or-nothing rollback semantics.
- `Subscribe` request — real-time push notifications on server mutations.
- BLAKE3 hash-chain verification — each card/tag version carries a hash computed from a canonical byte layout plus its ancestor hash.
- Length-prefixed postcard binary wire format (4-byte BE length + payload, 16 MiB maximum message size).
- Version handshake with semver compatibility (major version must match).
- `ChangeSet` type for incremental sync deltas (cards, tags, deletions, root).
- Priority placement algorithm — midpoint + random jitter to avoid collisions on concurrent inserts.
- Sequence history tracking (`SequenceHistoryEntry`, per-operation details).
- `CardFilter` enum (All / Blazed / Extinguished) for filtered listing.
- Error types: `ProtocolError`, `PushError`, `BatchItemError`, `WireError`, `HandshakeError`, `HashVerificationError`.
- Card and tag version history queries with optional limits.

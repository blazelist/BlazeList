# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-05-04

### Added

- `Tag::implies` field — direct parent tag IDs forming an implication
  graph. The field is sorted and deduplicated on every ingress. New
  `Tag::first_with_implies` and `Tag::next_with_implies` constructors
  let callers specify the list explicitly; `Tag::first` and `Tag::next`
  default to an empty list and preserve the existing list respectively.
- `Tag::implies()` accessor returning a sorted slice of parent IDs.
- New module `tag::graph` with a `TagGraph` helper exposing
  `from_tags` / `from_pairs`, `upsert` / `remove`, `closure_of`,
  `missing_for_card`, `detect_cycle`, and an `affected_cards_for_change`
  free function. Shared by both the server-side validator and the
  client-side confirm dialog.
- `PushError::TagImplicationViolation { card_id, missing }` — returned
  when a card push (or any card appearing in a batch's post-batch
  state) is missing a transitively-implied tag.
- `PushError::TagImplicationCycle { cycle }` — returned when a tag push
  (or the post-batch tag graph produced by a batch) would contain a
  cycle.
- `PushError::OrphanedTagImpliesReference { tag_id, referencing_tag_ids }` —
  returned when deleting a tag that another live tag still declares in its
  `implies` list. Without this guard, the referenced tag would move to
  `deleted_entities` and every card holding the implying tag would become
  permanently non-compliant.
- `PushError::TagImpliesUnknown { tag_id, missing }` — returned when a
  pushed tag's `implies` list references one or more tag ids that are not
  present as live tags in the post-batch snapshot (either never seen or
  already deleted).
- `GetAllCardHistories` request variant — bulk fetch of card version
  histories with optional per-card limit and card ID filter.
- `GetAllTagHistories` request variant — bulk fetch of tag version
  histories with optional per-tag limit and tag ID filter.
- `AllCardHistories` and `AllTagHistories` response variants carrying
  `HashMap<Uuid, Vec<Card/Tag>>` payloads.

### Changed

- **Breaking:** `Tag` gains an `implies: Vec<Uuid>` field at the end of
  the struct. The postcard wire format changes because postcard does
  not tolerate struct field additions between versions. All clients
  and servers must upgrade together.
- **Breaking:** `canonical_tag_hash` takes an extra `implies: &[Uuid]`
  argument. The implies block is appended to the canonical bytes
  **only when non-empty**, so a tag with `implies = []` hashes
  byte-identically to the pre-feature format — existing on-disk tag
  hashes continue to verify after upgrade.
- **Breaking:** `Tag::from_parts` takes an additional `implies:
  Vec<Uuid>` argument. Storage layers must feed it through when
  reconstructing rows.
- **Breaking:** `PushError` gains four appended variants at positions 7,
  8, 9, and 10 (`TagImplicationViolation`, `TagImplicationCycle`,
  `OrphanedTagImpliesReference`, `TagImpliesUnknown`). The existing
  variants keep their positions, but any exhaustive match on `PushError`
  will need new arms.
- **Breaking:** `affected_cards_for_change` now takes only `(next, cards)`
  — the unused `prev` parameter has been removed. Callers that want
  diff semantics against a previous graph can compute `prev.missing_for_card`
  and `next.missing_for_card` themselves.
- `Tag.implies` no longer carries `#[serde(default)]`; cross-major wire
  compatibility is handled by the major version bump, not by serde
  attributes, and postcard does not honor the attribute for trailing
  positional fields anyway.
- **Breaking:** `VersionResult::Ok` now carries `server_version: Version`
  so clients can detect server version changes during the handshake.
- **Breaking:** `client_handshake` returns `Version` instead of `()`.

## [2.1.0] - 2026-03-15

### Removed

- `compute_priority` and `priority_percentage` public exports (moved to
  `blazelist-client-lib::priority` where they are actually used)
- `rand` dependency — no longer needed without priority computation

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

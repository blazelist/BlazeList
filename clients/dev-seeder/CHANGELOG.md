# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-05-04

### Added

- Environment size presets (`--preset small|medium|large`) for quick
  switching between dataset sizes. Medium (400 cards, 18 tags) is the
  new default — roughly a third of the previous 1200/50. `--cards` and
  `--tags` flags still work as explicit overrides.
- Seeded tag implications: `apply_seeded_implications` picks ~25% of
  generated tags and appends a `next_with_implies` version pointing at
  1–2 earlier-indexed (smaller-index) live tags, guaranteeing the
  resulting implication graph is a DAG. Every card generator now
  threads a `TagGraph` snapshot and closes its tag sets under that
  graph before writing a card version, so the seeded batch passes the
  server's new implication invariant validator even with non-empty
  implies. Exercises both the "tag gained implies later" path and the
  retroactive-closure code.

### Changed

- **Breaking:** Wire-compatible with protocol 3.0.0 only.
- Phase progress diagnostics now use `tracing` instead of `println!`,
  with `tracing-subscriber` initialised in `main()` and levels
  controllable via `RUST_LOG` (default `info`).

## [2.1.0] - 2026-03-15

### Changed

- Card content patterns now include markdown horizontal rule separators (`---`)
  for testing hr rendering

### Fixed

- Priority generation now spans the full `i64` range (`i64::MIN..=i64::MAX`)
  instead of only the non-negative half, populating the entire list

## [2.0.0] - 2026-03-15

### Changed

- Dev seeder now removes doomed tags from cards before deleting them, matching
  new server referential integrity enforcement.

## [1.0.0] - 2026-03-07

### Added

- Deterministic seeded data generation via ChaCha8 RNG (default seed: 42)
- CLI with configurable server address, RNG seed, card count (default
  1200), and tag count (default 50)
- Eight markdown content patterns: short sentences, heading + paragraphs,
  GFM task lists, bullet lists, code blocks, multi-paragraph essays,
  blockquotes, and heading + bullet + notes
- Tag generation with optional colors (~60 % receive a color from a
  10-color palette) and version history (~30 % get 1–3 renames)
- Card generation with weighted distributions for tag assignment, blazed
  status (~40 %), due dates (temporal bias toward near-today), and version
  history depth (10 % single version, 27 % short, 63 % deep)
- Five edit types for card history: content rewrites, priority shifts, tag
  changes, blazed toggles, and due date changes
- Internal card linking: ~50 % of cards contain UUID references to other
  cards, including deduplication and self-reference edge cases
- Deleted entity generation (~75 % of a separate doomed card set, 3–5
  doomed tags) to exercise sync of deleted entities
- Three-phase push strategy: batch create, batch delete doomed entities,
  then 120 individual extra operations for rich sequence history
- QUIC client with insecure certificate verification for development use

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.1.0] - 2026-07-05

### Added

- New `gen_card_content` pattern: a GFM markdown table (heading, intro, 2–4
  columns, 3–7 rows of random-word cells, trailing note); ~1 in 12 cards.
- Three Markdown blockquote content patterns: multi-paragraph, nested, and
  list-wrapping.

### Changed

- `generate_card_history` now skews edits toward single-field changes
  (~80 % one field, 17 % two, 3 % three) across the five edit types
  (content / priority / tags / blazed / due-date), replacing the previous
  near-uniform 1–3 fields per edit.

## [3.0.0] - 2026-05-04

### Added

- Environment size presets `--preset small|medium|large`; medium
  (400 cards, 18 tags) is the new default (was 1200/50). `--cards` and
  `--tags` still work as explicit overrides.
- Seeded tag implications: `apply_seeded_implications` gives ~25 % of tags a
  `next_with_implies` version implying 1–2 earlier-indexed tags (implication
  graph is a DAG), and each card's tag set is closed under the resulting
  `TagGraph` before its card version is written.

### Changed

- **Breaking:** Wire-compatible with protocol 3.0.0 only.
- Progress diagnostics now use `tracing` (was `println!`); log levels via
  `RUST_LOG` (default `info`).

## [2.1.0] - 2026-03-15

### Changed

- Card content patterns now include markdown horizontal rules (`---`).

### Fixed

- Priority generation now spans the full `i64` range
  (`i64::MIN..=i64::MAX`), not just non-negative values.

## [2.0.0] - 2026-03-15

### Changed

- Removes doomed tags from cards before deleting them, matching new server
  referential-integrity enforcement.

## [1.0.0] - 2026-03-07

### Added

- Deterministic data generation via ChaCha8 RNG (default seed 42).
- CLI: configurable server address, RNG seed, card count (default 1200), and
  tag count (default 50).
- Eight markdown content patterns: short sentences, heading + paragraphs,
  GFM task lists, bullet lists, code blocks, multi-paragraph essays,
  blockquotes, and heading + bullet + notes.
- Tag generation: ~60 % get a color from a 10-color palette; ~30 % get 1–3
  renames.
- Card generation with weighted distributions for tag assignment, blazed
  (~40 %), due dates (biased near-today), and version-history depth
  (10 % single, 27 % short, 63 % deep).
- Five card edit types: content, priority, tags, blazed, and due date.
- Internal card linking: ~50 % of cards hold UUID references to other cards
  (incl. deduplication and self-reference cases).
- Deleted-entity generation (~75 % of a separate doomed card set, 3–5 doomed
  tags) to exercise sync of deleted entities.
- Three-phase push: batch create, batch delete doomed entities, then 120
  individual extra operations for sequence history.
- QUIC client with insecure certificate verification for development use.

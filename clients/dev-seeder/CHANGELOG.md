# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
  doomed tags) to exercise sync tombstones
- Three-phase push strategy: batch create, batch delete doomed entities,
  then 120 individual extra operations for rich sequence history
- QUIC client with insecure certificate verification for development use

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.1.0] - 2026-03-15

### Added

- Tag-inclusive search: `filter_cards` accepts an optional tag list to include
  tag names in full-text search matching
- Linked card preview underline coloring by status (active vs blazed)
- `compute_priority` and `priority_percentage` functions (moved from protocol)
- `resolve_collision` function for computing a valid priority when the desired
  value is already taken by another card

### Changed

- Edge inserts (top/bottom of list) now cap priority jumps to ~32k instead of
  halving the full i64 range, dramatically reducing rebalance frequency for
  sequential insertions
- Named constants for priority computation: `MAX_EDGE_GAP` (65,536) and
  `JITTER_DIVISOR` (16)
- Markdown horizontal rule rendering with balanced vertical spacing

## [2.0.0] - 2026-03-15

### Added

- Due date sort orders (ascending and descending)
- Include-overdue option in due date filtering
- Inline linked-card preview rendering with short UUID + card title
- `TagFilterMode` (And / Or) for multi-tag filtering

### Changed

- Major version bump for protocol compatibility.
- Card priority uses the full `i64` range (was `NonNegativeI64`),
  updating placement and rebalancing logic accordingly.
- Replaced `HashMap` with `IndexMap` for deterministic iteration order;
  use `sort_unstable` where stable ordering is not required.

## [1.0.0] - 2026-03-07

### Added

- Platform-agnostic `Client` trait abstracting card/tag CRUD, root state
  queries, incremental sync, batch push, and subscription
- Incremental sync helpers (`apply_card_changeset`, `apply_tag_changeset`)
  that merge server changesets into local state
- Filtering pipeline: blaze status, full-text search, tag filter with
  AND/OR mode and "no tags" option, due date filter
  (overdue/today/upcoming), and linked-card filter
- Eight sort orders: priority, created-at, modified-at, and title —
  each ascending and descending
- Markdown processing via comrak (GFM): plain-text extraction, card
  preview generation, task-list checkbox toggling, and task progress
  counting
- Bidirectional card linking: extract forward links (UUIDs in content),
  compute back links, resolve linked cards to previews, single-pass
  link-count computation, and post-process HTML to make UUIDs clickable
- WCAG 2.0 relative-luminance calculation for tag chip color contrast
  (automatically lightens text on dark backgrounds)
- Due date utilities: status computation, badge formatting, display
  formatting, and quick presets (Today, Tomorrow, Next Monday, Next
  Friday)
- Priority placement and automatic gap rebalancing — expands from the
  insertion point to find packed ranges and redistributes evenly
- Relative timestamp formatting ("5s ago", "3d ago", etc.)

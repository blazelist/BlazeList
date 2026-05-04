# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-05-04

### Added

- `tag_graph` module re-exporting `blazelist_protocol::TagGraph`,
  `TagImplicationCycle`, and `affected_cards_for_change` so WASM and
  other client code can compute implication closures, detect cycles,
  and preview the set of cards that would need new versions when a
  tag's implies list changes.
- `get_all_card_histories` and `get_all_tag_histories` convenience methods
  on the `Client` trait for bulk history fetching
- `compute_all_link_counts_recursive` — augments direct link counts with
  transitive reachability count via BFS expansion
- `transitive` field on `LinkCounts` struct for cards reachable beyond
  direct forward/back links
- `mutual` field on `LinkCounts` struct — count of cards that this card
  links to and that also link back. `compute_all_link_counts` classifies
  each pair as forward-only, back-only, or mutual; the recursive variant
  sums all three for the BFS seed set.
- `expand_linked_cards` BFS traversal for recursive linked card expansion
- `NextTue`, `NextWed`, `NextThu`, `NextSat`, and `NextSun` variants on
  `DueDatePreset` — the dropdown now offers a quick-pick for every
  weekday rather than only Monday and Friday.

### Changed

- **Breaking:** `LinkCounts.forward` and `.back` are now exclusive of
  mutual links — a bidirectional pair contributes only to `mutual`, not
  to `forward` or `back`. Consumers displaying these counts must read
  the new `mutual` field to recover the previous total.
- **Breaking:** `DueDateFilter::Upcoming` variant removed — use
  `TodayAndUpcoming` instead. "Next 7 days" and "Next 14 days" filters
  now include today (previously they started from tomorrow).
- **Breaking:** Tied to protocol 3.0.0 — `Tag::from_parts` takes an
  additional `implies: Vec<Uuid>` argument and `Tag` carries an
  `implies` field on the wire. Tag client code upgraded accordingly.
- **Breaking:** `affected_cards_for_change` now takes only `(next, cards)`
  — the previously unused `prev` parameter has been removed. Call sites
  in the tag-detail editor updated accordingly.
- **Breaking:** UUID extraction in card content now requires the UUID to
  start the text or be preceded by whitespace, naturally excluding UUIDs
  embedded in URLs, markdown link targets, and text without a separator.
  Affects `extract_card_links` and the markdown UUID-to-card-link
  post-processor.

## [2.3.0] - 2026-03-23

### Added

- `InTwoDays` variant to `DueDatePreset` with `in_two_days_midnight()` helper —
  quick-pick option for setting due dates to two days from now, automatically
  available in card detail and editor dropdowns via `DueDatePreset::ALL`
- `reconcile_offline_queue` function that filters an offline card queue against
  local state — brand-new cards (ancestor hash is zero) are always kept, and
  only cards whose local version is strictly newer are dropped

## [2.2.0] - 2026-03-17

### Added

- `wrap_code_blocks_with_copy_button` post-processor that wraps rendered `<pre>`
  code blocks in a container with a hover-reveal copy-to-clipboard button

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

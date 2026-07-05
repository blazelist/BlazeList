# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [4.0.0] - 2026-07-05

### Added

- `TagFilterMode::Nor` and `TagFilterMode::Nand` variants — exclusion
  filters. `Nor` excludes cards with one or more selected tags; `Nand`
  excludes only cards having all selected tags (they coincide for a
  single tag).
- `TagFilterMode` helpers: `description`, `ALL`, `url_value`,
  `from_url_value`, and `allows_no_tags`.
- `TagFilterMode::next` — cycles all four variants (OR → AND → NOR →
  NAND → OR); replaces the binary `toggle`.
- `DueDateStatus` now derives `Debug`, `Clone`, `Copy`, `PartialEq`,
  and `Eq`.
- `DueDateFilter::url_value` and `DueDateFilter::from_url_value` —
  round-trip the due-date filter through its `f.due` URL query token.

### Changed

- **Breaking:** `TagFilterMode::label` returns `&'static str` (was
  `&str`).
- **Breaking:** `TagFilterMode::toggle` removed — replaced by
  `TagFilterMode::next`, which cycles all four modes.

### Fixed

- Bracket-wrapped card UUIDs with no inner whitespace — `(UUID)`,
  `[UUID]`, `{UUID}` — are now detected as card links; markdown link
  and reference targets (`[text](UUID)`, `[text][UUID]`) remain
  excluded. Affects `extract_card_links` and the markdown
  UUID-to-card-link post-processor.

## [3.0.0] - 2026-05-04

### Added

- `tag_graph` module re-exporting `blazelist_protocol::TagGraph`,
  `TagImplicationCycle`, and `affected_cards_for_change` — compute
  implication closures, detect cycles, and preview the cards needing
  new versions when a tag's implies list changes.
- `get_all_card_histories` and `get_all_tag_histories` — bulk-history
  convenience methods on the `Client` trait.
- `compute_all_link_counts_recursive` — adds transitive reachability
  counts via BFS.
- `transitive` field on `LinkCounts` — cards reachable beyond direct
  forward/back links.
- `mutual` field on `LinkCounts` — count of cards this card links to
  that also link back; `compute_all_link_counts` classifies each pair
  as forward-only, back-only, or mutual.
- `expand_linked_cards` — BFS traversal for recursive linked-card
  expansion.
- `NextTue`, `NextWed`, `NextThu`, `NextSat`, and `NextSun` variants on
  `DueDatePreset` — quick-pick for every weekday.

### Changed

- **Breaking:** `LinkCounts.forward` and `.back` now exclude mutual
  links — a bidirectional pair counts only toward `mutual`. Read the
  new `mutual` field to recover the previous total.
- **Breaking:** `DueDateFilter::Upcoming` removed — use
  `TodayAndUpcoming`. "Next 7 days" and "Next 14 days" now include
  today (previously started tomorrow).
- **Breaking:** Tied to protocol 3.0.0 — `Tag::from_parts` takes an
  added `implies: Vec<Uuid>` argument and `Tag` carries an `implies`
  field on the wire.
- **Breaking:** `affected_cards_for_change` now takes only
  `(next, cards)` — the unused `prev` parameter is removed.
- **Breaking:** UUID extraction now requires the UUID to start the text
  or be preceded by whitespace, excluding UUIDs embedded in URLs and
  markdown link targets. Affects `extract_card_links` and the
  markdown UUID-to-card-link post-processor.

## [2.3.0] - 2026-03-23

### Added

- `InTwoDays` variant on `DueDatePreset` with `in_two_days_midnight()`
  helper — quick-pick for two days out, exposed via `DueDatePreset::ALL`.
- `reconcile_offline_queue` — filters an offline card queue against
  local state: brand-new cards (ancestor hash zero) are always kept;
  cards whose local version is strictly newer are dropped.

## [2.2.0] - 2026-03-17

### Added

- `wrap_code_blocks_with_copy_button` — wraps rendered `<pre>` code
  blocks with a hover-reveal copy-to-clipboard button.

## [2.1.0] - 2026-03-15

### Added

- `filter_cards` accepts an optional tag list, adding tag names to
  full-text search matching.
- Linked-card preview underline colored by status (active vs blazed).
- `compute_priority` and `priority_percentage` functions (moved from
  protocol).
- `resolve_collision` — computes a valid priority when the desired
  value is already taken by another card.

### Changed

- Edge inserts (top/bottom of list) cap priority jumps to ~32k instead
  of halving the full i64 range, reducing rebalance frequency for
  sequential insertions.
- Named priority constants `MAX_EDGE_GAP` (65,536) and `JITTER_DIVISOR`
  (16).
- Balanced vertical spacing for markdown horizontal rules.

## [2.0.0] - 2026-03-15

### Added

- Due-date sort orders (ascending and descending).
- Include-overdue option in due-date filtering.
- Inline linked-card preview rendering (short UUID + card title).
- `TagFilterMode` (And / Or) for multi-tag filtering.

### Changed

- Major version bump for protocol compatibility.
- Card priority uses the full `i64` range (was `NonNegativeI64`),
  updating placement and rebalancing accordingly.
- Replaced `HashMap` with `IndexMap` for deterministic iteration order.

## [1.0.0] - 2026-03-07

### Added

- Platform-agnostic `Client` trait — card/tag CRUD, root state queries,
  incremental sync, batch push, and subscription.
- Incremental sync helpers `apply_card_changeset` and
  `apply_tag_changeset` — merge server changesets into local state.
- Filtering pipeline: blaze status, full-text search, tag filter
  (AND/OR mode, "no tags" option), due-date filter
  (overdue/today/upcoming), and linked-card filter.
- Eight sort orders: priority, created-at, modified-at, and title —
  each ascending and descending.
- Markdown processing via comrak (GFM): plain-text extraction,
  card-preview generation, task-list checkbox toggling, and
  task-progress counting.
- Bidirectional card linking: extract forward links (UUIDs in content),
  compute back links, resolve linked-card previews, single-pass
  link-count computation, and clickable-UUID HTML post-processing.
- WCAG 2.0 relative-luminance calculation for tag-chip contrast
  (lightens text on dark backgrounds).
- Due-date utilities: status computation, badge and display formatting,
  and quick presets (Today, Tomorrow, Next Monday, Next Friday).
- Priority placement with automatic gap rebalancing — redistributes
  packed ranges evenly from the insertion point.
- Relative timestamp formatting ("5s ago", "3d ago").

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.2.0] - 2026-03-07

### Added

- App icons (favicon, Apple touch icon, 192px and 512px PWA icons) derived
  from the new BlazeList logo
- PWA manifest now includes icon entries for installable app support

## [1.1.0] - 2026-03-07

### Added

- Automatic reconnection with exponential backoff (1 s → 30 s) when the
  server connection is lost; backoff resets on successful reconnection
- Browser `visibilitychange` and `online` event listeners that interrupt
  the backoff sleep and reconnect immediately when the user returns
- Click-to-reconnect on the sync indicator when disconnected

### Fixed

- "Add to top" placed card partway down the list instead of at the top
  (cards were unsorted before computing priority placement)
- Card move operations (top / up / down / bottom) used the unfiltered
  card list, causing incorrect positioning when filters were active

### Changed

- Increased button and input touch targets on small viewports for
  easier tapping on mobile devices

## [1.0.0] - 2026-03-07

### Added

#### Connection & sync
- WebTransport connection with self-signed certificate support
  (SHA-256 hash fetched from server's HTTP endpoint)
- Protocol version handshake on connect
- Initial full sync and incremental sync via `GetChangesSince`
- Real-time subscription stream — automatic incremental sync on every
  server mutation notification
- Connection status indicator (Connected / Connecting / Syncing /
  Disconnected) with manual sync button

#### Cards
- Create cards with top or bottom placement
- Edit card content, tags, and due date with live markdown preview
- Delete cards (with confirmation dialog)
- Toggle blaze status (active / blazed)
- Move cards: to top, up one, down one, to bottom, or jump to a
  specific position — all operations use the filtered list order
- Priority placement with automatic gap rebalancing when the priority
  space is exhausted
- Card version history viewer with restore and fork actions
- Copy card ID to clipboard
- Debounced content auto-save (300 ms)
- Pending version batching to reduce server round-trips

#### Markdown
- Full GitHub Flavored Markdown rendering (strikethrough, tables,
  autolinks, task lists) via comrak
- Interactive task-list checkboxes — click any list item to toggle
- Live split-pane editor with preview toggle
- Card UUID references in content rendered as clickable links

#### Tags
- Create tags from the sidebar
- Rename tags inline with save/cancel
- Assign and clear custom RGB colors with hex input and color picker
- Delete tags (with confirmation)
- Tag version history viewer
- Filter by tags: multi-select with AND/OR mode toggle, plus a
  "no tags" filter for untagged cards
- Tag color dots on card list items (grid of up to 9, +N overflow)

#### Due dates
- Set due dates via quick presets (Today, Tomorrow, Next Monday,
  Next Friday), a dropdown menu, or a native date picker
- Clear due dates
- Due date badges with relative status and color coding (overdue /
  today / upcoming)
- Filter by due date status (Overdue / Today / Upcoming)

#### Linked cards
- Bidirectional card linking: forward links (UUIDs in content),
  back links (inferred), and mutual links
- Linked cards section in the detail panel with direct navigation
- Link count indicators on card list items (forward and back counts)
- Filter to show a source card and all its linked cards

#### Filtering & sorting
- Status filter (Active / All / Blazed)
- Full-text search across card content with clear button
- Sort by priority, creation date, modification date, or due date
- All filters compose and persist in URL query parameters (browser
  back/forward compatible)

#### Layout & responsiveness
- Three-panel layout: resizable sidebar, card list, and detail panel
- Drag-to-resize handles with min/max constraints
- Sidebar hidden by default on viewports < 768 px, toggled via
  hamburger menu with overlay
- Adaptive initial detail panel width (50 % of viewport, clamped
  280–800 px)

#### Sidebar
- Alphabetically sorted tag list with manage button per tag
- Statistics section: root hash, sequence number, card counts
  (total / active / blazed), tag count, deleted entity count, and
  last sync time
- Expandable sequence history with per-entry operation details and
  clickable entity navigation

#### Card list
- Card preview with truncated content (first 200 chars)
- Zero-padded index numbers
- Task progress indicator (completed / total)
- Modified timestamp in relative format (auto-refreshed every 30 s)

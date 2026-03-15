# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-03-15

### Added

- Option to disable push debounce for instant card updates (toggle in settings)
- `BLAZELIST_DEFAULT_DEBOUNCE_ENABLED` server-side environment variable

#### Offline-first storage
- Cards, tags, deleted entities, and root state are persisted in the
  browser's Origin Private File System (OPFS). The UI renders instantly from
  cached data on startup; a WebTransport connection syncs in the background.
- Card version history, tag version history, and sequence history are cached
  locally in OPFS. Previously viewed histories render instantly from cache
  while a background refresh fetches the latest data from the server.
- OPFS is now required — the app refuses to start if OPFS is unavailable
  (e.g., insecure context or unsupported browser).
- Request `navigator.storage.persist()` on startup to reduce eviction risk.
- Automatic `RootHashMismatch` recovery: wipes the local cache and performs
  a full re-sync from the server.

#### Settings & configuration
- Settings page with device-local preferences: auto-save, auto-sync,
  markdown preview, drag & drop reorder, and push debounce delay
- Auto-save for card editing with configurable countdown timer
- Periodic sync check with configurable interval (default 10 s) and countdown in header
- Configurable push debounce delay (default off; 5 s when enabled) with countdown in header
- `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS` server-side environment variable
- Server-side default settings via `/config` endpoint
  (`BLAZELIST_DEFAULT_*` env vars); priority chain is
  localStorage > server env var > hardcoded default

#### Editor
- Searchable tag lists in the card editor and sidebar
- Tag panel displays side-by-side with the editor textarea on wide viewports,
  dynamically matching the textarea height
- Inline linked-card previews for UUID references in markdown
  (short UUID + card title)
- Unsaved changes guard with `beforeunload` confirmation

#### Navigation & filtering
- Browser back/forward navigation via `pushState` + `popstate`
- Due date filter dropdown with time range options
  (Overdue / Today / Today & upcoming / Upcoming) and include-overdue toggle
- Sorting by due date (ascending and descending)
- Active accent styling on sort dropdown and search input when filters are set
- "Filter by due date" placeholder when no due date filter is active

#### Keyboard & interaction
- Keyboard shortcuts (can be disabled in settings, `?` always shows help)
- Move card up/down via Shift+J / Shift+K
- Go to first/last card via g / G
- New card at bottom/top via n / N, above/below selected via o / O
- Search confirmation with Enter selects the first filtered card
- New card button dropdown with placement options (bottom, top, above/below selected)
- Newly created cards are automatically selected
- Position hint shown on the new card page (e.g. "Adding below ...")
- Device-local drag & drop card reorder toggle
- Two-step tag deletion with inline error display

#### Sync indicator
- Push debounce and auto-sync countdowns shown in header
- Sync duration and operation count in sidebar stats
- Reconnect countdown with click-to-reconnect

### Changed

- Removed position dropdown from the card editor save button (position is now
  chosen before opening the editor via button dropdown or keyboard shortcut)
- Replaced exponential backoff reconnection with a fixed 5-second retry
  countdown; the sync indicator shows seconds remaining and can be clicked
  to reconnect immediately
- Tag deletion atomically removes the tag from all referencing cards via
  `PushBatch` before deleting, matching server referential integrity
- App initialization loads from OPFS before entering the connection loop
  and uses incremental sync when cached state exists
- Renamed "Auto-sync with server" to "Periodic sync check" with clearer
  description reflecting its role as a consistency verification mechanism
- Split sidebar stats into individual entries: Total Cards, Active Cards,
  Blazed Cards, Tags, Deleted Entities, Total Entities
- Last Sync timestamp now updates every second and always displays in seconds
- Relative timestamp refresh interval changed from 30 s to 1 s
- CSS split from a single monolithic file into modular files
- Card priority uses the full `i64` range (was `NonNegativeI64`)

### Fixed

- Sticky hover states on tag elements for touch devices
- Sync indicator layout jump when the refresh icon appeared/disappeared

## [1.2.0] - 2026-03-07

### Added

- App icons (favicon, Apple touch icon, 192px and 512px PWA icons) derived
  from the new BlazeList logo
- PWA manifest now includes icon entries for installable app support

## [1.1.0] - 2026-03-07

### Added

- Automatic reconnection with exponential backoff (1 s -> 30 s) when the
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
- Debounced content auto-sync (1000 ms)
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
  280-800 px)

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
- Modified timestamp in relative format (auto-refreshed every 5 s)

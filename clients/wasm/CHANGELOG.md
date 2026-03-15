# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [2.2.0] - 2026-03-15

### Added

- UI scale setting changed to a simple number input field (50–300%)
- Configurable swipe trigger distance for left and right directions separately
  (default: 100 px right, 90 px left; range 40–150 px), shown in settings when
  touch swipe is enabled
- `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT` and
  `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT` server-side environment variables
- "Clear tag search on select" setting (default: enabled) — clears the tag
  search input in the sidebar and card editor after clicking a tag
- `BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH` server-side environment variable
- Override sidebar/detail panel width settings — each is a toggle that reveals
  a width input when enabled (default: off, uses standard defaults)
- `BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH`,
  `BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH`,
  `BLAZELIST_DEFAULT_SIDEBAR_WIDTH`, and `BLAZELIST_DEFAULT_DETAIL_WIDTH`
  server-side environment variables
- Conditional tooltips on card previews and tag names in the sidebar — tooltip
  appears only when the text is actually truncated
- "Reset all settings to defaults" button at the bottom of the settings page;
  clears all saved preferences and reloads the page
- Full offline PWA startup — service worker precaches all app assets so the
  UI loads instantly even without a network connection

### Changed

- Replaced `log`/`console_log` with `tracing`/`tracing-wasm` for structured,
  level-filtered logging
- Touch swipe uses rubber-band physics — 1:1 movement until the threshold,
  then diminishing drag beyond it; action label and background color fade in
  progressively and only commit when the threshold is reached
- Sidebar and detail panel can be resized to smaller minimums (sidebar 80 px,
  detail 200 px) and larger maximums (sidebar 500 px, detail 1400 px)
- Card list indicator: thin left accent bar on each card — green for active,
  red for blazed (matching card header status colors), brighter when selected
- Tag names in the sidebar truncate with ellipsis instead of wrapping,
  showing full text naturally when the sidebar is wider

### Fixed

- Pushes with stale OPFS-cached ancestor hashes no longer race against
  the initial sync — the client is not exposed globally until sync completes,
  so user-triggered saves during connection go to the offline queue and are
  reconciled after sync finishes
- Stale client from a previous connection is cleared on reconnect, preventing
  pushes through a dead transport during the sync window
- Live card and version pushes now rebase on ancestor hash mismatch instead
  of falling through to the offline queue — edits are applied on top of the
  server's latest version immediately, avoiding unnecessary queuing
- Offline queue flush also rebases on ancestor hash mismatch instead of
  silently dropping queued cards — the user's content is preserved on top of
  the server's latest version
- New card auto-save now works offline — card is added to local state and
  queued for sync instead of silently failing when disconnected
- Existing card updates with `DuplicatePriority` in the offline queue are
  now resolved by fetching the server version and rebasing with `.next()`,
  instead of looping forever in `remaining`
- Offline queue no longer silently drops cards on unhandled push errors —
  only `AlreadyDeleted` cards are dropped; all other errors keep the card
  queued for retry on the next sync cycle
- Sync failures now display an error message in the sync indicator bar,
  clearing automatically on the next successful sync

## [2.1.1] - 2026-03-15

### Fixed

- Offline queue flush now re-adds pushed cards to local state and persists to
  OPFS, preventing cards from disappearing until page reload after reconnect
- App header wraps gracefully on small viewports with consistent row spacing

## [2.1.0] - 2026-03-15

### Added

- "Include tags in search" setting (default: enabled) — search matches card
  content and tag names; the "no tags" special filter is excluded from search
- `BLAZELIST_DEFAULT_SEARCH_TAGS` server-side environment variable
- UI scale setting (75 % - 200 %) to increase or decrease the size of the
  entire interface
- `BLAZELIST_DEFAULT_UI_SCALE` server-side environment variable
- UI density setting: compact (default, unchanged) or cozy (larger tag dots,
  more spacing between cards)
- `BLAZELIST_DEFAULT_UI_DENSITY` server-side environment variable
- Keyboard shortcuts for due dates: `t` (set to today), `T` (set to tomorrow),
  `C` (clear due date)
- Keyboard shortcut `,` to open settings
- Keyboard shortcuts panel as a normal pane (like settings) instead of a popup
  overlay — press `?` or click "View shortcuts" in settings
- Touch swipe gestures on cards (disabled by default, enable in settings):
  swipe right to blaze/extinguish, swipe left to set due date to today
  (or tomorrow if already today)
- `BLAZELIST_DEFAULT_TOUCH_SWIPE` server-side environment variable
- Auto-save for new cards — seamlessly transitions to editing mode after the
  initial save without losing editor state
- Offline card and tag operations with a pending push queue that drains
  automatically on reconnect
- Tag creation uses the same color picker style as editing an existing tag;
  default color shown as placeholder when no color is explicitly selected

### Changed

- Auto-save while editing is now disabled by default (was enabled)
- Keyboard shortcuts help is now a pane in the detail panel area instead of a
  modal overlay
- Pane transitions (settings, shortcuts, card detail) share an unsaved-changes
  guard that prompts before discarding edits
- Linked card UUID underlines are now colored green for active cards and red
  for blazed cards, matching the card header status color
- Markdown horizontal rule (`---`) styling improved with balanced spacing and
  slightly brighter color

### Fixed

- Offline queue no longer silently drops cards on `DuplicatePriority` — the
  flush now recomputes priority (with rebalancing if gaps are exhausted) and
  retries the push
- Auto-sync no longer destroys unsaved editor content — the detail panel uses
  a memoized open signal to prevent unnecessary re-renders
- Query parameters now update when saving a new card, keeping the URL in sync
  with the selected card
- Reconnect no longer gets stuck in "Connecting..." — simplified to a fixed
  5-second retry instead of exponential backoff

### Removed

- Drag & drop card reorder option and all associated drag-and-drop handlers
- `BLAZELIST_DEFAULT_DRAG_DROP` server-side environment variable

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

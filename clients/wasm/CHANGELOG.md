# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [3.0.0] - 2026-05-04

### Added

- **Tag implications.** The tag detail view gains an "Implies" section
  where you can add direct parent tags. While editing, an inline
  affected-cards preview lists every card whose tag set would need a
  new version under the new graph (with the missing chips highlighted).
  Save runs local cycle detection as a fast-fail, then submits one
  `PushBatch` containing the new tag version plus every affected card
  version — the server accepts or rejects the whole thing atomically.
- Card editor auto-cascades transitively-implied tags when a tag is
  toggled on. Removing a chip cascades upward — anything that
  transitively requires the removed tag is removed alongside it,
  while tags that don't depend on it stay selected.
- Client-side cache schema stamp (`wasm_version+protocol_version`) written
  to localStorage whenever `save_local_state` successfully writes
  `blazelist.db`. Checked at load time — before any network activity —
  so client upgrades automatically wipe incompatible OPFS caches even
  offline, and the stale main-DB / populated history-cache "ghost" state
  cannot happen. Replaces the previous connect-time fingerprint eviction
  that required a successful protocol handshake and therefore could never
  recover from offline upgrades. Offline queue is preserved across schema
  changes so unsynced user edits survive client upgrades
- `Ctrl+Enter` / `Cmd+Enter` keyboard shortcut to save/create the active card
- `Enter` in tag search input toggles the first matching tag
- Multi-level swipe-left gesture for due date control: cycles through today,
  tomorrow, in-2-days, and clear based on the card's current due date
- Keyboard sub-menus for due date (`d`), sort (`s`), and linked cards (`l`)
  shortcuts with floating popup and `q`/`Esc` to cancel
- Direct keyboard shortcuts: `a`/`A`/`b` blaze filter, `v`/`V` tag filter
  mode / "no tags" filter, `i` include-overdue, `x` toggle sidebar, `r`
  reset all filters, `f`/`/` focus search, `F` focus tag search, `h`
  browser history back, `y` copy card ID, `Y` new tag
- `Enter` in sidebar tag search toggles the first matching tag filter
  and blurs the input; `Esc` blurs without toggling
- Prev/next navigation buttons (‹/›) in card detail header — mirrors `k`/`j`
  keyboard shortcuts for mouse/touch users, disabled at list boundaries
- Pre-fetch all card/tag/sequence history during sync for full offline access
- Today quick-filter button beside due date dropdown
- Card schedule dropdown now lists every weekday — "Next tuesday",
  "Next wednesday", "Next thursday", "Next saturday", and "Next sunday"
  join the existing "Next monday" / "Next friday" entries in both the
  card editor and card detail due-date dropdowns. Labels are spelled
  out and lowercased to match "Tomorrow" / "In 2 days".
- Full link graph cache — background BFS computes the complete reachable
  card set for each linked card via `requestIdleCallback`, processing
  what fits in each idle window. Persisted to OPFS, survives reloads,
  invalidated on content changes. Card detail reads cached results
  instantly instead of re-running BFS. Cache misses on card selection
  compute on demand and inject into cache.
- Show transitive link counts in card list setting (default: on) — ⋯N
  indicators derived from the link graph cache
- Linked card filter dropdown in card detail — filter by forward only (→),
  back only (←), or direct (↔) links in addition to the existing "all linked"
  button
- Recursive linked cards setting (enabled by default) — transitively expands
  all linked cards in card detail, following forward and back links through
  the entire web of connected cards with deduplication
- Blaze status highlighting on linked cards in card detail view, matching
  the highlighting used elsewhere in the UI
- "Local Cache" section in sidebar stats showing history cache sizes,
  link graph progress (N/M with percentage and progress bar), and
  offline queue status
- Swipe action toast with undo for mobile touch gestures
- Brief "Copied ID" toast notification on `y` shortcut and on the card
  detail copy-ID button (shows the first 8 characters of the UUID,
  auto-dismisses after 1.5 s)
- Configurable swipe undo toast timeout (setting and
  `BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS` env var)
- Reusable `Timestamp` component for consistent date/time display
  (full UTC value shown inline, with hover tooltip)
- "Show card last-edited time" setting (default: off) — the relative-
  time label ("x ago") on the right of each card-list row is now
  hidden by default on every viewport. Users who want it can opt in
  via the setting or the `BLAZELIST_DEFAULT_SHOW_CARD_TIME` env var
- Detail-panel expand/collapse mode with `⛶ / ⤢` header toggle (and
  `m` / `M` keyboard shortcuts). Phones default to expanded, desktop
  to collapsed; the previous tag-filter-mode bindings move to `v` / `V`
- Fullscreen (expanded) detail panel uses native page scroll instead
  of an internal scroll container
- "Clear local cache" button in the settings Danger Zone — wipes all
  cached cards, tags, history, and link-graph data and reloads the app
  for a full re-sync from the server
- Mutual-link indicator (`↔N`) in card list rows and card detail
  summary, alongside the existing `→N` / `←N` / `⋯N` indicators

### Changed

- **Breaking:** Tied to protocol 3.0.0. The OPFS cache schema stamp
  (`wasm_version+protocol_version`) bumps automatically, so previous
  installations re-sync from scratch on first launch after upgrade.
- Quick due-date preset button toggles smartly: when a card is already
  due today (or overdue), clicking now sets it to tomorrow instead of
  resetting to today. The button label updates to reflect the next action.
- Default swipe-right (blaze) threshold is now 135 px (was 100 px); default
  swipe-left (due date) threshold is 115 px (was 90 px)
- Swipe-undo and error toasts on phones (≤ 480 px) get a wider
  max-width cap (`calc(100vw - 1rem)`) and tighter chrome (smaller
  inline gap, container padding, and Undo button padding) so short
  messages like "Blazed 🔥" or "Due: Today" stay on a single line
  instead of wrapping mid-phrase.
- Copy-ID toast dismiss timer resets when the same shortcut fires
  repeatedly, so back-to-back copies don't truncate the toast
- `ConfirmDeletePrompt` accepts `first_prompt: impl Fn() -> String`
  so it can carry dynamic messages. Both existing call sites in
  `card_detail.rs` and `tag_detail.rs` updated to pass closures.
- Atomic all-or-nothing cache load: `app.rs` startup now gates loading
  of `history.db` and `blazelist-link-cache.db` on `load_local_state`
  actually finding valid data in `blazelist.db`. If the main DB is
  missing/corrupt/empty, derived caches are cleared as well, so the
  sidebar never shows the "0 cards + hundreds of cached card histories"
  ghost state observed on older builds
- Sidebar `Local Cache` labels renamed: "Cards" → "Card Histories" and
  "Tags" → "Tag Histories". Those rows count `cached_card_history_count`
  / `cached_tag_history_count` respectively, not the main-DB card/tag
  counts (which are already shown under "Data"); the old labels misled
  readers into thinking they were seeing a parallel copy of the main DB
- Card list uses Leptos `<For>` keyed diffing instead of `.map().collect()` —
  DOM nodes persist across card updates instead of being recreated
- All card-item data (blazed status, due date, tags, preview) is now reactive
  via per-card `Memo` derivations instead of stale captured values
- Tag dots use tracked `state.tags.get()` so tag color changes propagate
- Card tag dots use a 4-dot 2×2 grid (custom-colored tags only); default-
  colored tags roll up into a single `+N` overflow count styled like the
  linked-card indicators
- Card list and card detail share a single `link_indicators_view`
  component and CSS class set; the per-detail `summary-*` classes are
  dropped. Linked-cards list and summary are now sorted
  mutual → forward → back → transitive to match the indicator order.
- `B` (capital) now blazes/extinguishes a card; `a`/`A`/`b` are direct filter
  shortcuts (blaze filter sub-menu removed — keys don't conflict)
- Settings panel organized into titled sections (Sync & Saving, Editor,
  Search & Filtering, Linked Cards, Input, Appearance, Layout, Danger Zone)
- Each setting shows its `BLAZELIST_DEFAULT_*` env var name
- Sub-settings (intervals, thresholds, widths) are now disabled instead
  of hidden when their parent toggle is off, keeping the UI stable
- Consistent indentation for env var info below sub-settings
- Removed redundant per-card position and link_counts Memos — inlined HashMap
  lookups, reducing reactive graph by 2N nodes
- Progressive card list rendering — the first 40 cards render
  immediately and the rest fill in via `requestIdleCallback`, keeping
  the initial paint snappy on large lists
- Card detail panel restructured into uniform bordered sections: controls
  (actions + due date), details (metadata), linked cards, and history
- Keyboard shortcuts ignore Ctrl/Alt/Meta modifiers so browser shortcuts
  (Ctrl+F, Alt+D, etc.) pass through correctly
- "Restore version" and "New from this version" (simple placement) in
  version history now queue offline via `push_card_or_queue`, matching
  the regular card edit / new card flows — they no longer silently
  require a live connection
- Operations that genuinely can't be queued offline (delete card,
  create / edit / delete tag, fork with priority rebalance) now show
  a prominent red `ErrorToast` explaining why the action didn't go
  through, instead of silently doing nothing. The toast fires both
  up-front when offline is already detected AND when the underlying
  client call returns `ClientError::ConnectionLost` (the common case
  where the server dropped but the client hasn't noticed yet). The
  `tag_detail` delete inline error is replaced by the same toast for
  consistency
- "Restore version" did not visually refresh the card detail after
  the upsert — `CardDetail`'s outer closure intentionally snapshots
  `state.cards` for editor stability, so the restored content stayed
  invisible until the user re-selected the card. `on_restore` now
  toggles `selected_card` after the upsert to force the detail to
  re-read, matching the behaviour the user expects from clicking
  "Restore"
- Service worker's default fetch fall-through was resolving to
  `undefined` on cache miss, causing the SW to throw
  `TypeError: Failed to convert value to 'Response'` on every
  cert-hash / config fetch while the server was unreachable. Now
  falls back to a synthetic 503 `Response` like the hashed-asset
  branch does
- Offline edits dropped during flush reconciliation (when the rebased
  version is rejected by the server for reasons other than
  `ConnectionLost`) now show an error toast so the user knows the
  edit was discarded, instead of only logging a warning
- Today quick-filter button now clears all other filters (blaze, tag,
  linked cards, search) and always enables "include overdue". The
  separate auto-include-overdue setting has been removed since the
  Today button handles it directly
- "New from this" in version history no longer creates a card silently —
  it now opens the card editor prefilled with the selected version's
  content, tags, and due date so the user can tweak before saving
- UUIDs embedded inside URLs, markdown link targets, or glued to
  surrounding text are no longer rendered as clickable card-link
  previews — only UUIDs at the start of text or after whitespace
  count as card references
- Due date filter dropdown: "Next 7 days" / "Next 14 days" labels
  replace "This week" / "Two weeks", and both ranges now include
  today (previously they started from tomorrow). The redundant
  "All upcoming" option is removed; bookmarked URLs with
  `f.due=upcoming` fall back to "Today & upcoming". The `U`
  due-date-submenu shortcut for "Upcoming only" is removed.

### Removed

- `evict_stale_caches` and the connect-time fingerprint check in
  `connect_and_run` — superseded by the load-time schema stamp, which
  handles the same job without requiring a server connection
- `build_cache_fingerprint` helper (no callers remain)
- `AppState::cache_bust` signal (only ever used to force a re-render
  after the old eviction mutated state behind the card list's back; the
  new eviction runs before the card list is mounted so no bust needed)
- `Client::server_version` field and method in the WASM transport —
  only consumer was the deleted fingerprint computation

### Fixed

- **PWA cold start hangs indefinitely when the network is stalled**
  (not just offline — flaky mobile signal, VPN transitions, captive
  portals, or an unreachable server that still accepts TCP). The
  service worker's navigation handler was network-first with a
  `.catch()` fallback to the cache, which looks correct but breaks
  when `fetch()` never resolves or rejects — the catch never fires
  and the browser waits on the navigation forever. Symptom: Android
  PWA stuck on the system splash screen (never reaches the in-HTML
  "Loading…") and desktop Chrome spins indefinitely. Navigation
  requests are now cache-first with stale-while-revalidate: the
  cached `/index.html` is served instantly, and a background fetch
  refreshes the cache when the network is healthy.
- Service worker `install` now fetches each precache URL with a
  hard 15s timeout and an `AbortController`, so a hanging network
  during an SW update can't wedge the worker in "installing" state
  (which would prevent `activate` / `clients.claim()` from ever
  running). The existing `Promise.allSettled` + partial-success
  behavior is preserved.
- `apply_server_config()` (the `/config` fetch that seeds settings
  defaults) now runs in parallel with `connection_loop()` instead
  of blocking it. Previously a hanging `/config` request would
  prevent the connection loop from ever starting, so the app
  couldn't reconnect when the network came back. Safe because
  each server-config write is guarded by `has_*()` — a user choice
  in localStorage still wins.
- Fullscreen (expanded) card detail on phones added a few pixels of
  horizontal scroll even when the content fit the viewport. On screens
  ≤480 px `.card-detail` uses 0.875 rem horizontal padding while
  `.detail-section` / `.linked-card-list` still use `-1rem` negative
  margins to stretch edge-to-edge, and the native-scroll refactor left
  `.main-layout` / `.detail-panel.expanded` with `overflow: visible`
  so the bleed reached the body. `.main-layout` now uses
  `overflow-x: clip; overflow-y: visible` in expanded mode — horizontal
  overflow is contained while vertical content still flows up to the
  body's native page scrollbar.
- Toggling the "Recursive linked cards" setting required a page reload
  to take effect — the linked-cards section now reactively tracks the
  setting and the in-flight idle-callback chain bails out on early-return
- Tag detail label corrected from "Transitively implied" to
  "Transitively implies"
- Swipe toggle mode setting overdue tasks to today instead of skipping
  ahead to tomorrow
- Swipe background color for extinguish action now shows cyan
- Offline-created cards silently dropped by flush reconciliation
- PWA offline cold start requiring navigate-away-and-back on Android —
  OPFS cache is now loaded before fetching `/config`, so the network
  request can no longer stall the splash screen when offline
- Offline card edits lost on app restart — the main OPFS database is
  now persisted alongside the offline queue, so changes survive closing
  and reopening the app
- Entire local database wiped on version-fingerprint change or root-hash
  mismatch when the follow-up full sync failed (network drop, server
  unreachable) — `blazelist.db` is no longer deleted eagerly; the
  initial sync now overwrites it atomically via `save_local_state`, and
  the old data is kept as a fallback when the sync cannot complete
- Swipe due date comparison using exact DateTime instead of calendar date
- Right sidebar panel overlap when opening new tag while shortcuts panel visible
- Keyboard sub-menus dismissed by modifier keys (Shift, etc.) before capital
  letter could be typed
- Long URLs and unbroken text in card content, editor preview, and version
  history now wrap instead of overflowing the viewport
- Linked card filter buttons (Filter Linked, Forward only, Back only, Direct)
  now clear the search query so results aren't hidden by stale search text
- `card_list.rs` blake3-hashing every card on every edit (regression from
  the link-graph cache work) — the hash pass is now scoped to the cards
  whose link cache could be affected, restoring steady-state performance

## [2.6.0] - 2026-03-27

### Fixed

- Offline cold start failures — replaced all-or-nothing `cache.addAll()` with
  resilient `Promise.allSettled()` so partial precache success still installs
  the service worker, preventing blank pages when large assets (e.g. WASM
  binary) fail to download on flaky mobile connections
- `isHashedAsset` regex not matching `_bg.wasm` multi-segment extensions,
  causing the WASM binary to use network-first instead of cache-first and
  adding unnecessary latency on offline cold starts
- Navigation fallback chain only tried `/index.html` — now tries the request
  URL, `/index.html`, `/`, and finally an inline offline page explaining the
  user needs to connect once
- Cross-origin requests intercepted by the fetch handler — added origin guard
  to let them pass through
- Service worker `skipWaiting()` called unconditionally — on partial precache
  failure during updates, the new service worker would activate and purge the
  old complete cache, leaving the user with a broken partial cache; now only
  skips waiting when all assets are cached successfully
- Service worker registration errors silently swallowed — added `.catch()`
  with console logging
- Connection status showed "Connected" before the client was globally
  available and could get stuck on "Syncing" after auto-sync or manual
  sync — consolidated all status updates so the UI only reports connected
  once pushes can actually succeed
- Blaze/extinguish toggle in card detail view not updating after blazing via
  button click, keyboard shortcut, or swipe — the status badge, button text,
  and button class were computed once from a snapshot; now use a reactive `Memo`
  that tracks `state.cards` so the UI updates without re-rendering the entire
  detail panel (which would lose editor state and version-history expansion)
- Due date display in card detail view not updating after setting or clearing
  via button, preset, date picker, keyboard shortcut, or swipe — the due date
  badge, date picker value, clear button, and metadata row were computed once
  from a snapshot; now use a reactive `Memo` (same pattern as the blaze fix).
  Also added no-op guards in the detail panel and keyboard shortcut handlers
  to skip creating duplicate card versions when the due date is unchanged

### Changed

- Relaxed priority reorder gating — card reordering is now only blocked when a
  non-default sort order is active; previously it was also blocked during search,
  which was unnecessarily restrictive since search preserves priority order
- Centralized reorder check into `AppState::reorder_allowed()` so card detail
  nav buttons and keyboard shortcuts (Shift+J / Shift+K) share the same logic;
  keyboard shortcuts previously had no reorder guard at all

## [2.5.0] - 2026-03-23

### Fixed

- Offline-created cards silently dropped by flush reconciliation — the filter
  compared queued cards against local state (which includes optimistically
  inserted cards) using `>=`, so new cards with the same count were dropped
  without ever reaching the server. Now uses `reconcile_offline_queue` from
  `blazelist-client-lib` which skips brand-new cards and uses strict `>`
- `HashVerificationFailed` errors (server doesn't have the card at all) now
  handled in both `push_card_or_queue` and `flush_offline_queue` by recreating
  the card as a first version, preserving all content. Previously these fell
  through to the catch-all error handler, leaving the card stuck in the queue

## [2.4.0] - 2026-03-17

### Added

- Copy-to-clipboard button on markdown code blocks — hover-reveal button in the
  top-right corner of rendered `<pre>` blocks copies the text content to the
  clipboard, wired into card detail, version history, and card editor preview
- Orange dotted-underline styling for markdown links in card content preview,
  matching the card UUID link treatment with `--accent` / `--accent-hover` colors

### Changed

- Renamed `?card=` query parameter to `?entity=` since it selects both cards
  and tags

### Fixed

- Loading screen showed scrollbars due to browser default 8 px body margin
  combined with `height:100vh` — set `margin:0` inline on `<body>` and removed
  the previous `margin:-8px;padding:8px` workaround
- Periodic sync check destroyed the card detail view and closed the version
  history panel — the outer reactive closure tracked `state.cards`, so any
  incremental sync replaced the entire component. Now uses untracked access
  with a self-healing tracked fallback for initial page load, matching the
  tag detail fix. Card and tag version history also re-trigger their fetch
  on connection status changes so history loads after the client connects
  on page reload
- Fenced code blocks in card detail and version history had a double-stacked
  dark background — `<code>` inside `<pre>` inherited its own
  `rgba(0,0,0,0.4)` on top of the `<pre>` background; reset `pre code` to
  transparent

## [2.3.1] - 2026-03-16

### Fixed

- Unsaved changes indicator on new tag creation and tag editing stayed dirty
  after reverting inputs to their original state — now uses a reactive Effect
  that compares against the original values, matching the card editor behavior

## [2.3.0] - 2026-03-16

### Fixed

- Periodic sync check destroying the tag detail panel and losing unsaved
  edits — the outer reactive closure tracked `state.tags` and `state.cards`,
  so any incremental sync replaced the entire `TagDetail` component, wiping
  in-progress title renames and color changes. The tag check now uses
  untracked access with a self-healing tracked fallback for initial page load.
- Setting a tag color discarded an in-progress title rename — the color push
  updated `state.tags`, which triggered the same reactive destruction

### Changed

- Unified tag editing: title and color are now edited together in a single
  editing mode with shared Save / Cancel buttons at the bottom, matching the
  card editing UX. Unsaved tag edits are protected by the same
  `has_unsaved_changes` guard and `beforeunload` confirmation as cards.
- New tag creation form now shows an `(unsaved)` indicator and prompts
  before discarding via Cancel or close, matching the card creation flow

## [2.2.1] - 2026-03-15

### Fixed

- "Today + overdue" due date filter (`f.inc_overdue` query parameter) was not
  restored on page reload — the toggle was hardcoded to `false` on init instead
  of being parsed from the URL like the other filters

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

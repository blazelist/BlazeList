# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [4.0.0] - 2026-07-05

### Added

- Markdown blockquotes (`>`) now render with a left accent bar and de-emphasized text in card detail, the editor preview, and version-history previews.
- The card detail "Tasks" row shows a rounded completion percentage alongside the count (e.g. `2/10 (20%)`); card-list items keep the bare `done/total`.
- A "Save" button saves the card without closing the editor (the old button becomes "Save & Close"), also bound to `Ctrl+S` / `Cmd+S`; in new-card flows it switches to editing the just-created card so later saves update it in place.
- Drag-and-drop card reordering (off by default, active only under priority sort) with two modes in Settings → Input: **Anywhere on card** (desktop) and **Card number only** (drag handle on the leading number, mobile-friendly, preserving native scroll and swipes). Drops coalesce into the card-move debounce. Env vars: `BLAZELIST_DEFAULT_DRAG_AND_DROP_ENABLED`, `BLAZELIST_DEFAULT_DRAG_AND_DROP_MODE`.
- The "Adding to bottom" hint in the new-card editor is now a dropdown to change the insertion point (top / bottom, or above/below the targeted card) while drafting; shown only before the first save.
- "Extinguish when setting a due date" setting (default on) extinguishes a Blazed card when its due date is set or changed, with a sub-option to also extinguish on clear (default on). Env vars: `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_SET`, `BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_CLEAR`.
- "Clear due date when blazing" setting (default on) clears a card's due date when it's blazed; extinguishing leaves it untouched. Env var: `BLAZELIST_DEFAULT_CLEAR_DUE_ON_BLAZE`.
- Levels mode for the swipe-left gesture: swipe distance directly picks Today / Tomorrow / In 2 days / Clear-due through progressively narrower zones, with the label and tint updating live. Settings → Touch swipe gestures → Swipe-left mode (default `levels`); the previous behavior stays as `cycle`.
- Levels-mode zone widths are configurable per device: `swipe_levels_zone_today_width` (default 75 px), `swipe_levels_zone_tomorrow_width` (default 60 px), `swipe_levels_zone_soon_width` (default 55 px).
- Tag-filter gains two exclusion modes alongside OR / AND: **NOR** (exclude cards with any selected tag) and **NAND** (exclude cards with all selected tags); with one tag selected the two behave identically.
- The tag-mode button cycles OR → AND → NOR → NAND; the `v` shortcut opens a sub-menu (`o`=OR, `a`=AND, `n`=NOR, `N`=NAND, `q`/`Esc` to cancel). Mode round-trips in the URL as `f.tag_mode=and|nor|nand` (omitted for OR).
- Today quick-filter keyboard shortcut: `d` then `n` clears every other filter and shows today + overdue. Gated on the "Show Today quick-filter button" setting.
- The card detail header gains a `⧉` copy-content button that copies the card's full markdown to the clipboard, with a toast preview (matching the `y` copy-ID UX).
- Per-card and per-tag version-history rows show operation badges — Content / Priority / Tags / Blazed / Extinguished / Due / Created / Touched for cards; Title / Color / Implies / Created / Touched for tags — with a chip bar to filter by kind. `Touched` marks a version where only the timestamp changed, so those no longer render as empty rows.
- Card-move debounce: a burst of rapid moves (Shift+J / Shift+K, the detail-panel move buttons, or any rebalance) coalesces into one push after an idle window, recording one history entry per burst. Controlled by `priority_debounce_enabled` (default on) and `priority_debounce_delay_ms` (default `3000`) in Settings → "Card-move debounce". Env vars: `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_ENABLED`, `BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS`.
- A pending card-move burst is flushed and pushed on tab hide/unload and on navigation (selecting another entity, opening the editor, or starting a new card/tag), so closing a tab or a quick `j` / click / Escape loses at most one burst.

### Changed

- **Breaking:** the auto-sync interval is renamed and rescaled to milliseconds: `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL` (seconds, default `10`) → `BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS` (ms, default `10000`); the `auto_sync_interval` `/config` key and matching localStorage key rename in lockstep (old key swept on startup). The settings input now takes 5000–300000 ms; the indicator still shows whole seconds.
- The "No tags" filter is now incompatible with AND, NOR, and NAND (previously only AND): switching to any non-OR mode clears it, and enabling "No tags" resets the mode to OR.
- The card editor auto-focuses the textarea when editing existing cards too (previously new-card only), so `Ctrl+S` save-and-stay keeps focus.
- Swipe settings are reorganized into Right swipe / Left swipe / Undo subsections, with one trigger field per mode (Cycle / Levels) greyed out unless active.
- **Breaking:** swipe trigger env vars renamed for symmetry: `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT` → `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_CYCLE`, `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT` → `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_CYCLE`; new `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS` (default 135 px) and `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS` (default 95 px) give each mode its own trigger. Matching localStorage keys rename in lockstep, so existing thresholds reset to the new defaults.
- **Breaking:** the general-purpose push-debounce is gone — only card moves are debounced now; every other edit (checkbox toggles, due-date changes, content saves, tag changes, blaze toggles, swipe actions) pushes immediately. Removed env vars `BLAZELIST_DEFAULT_DEBOUNCE_ENABLED` / `BLAZELIST_DEFAULT_DEBOUNCE_DELAY`, their localStorage keys (swept on startup), and the `debounce_enabled` / `debounce_delay` `/config` keys — replaced by the card-move debounce (see Added).
- Card detail header icons are reflowed into one row; on phones (≤ 480 px) the actions wrap above the status badge when they don't fit.
- In card/tag detail, history rows and filter chips align with the metadata labels above, and each version row's click target and hover/expanded background extend to the section edges.
- Raised the WASM shadow stack from 1 MiB to 8 MiB to stop deep debug-build render paths (the detail panel especially) from overflowing it and crashing; `--release` builds were unaffected.

### Removed

- **Breaking:** auto-save while editing (with its countdown / "Saving" / "Saved" indicator and the `Delay (seconds)` setting) is gone — saving is now always explicit (Save / Update button or `Ctrl+S`). Removed env vars `BLAZELIST_DEFAULT_AUTO_SAVE` / `BLAZELIST_DEFAULT_AUTO_SAVE_DELAY` and their localStorage keys (swept on startup).

### Fixed

- Transitive (recursive) link counts refresh immediately after an edit instead of sometimes needing a restart, including cards pulled in when an edit links two previously separate chains together.
- The tag filter no longer auto-expands implied tags — it mirrors the URL exactly and a chip's `x` removes only that tag (previously, navigating back added implied tags as chips and removing one wiped the rest; the card-editor cascade is unchanged).
- GFM markdown tables (card content, version-history and editor previews) no longer render as blank cells — they now have cell padding, bold headers, and a border around every cell.
- Saving a card (`Ctrl+S` / Save) is skipped when content, tags, and due date are unchanged, avoiding version-history entries that differ only in timestamp; new-card creation is unaffected.

## [3.0.0] - 2026-05-04

### Added

- **Tag implications:** the tag detail view gains an "Implies" section for adding direct parent tags, with a live preview of every affected card (missing chips highlighted). Save runs local cycle detection, then applies the new tag version plus every affected card version atomically.
- The card editor auto-cascades transitively-implied tags: toggling a tag on adds its implied tags, and removing a chip also removes anything that transitively requires it (tags that don't depend on it stay).
- A local cache schema stamp (`wasm_version` + `protocol_version`) is checked at load time, before any network activity, so client upgrades wipe incompatible caches even offline; the offline queue is preserved so unsynced edits survive the upgrade.
- `Ctrl+Enter` / `Cmd+Enter` saves/creates the active card.
- `Enter` in the tag search input toggles the first matching tag.
- Multi-level swipe-left gesture for due dates: cycles today → tomorrow → in 2 days → clear based on the card's current due date.
- Keyboard sub-menus for due date (`d`), sort (`s`), and linked cards (`l`), with `q`/`Esc` to cancel.
- Direct keyboard shortcuts: `a`/`A`/`b` blaze filter, `v`/`V` tag-filter mode / "no tags", `i` include-overdue, `x` toggle sidebar, `r` reset all filters, `f`/`/` focus search, `F` focus tag search, `h` browser back, `y` copy card ID, `Y` new tag.
- `Enter` in sidebar tag search toggles the first matching tag filter and blurs the input; `Esc` blurs without toggling.
- Prev/next buttons (‹/›) in the card detail header, mirroring `k`/`j`, disabled at list boundaries.
- Pre-fetch all card/tag/sequence history during sync for full offline access.
- Today quick-filter button beside the due-date dropdown.
- Due-date dropdowns (card editor and detail) now list every weekday — "Next tuesday/wednesday/thursday/saturday/sunday" join the existing "Next monday"/"Next friday".
- Full link-graph cache computed in the background and persisted across reloads, so card detail shows linked cards instantly; invalidated on content changes.
- "Show transitive link counts in card list" setting (default on) — `⋯N` indicators.
- Linked-card filter dropdown in card detail: forward only (→), back only (←), or direct (↔), alongside the existing "all linked".
- "Recursive linked cards" setting (default on) — card detail transitively expands all linked cards through forward and back links, deduplicated.
- Blaze-status highlighting on linked cards in card detail.
- "Local Cache" section in sidebar stats: history-cache sizes, link-graph progress (N/M with percentage and bar), and offline-queue status.
- Swipe-action toast with undo for mobile touch gestures.
- "Copied ID" toast on the `y` shortcut and the copy-ID button (first 8 chars of the UUID, auto-dismisses after 1.5 s).
- Configurable swipe-undo toast timeout (setting + `BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS`).
- Consistent date/time display showing the full UTC value inline with a hover tooltip.
- "Show card last-edited time" setting (default off) — the "x ago" label on card rows is hidden by default; opt in via the setting or `BLAZELIST_DEFAULT_SHOW_CARD_TIME`.
- Detail-panel expand/collapse with a `⛶ / ⤢` header toggle and `m` / `M` shortcuts (phones default expanded, desktop collapsed); the old tag-filter-mode bindings move to `v` / `V`.
- The expanded detail panel uses native page scroll instead of an internal scroll container.
- "Clear local cache" button in the settings Danger Zone — wipes all cached cards, tags, history, and link-graph data and reloads for a full re-sync.
- Mutual-link indicator (`↔N`) in card rows and card detail, alongside `→N` / `←N` / `⋯N`.

### Changed

- **Breaking:** tied to protocol 3.0.0; the cache schema stamp bumps automatically, so existing installations re-sync from scratch on first launch after upgrade.
- The quick due-date preset button toggles smartly: if a card is already due today or overdue, it sets tomorrow instead of today, and the label updates to match.
- Default swipe-right (blaze) threshold is 135 px (was 100 px); swipe-left (due date) is 115 px (was 90 px).
- Swipe-undo and error toasts on phones (≤ 480 px) are wider and tighter so short messages like "Blazed 🔥" or "Due: Today" stay on one line.
- The copy-ID toast dismiss timer resets on repeated copies so back-to-back copies don't truncate it.
- The cache now loads all-or-nothing: if the main database is missing, corrupt, or empty, the derived history and link caches are cleared too, so the sidebar no longer shows the "0 cards + hundreds of cached histories" ghost state.
- Sidebar Local Cache labels renamed "Cards" → "Card Histories" and "Tags" → "Tag Histories" to clarify they count cached histories, not main-DB cards/tags.
- The card list updates reactively in place: rows persist across updates (keyed diffing), and blazed status, due date, tags, preview, and tag-color changes all propagate live instead of showing stale captured values.
- Card tag dots use a 4-dot 2×2 grid for custom-colored tags; default-colored tags roll up into a `+N` overflow count.
- The linked-cards list and summary are sorted mutual → forward → back → transitive to match the indicator order.
- `B` now blazes/extinguishes a card; `a`/`A`/`b` are direct filter shortcuts (the blaze-filter sub-menu is removed).
- Settings organized into titled sections (Sync & Saving, Editor, Search & Filtering, Linked Cards, Input, Appearance, Layout, Danger Zone).
- Each setting shows its `BLAZELIST_DEFAULT_*` env var name, consistently indented below the control.
- Sub-settings (intervals, thresholds, widths) are disabled rather than hidden when their parent toggle is off, keeping the UI stable.
- Progressive card-list rendering: the first 40 cards paint immediately and the rest fill in, keeping large lists snappy.
- Card detail restructured into uniform bordered sections: controls (actions + due date), details, linked cards, and history.
- Keyboard shortcuts ignore Ctrl/Alt/Meta so browser shortcuts (Ctrl+F, Alt+D, etc.) pass through.
- "Restore version" and "New from this version" now queue offline instead of silently requiring a live connection.
- Operations that can't be queued offline (delete card; create/edit/delete tag; fork with priority rebalance) now show a red error toast explaining the failure instead of silently doing nothing — both when already offline and when a push hits a lost connection.
- "Restore version" now immediately refreshes the card detail instead of leaving the restored content invisible until you re-select the card.
- The service worker returns a synthetic 503 on cache miss instead of throwing `TypeError: Failed to convert value to 'Response'` on cert-hash/config fetches while the server is unreachable.
- Offline edits dropped during flush reconciliation now show an error toast instead of only logging, so you know the edit was discarded.
- The Today quick-filter button now clears all other filters (blaze, tag, linked cards, search) and always includes overdue; the separate auto-include-overdue setting is removed.
- "New from this" in version history now opens the editor prefilled with the version's content, tags, and due date instead of creating a card silently.
- UUIDs inside URLs, markdown link targets, or glued to surrounding text no longer render as clickable card links — only UUIDs at the start of text or after whitespace count.
- Due-date filter: "Next 7 days" / "Next 14 days" replace "This week" / "Two weeks" and now include today; "All upcoming" is removed (bookmarked `f.due=upcoming` falls back to "Today & upcoming"), and the `U` sub-menu shortcut for "Upcoming only" is removed.

### Removed

- Removed the old connect-time cache-eviction path — the fingerprint check and its now-unused helpers — superseded by the load-time schema stamp (see Added).

### Fixed

- **PWA cold start hung indefinitely on a stalled network** (not just offline — flaky signal, VPN transitions, captive portals, or a server that accepts TCP but never responds): the Android PWA stuck on the splash screen and desktop Chrome spun forever. Navigation is now cache-first with stale-while-revalidate — cached `/index.html` serves instantly and refreshes in the background.
- Service worker install fetches each precache URL with a 15s timeout, so a hanging network during an update can't wedge the worker in the "installing" state.
- The `/config` fetch that seeds settings defaults now runs in parallel with the connection loop instead of blocking it, so a hanging `/config` no longer prevents reconnecting when the network returns (user choices in localStorage still win).
- Expanded card detail on phones no longer adds a few pixels of horizontal scroll when the content fits the viewport.
- Toggling "Recursive linked cards" now takes effect immediately instead of requiring a page reload.
- Tag detail label corrected from "Transitively implied" to "Transitively implies".
- Swipe toggle mode now sets overdue cards to today instead of skipping ahead to tomorrow.
- The swipe background for the extinguish action now shows cyan.
- Offline-created cards silently dropped by flush reconciliation.
- PWA offline cold start no longer needs navigate-away-and-back on Android — the local cache loads before the `/config` fetch, so it can't stall the splash screen offline.
- Offline card edits lost on app restart — the main database is now persisted alongside the offline queue, so changes survive closing and reopening.
- The local database is no longer wiped on a fingerprint change or root-hash mismatch when the follow-up full sync fails — the initial sync overwrites it atomically and keeps the old data as a fallback.
- Swipe due-date comparison now uses the calendar date instead of the exact time.
- Right sidebar panel overlap when opening a new tag while the shortcuts panel was visible.
- Keyboard sub-menus dismissed by modifier keys (Shift, etc.) before a capital letter could be typed.
- Long URLs and unbroken text in card content, editor preview, and version history now wrap instead of overflowing the viewport.
- Linked-card filter buttons (Filter Linked, Forward only, Back only, Direct) now clear the search query so results aren't hidden by stale search text.
- Fixed a regression that hashed every card on every edit; hashing is now scoped to the cards whose link cache could be affected.

## [2.6.0] - 2026-03-27

### Fixed

- Offline cold-start failures — partial precache success now still installs the service worker, preventing blank pages when large assets (e.g. the WASM binary) fail to download on flaky connections.
- The WASM binary (`_bg.wasm`) now uses cache-first instead of network-first, cutting latency on offline cold starts.
- Navigation fallback now tries the request URL, `/index.html`, `/`, then an inline offline page, instead of only `/index.html`.
- Cross-origin requests now pass through the service worker instead of being intercepted.
- The service worker skips waiting only when all assets cached successfully, so a partial precache failure during an update can't replace a complete cache with a broken one.
- Service worker registration errors are now logged instead of silently swallowed.
- Connection status no longer shows "Connected" before pushes can succeed, and no longer gets stuck on "Syncing" after auto or manual sync.
- Blaze/extinguish in card detail now updates the status badge and button live (via click, shortcut, or swipe) without re-rendering the whole panel.
- Due-date display in card detail now updates live (via button, preset, date picker, shortcut, or swipe); no-op guards skip duplicate versions when the due date is unchanged.

### Changed

- Card reordering is now blocked only under a non-default sort, not during search (search preserves priority order).
- Card-detail nav buttons and Shift+J / Shift+K now share one reorder check; the shortcuts previously had no reorder guard.

## [2.5.0] - 2026-03-23

### Fixed

- Offline-created cards silently dropped by flush reconciliation — new cards with the same count are no longer dropped before reaching the server.
- Cards the server doesn't have (`HashVerificationFailed`) are now recreated as a first version, preserving content, instead of getting stuck in the offline queue.

## [2.4.0] - 2026-03-17

### Added

- Copy-to-clipboard button on markdown code blocks (hover-reveal, top-right of `<pre>` blocks) in card detail, version history, and editor preview.
- Orange dotted-underline styling for markdown links in card previews, matching the card-UUID link treatment.

### Changed

- Renamed the `?card=` query parameter to `?entity=` (it selects both cards and tags).

### Fixed

- Loading screen no longer shows scrollbars from the default body margin.
- Periodic sync no longer destroys the card detail view or closes version history; card and tag history also re-fetch on connection-status changes so history loads after connecting on reload.
- Fenced code blocks no longer show a double-stacked dark background in card detail and version history.

## [2.3.1] - 2026-03-16

### Fixed

- The unsaved-changes indicator on tag creation and editing no longer stays dirty after reverting inputs to their original values.

## [2.3.0] - 2026-03-16

### Fixed

- Periodic sync no longer destroys the tag detail panel or wipes in-progress title renames and color changes.
- Setting a tag color no longer discards an in-progress title rename.

### Changed

- Unified tag editing: title and color are edited together with shared Save / Cancel, protected by the same unsaved-changes guard and `beforeunload` confirmation as cards.
- New tag creation shows an `(unsaved)` indicator and prompts before discarding via Cancel or close.

## [2.2.1] - 2026-03-15

### Fixed

- The "Today + overdue" due-date filter (`f.inc_overdue`) is now restored from the URL on page reload instead of resetting to off.

## [2.2.0] - 2026-03-15

### Added

- UI scale setting is now a number input (50–300%).
- Configurable swipe trigger distance per direction (default 100 px right, 90 px left; range 40–150 px), shown when touch swipe is enabled. Env vars: `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT`, `BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT`.
- "Clear tag search on select" setting (default on) — clears the tag search input in the sidebar and card editor after clicking a tag. Env var: `BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH`.
- Override sidebar/detail-panel width settings — each a toggle that reveals a width input when enabled (default off). Env vars: `BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH`, `BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH`, `BLAZELIST_DEFAULT_SIDEBAR_WIDTH`, `BLAZELIST_DEFAULT_DETAIL_WIDTH`.
- Conditional tooltips on card previews and sidebar tag names — shown only when the text is truncated.
- "Reset all settings to defaults" button at the bottom of settings; clears saved preferences and reloads.
- Full offline PWA startup — the service worker precaches all app assets so the UI loads instantly without a network connection.

### Changed

- Touch swipe uses rubber-band physics — 1:1 movement until the threshold, then diminishing drag, with the action label and background fading in and committing only at the threshold.
- Sidebar and detail panel resize to wider ranges (sidebar 80–500 px, detail 200–1400 px).
- Card-list left accent bar: green for active, red for blazed, brighter when selected.
- Sidebar tag names truncate with ellipsis instead of wrapping, showing full text when the sidebar is wider.

### Fixed

- Saves during the initial sync no longer race against stale cached ancestor hashes — the client isn't exposed until sync completes, so saves go to the offline queue and reconcile afterward.
- A stale client from a previous connection is cleared on reconnect, preventing pushes through a dead transport.
- Live card/version pushes now rebase on an ancestor-hash mismatch instead of queuing, applying on top of the server's latest version.
- Offline-queue flush also rebases on an ancestor-hash mismatch instead of dropping queued cards.
- New-card auto-save now works offline instead of silently failing when disconnected.
- Offline-queue updates hitting `DuplicatePriority` now rebase on the server version instead of looping forever.
- The offline queue no longer drops cards on unhandled push errors — only `AlreadyDeleted` cards are dropped; all others stay queued for retry.
- Sync failures now show an error in the sync-indicator bar, clearing on the next successful sync.

## [2.1.1] - 2026-03-15

### Fixed

- Offline-queue flush re-adds pushed cards to local state (persisted to OPFS), so cards no longer disappear until reload after reconnect.
- App header wraps gracefully on small viewports with consistent row spacing.

## [2.1.0] - 2026-03-15

### Added

- "Include tags in search" setting (default on) — search matches card content and tag names (the "no tags" filter is excluded). Env var: `BLAZELIST_DEFAULT_SEARCH_TAGS`.
- UI scale setting (75%–200%) to resize the whole interface. Env var: `BLAZELIST_DEFAULT_UI_SCALE`.
- UI density setting: compact (default) or cozy (larger tag dots, more spacing). Env var: `BLAZELIST_DEFAULT_UI_DENSITY`.
- Due-date keyboard shortcuts: `t` (today), `T` (tomorrow), `C` (clear).
- Keyboard shortcut `,` to open settings.
- Keyboard shortcuts shown as a normal pane (like settings) instead of a popup overlay — press `?` or click "View shortcuts" in settings.
- Touch swipe gestures on cards (off by default): swipe right to blaze/extinguish, swipe left to set due today (or tomorrow if already today). Env var: `BLAZELIST_DEFAULT_TOUCH_SWIPE`.
- Auto-save for new cards — transitions to editing mode after the first save without losing editor state.
- Offline card and tag operations with a pending push queue that drains automatically on reconnect.
- Tag creation uses the same color-picker style as editing, with the default color shown as a placeholder.

### Changed

- Auto-save while editing is now disabled by default (was enabled).
- Pane transitions (settings, shortcuts, card detail) share an unsaved-changes guard that prompts before discarding edits.
- Linked-card UUID underlines are green for active cards and red for blazed cards.
- Markdown horizontal rule (`---`) styling improved with balanced spacing and a brighter color.

### Fixed

- The offline queue no longer drops cards on `DuplicatePriority` — the flush recomputes priority (rebalancing if gaps are exhausted) and retries.
- Auto-sync no longer destroys unsaved editor content.
- Query parameters update when saving a new card, keeping the URL in sync with the selected card.
- Reconnect no longer gets stuck in "Connecting…" — simplified to a fixed 5-second retry instead of exponential backoff.

### Removed

- Drag & drop card reorder option and its handlers. Removed env var: `BLAZELIST_DEFAULT_DRAG_DROP`.

## [2.0.0] - 2026-03-15

### Added

- Toggle to disable push debounce for instant card updates. Env var: `BLAZELIST_DEFAULT_DEBOUNCE_ENABLED`.

#### Offline-first storage
- Cards, tags, deleted entities, and root state persist in the browser's OPFS; the UI renders instantly from cache on startup while a WebTransport connection syncs in the background.
- Card, tag, and sequence history are cached in OPFS and render instantly, refreshing in the background.
- OPFS is now required — the app refuses to start without it (e.g. insecure context or unsupported browser).
- Requests persistent storage (`navigator.storage.persist()`) on startup to reduce eviction risk.
- Automatic `RootHashMismatch` recovery: wipes the local cache and does a full re-sync.

#### Settings & configuration
- Settings page with device-local preferences: auto-save, auto-sync, markdown preview, drag & drop reorder, and push-debounce delay.
- Auto-save for card editing with a configurable countdown timer.
- Periodic sync check with a configurable interval (default 10 s) and header countdown.
- Configurable push-debounce delay (default off; 5 s when enabled) with header countdown.
- Server-side default settings via the `/config` endpoint (`BLAZELIST_DEFAULT_*` env vars); priority chain is localStorage > server env var > hardcoded default. Adds `BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS`.

#### Editor
- Searchable tag lists in the card editor and sidebar.
- Tag panel sits beside the editor textarea on wide viewports, matching its height.
- Inline linked-card previews for UUID references (short UUID + card title).
- Unsaved-changes guard with `beforeunload` confirmation.

#### Navigation & filtering
- Browser back/forward navigation.
- Due-date filter dropdown (Overdue / Today / Today & upcoming / Upcoming) with an include-overdue toggle.
- Sort by due date (ascending and descending).
- Active accent styling on the sort dropdown and search input when filters are set.
- "Filter by due date" placeholder when no due-date filter is active.

#### Keyboard & interaction
- Keyboard shortcuts (can be disabled in settings; `?` always shows help).
- Move card up/down via Shift+J / Shift+K.
- First/last card via `g` / `G`.
- New card at bottom/top via `n` / `N`, above/below selected via `o` / `O`.
- Enter in search selects the first filtered card.
- New-card button dropdown with placement options (bottom, top, above/below selected); new cards are auto-selected, with a position hint on the new-card page (e.g. "Adding below …").
- Device-local drag & drop card reorder toggle.
- Two-step tag deletion with inline error display.

#### Sync indicator
- Push-debounce and auto-sync countdowns in the header.
- Sync duration and operation count in sidebar stats.
- Reconnect countdown with click-to-reconnect.

### Changed

- Removed the position dropdown from the editor's save button — position is chosen before opening the editor (button dropdown or shortcut).
- Replaced exponential backoff with a fixed 5-second reconnect retry, shown in the sync indicator with click-to-reconnect.
- Tag deletion atomically removes the tag from all referencing cards before deleting.
- App loads from OPFS before entering the connection loop, using incremental sync when cached state exists.
- Renamed "Auto-sync with server" to "Periodic sync check".
- Split sidebar stats into Total Cards, Active Cards, Blazed Cards, Tags, Deleted Entities, Total Entities.
- Last Sync timestamp updates every second and always shows seconds.
- Relative-timestamp refresh interval changed from 30 s to 1 s.
- Card priority now uses the full `i64` range (was `NonNegativeI64`).

### Fixed

- Sticky hover states on tag elements on touch devices.
- Sync-indicator layout jump when the refresh icon appeared/disappeared.

## [1.2.0] - 2026-03-07

### Added

- App icons (favicon, Apple touch icon, 192px/512px PWA icons) from the new BlazeList logo, plus PWA manifest icon entries for installable-app support.

## [1.1.0] - 2026-03-07

### Added

- Automatic reconnection with exponential backoff (1 s → 30 s) when the connection is lost, reset on success.
- Returning to the tab or coming back online interrupts the backoff and reconnects immediately.
- Click-to-reconnect on the sync indicator when disconnected.

### Fixed

- "Add to top" placed the card partway down the list instead of at the top.
- Card moves (top / up / down / bottom) used the unfiltered list, mispositioning cards when filters were active.

### Changed

- Larger button and input touch targets on small viewports for easier tapping on mobile.

## [1.0.0] - 2026-03-07

### Added

#### Connection & sync
- WebTransport connection with self-signed certificate support (SHA-256 cert hash fetched from the server's HTTP endpoint).
- Protocol version handshake on connect.
- Initial full sync and incremental sync via `GetChangesSince`.
- Real-time subscription stream — automatic incremental sync on every server mutation.
- Connection status indicator (Connected / Connecting / Syncing / Disconnected) with a manual sync button.

#### Cards
- Create cards with top or bottom placement.
- Edit content, tags, and due date with live markdown preview.
- Delete cards (with confirmation).
- Toggle blaze status (active / blazed).
- Move cards to top / up / down / bottom or jump to a specific position, all using the filtered list order.
- Priority placement with automatic gap rebalancing when priority space is exhausted.
- Card version history viewer with restore and fork.
- Copy card ID to clipboard.
- Debounced content auto-sync (1000 ms) with pending-version batching to reduce server round-trips.

#### Markdown
- Full GitHub Flavored Markdown (strikethrough, tables, autolinks, task lists).
- Interactive task-list checkboxes — click a list item to toggle.
- Live split-pane editor with preview toggle.
- Card UUID references in content rendered as clickable links.

#### Tags
- Create tags from the sidebar, rename inline with save/cancel, and delete with confirmation.
- Assign and clear custom RGB colors via hex input and color picker.
- Tag version history viewer.
- Filter by tags: multi-select with AND/OR toggle, plus a "no tags" filter for untagged cards.
- Tag color dots on card rows (grid of up to 9, `+N` overflow).

#### Due dates
- Set due dates via quick presets (Today, Tomorrow, Next Monday, Next Friday), a dropdown, or a native date picker; clear due dates.
- Due-date badges with relative status and color coding (overdue / today / upcoming).
- Filter by due-date status (Overdue / Today / Upcoming).

#### Linked cards
- Bidirectional linking: forward links (UUIDs in content), inferred back links, and mutual links.
- Linked-cards section in the detail panel with direct navigation.
- Forward/back link-count indicators on card rows.
- Filter to show a source card and all its linked cards.

#### Filtering & sorting
- Status filter (Active / All / Blazed).
- Full-text search across card content with clear button.
- Sort by priority, creation date, modification date, or due date.
- All filters compose and persist in URL query parameters (browser back/forward compatible).

#### Layout & responsiveness
- Three-panel layout: resizable sidebar, card list, and detail panel, with drag-to-resize handles and min/max constraints.
- Sidebar hidden by default below 768 px, toggled via hamburger menu with overlay.
- Adaptive initial detail-panel width (50% of viewport, clamped 280–800 px).

#### Sidebar
- Alphabetically sorted tag list with a manage button per tag.
- Statistics: root hash, sequence number, card counts (total / active / blazed), tag count, deleted-entity count, and last sync time.
- Expandable sequence history with per-entry operation details and clickable entity navigation.

#### Card list
- Card preview truncated to the first 200 chars, with zero-padded index numbers and a task-progress indicator (completed / total).
- Modified timestamp in relative format (auto-refreshed every 5 s).

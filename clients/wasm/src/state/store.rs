pub use crate::state::drag::DropEdge;
use crate::state::query_params::{
    get_query_params, parse_due_date_filter_from_params, parse_filter_from_params,
    parse_linked_cards_from_params, parse_no_tags_from_params, parse_selected_card_from_params,
    parse_sort_from_params, parse_tag_mode_from_params, parse_tags_from_params,
};
use crate::state::settings;
use crate::transport::client::Client;
use blazelist_client_lib::filter;
pub use blazelist_client_lib::filter::DueDateFilter;
pub use blazelist_client_lib::filter::SortOrder;
pub use blazelist_client_lib::filter::TagFilterMode;
use blazelist_protocol::CardFilter;
use blazelist_protocol::{Card, Entity, RootState, Tag, Utc};
use chrono::DateTime;
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

// Re-export moved utilities so existing imports keep working.
pub use crate::state::query_params::{restore_from_query_params, sync_query_params};
pub use blazelist_client_lib::color::tag_chip_style;
pub use blazelist_client_lib::display::format_relative_time;
pub use blazelist_client_lib::due_date::{
    DueDatePreset, DueDateStatus, due_date_status, format_due_date_badge, format_due_date_display,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "confirm")]
    fn js_confirm(message: &str) -> bool;
}

/// Derive the WebTransport server URL from the page's port.
///
/// Port layout (all share the same offset):
///   QUIC          47200 + offset
///   WebTransport  47400 + offset
///   HTTP cert     47600 + offset
///   Trunk         47800 + offset
///
/// So WT port = page port - 400.  Falls back to the default if the page
/// port can't be parsed (e.g. running outside the dev workflow).
fn derive_wt_url() -> String {
    const DEFAULT_WT_PORT: u16 = 47400;
    const TRUNK_TO_WT_DELTA: u16 = 400;

    let window = web_sys::window();
    let location = window.as_ref().map(|w| w.location());

    let host = location
        .as_ref()
        .and_then(|l| l.hostname().ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let wt_port = location
        .as_ref()
        .and_then(|l| l.port().ok())
        .and_then(|p| p.parse::<u16>().ok())
        .map(|trunk_port| trunk_port.wrapping_sub(TRUNK_TO_WT_DELTA))
        .unwrap_or(DEFAULT_WT_PORT);

    format!("https://{host}:{wt_port}")
}

/// Returns `true` if there are no unsaved changes, or the user confirms discard.
pub fn confirm_discard_changes(state: &AppState) -> bool {
    if state.has_unsaved_changes.get_untracked() {
        js_confirm("You have unsaved changes. Discard them?")
    } else {
        true
    }
}

/// Fire-and-forget flush of any in-flight priority-move burst. No-op when
/// no burst is pending. Returns `true` if a burst was flushed.
///
/// Called by the selection / editor helpers below before mutating any of
/// the `pub(in crate::state)` signals (`selected_card`, `editing`,
/// `creating_new`, `creating_new_tag`), which is why those fields are
/// visibility-restricted in the first place.
#[cfg(target_arch = "wasm32")]
pub(in crate::state) fn flush_pending_priority(state: &AppState) -> bool {
    let had_burst = state
        .pending_priority
        .with_untracked(|burst| burst.is_some());
    crate::state::pending_priority::flush(state);
    debug_assert!(
        state.pending_priority.with_untracked(Option::is_none),
        "flush_pending_priority did not clear pending burst"
    );
    had_burst
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::state) fn flush_pending_priority(_state: &AppState) -> bool {
    false
}

/// Change `selected_card` (card *or* tag UUID; `None` deselects).
/// Guards unsaved changes, flushes any pending priority burst, and
/// resets editing/creation/settings/shortcuts state. Returns `false`
/// if the user cancels the discard prompt.
pub fn set_selection(state: &AppState, target: Option<Uuid>) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    flush_pending_priority(state);
    state.selected_card.set(target);
    state.editing.set(false);
    state.creating_new.set(false);
    state.creating_new_tag.set(false);
    state.settings_open.set(false);
    state.shortcuts_open.set(false);
    sync_query_params(state);
    true
}

/// **Escape hatch — prefer [`set_selection`].** Sets `selected_card`
/// without the discard prompt or burst flush. Two legitimate callers:
/// `restore_from_query_params` (popstate; `CardDetail`'s `on_cleanup`
/// still flushes when the old panel unmounts) and `version_history`'s
/// force-rerender toggle (UUID is unchanged, so the burst still belongs
/// to the same card).
pub(crate) fn set_selection_without_flush(state: &AppState, target: Option<Uuid>) {
    state.selected_card.set(target);
}

/// Open the new-card editor at `position`. Guards unsaved changes,
/// flushes any pending priority burst, clears any existing selection,
/// closes settings/shortcuts panes, and sets `creating_new = true`.
/// Returns `false` if the user cancels.
/// Write `value` only when it actually changes the signal. Leptos
/// notifies subscribers on every `set` — including no-op writes like
/// `false -> false` — and the pane-switch closure in `home.rs` tracks
/// `settings_open`/`shortcuts_open`, so a redundant write there rebuilds
/// the whole `CardDetail` subtree. That bounce unmounts a just-mounted
/// `CardEditor`, whose `on_cleanup` clears the single-shot
/// `new_card_prefill` — silently blanking a "New from this" fork.
fn set_if_changed(signal: RwSignal<bool>, value: bool) {
    if signal.get_untracked() != value {
        signal.set(value);
    }
}

pub fn start_new_card(state: &AppState, position: NewCardPosition) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    flush_pending_priority(state);
    // Open the editor pane before clearing the selection so the
    // `detail_open` memo never passes through a transient closed state,
    // and use `set_if_changed` for the pane flags so a redundant
    // `false -> false` notify can't remount the editor (see above) and
    // drop the fork prefill.
    state.new_card_position.set(position);
    state.creating_new.set(true);
    state.selected_card.set(None);
    set_if_changed(state.editing, false);
    set_if_changed(state.creating_new_tag, false);
    set_if_changed(state.settings_open, false);
    set_if_changed(state.shortcuts_open, false);
    sync_query_params(state);
    true
}

/// Open the new-tag form. Guards unsaved changes, flushes any pending
/// priority burst, clears any existing selection, and sets
/// `creating_new_tag = true`. Returns `false` if the user cancels.
pub fn start_new_tag(state: &AppState) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    flush_pending_priority(state);
    // Same ordering + `set_if_changed` rationale as `start_new_card`:
    // open the new pane first, and avoid redundant pane-flag notifies
    // that would remount the form mid-flow.
    state.creating_new_tag.set(true);
    state.selected_card.set(None);
    set_if_changed(state.creating_new, false);
    set_if_changed(state.editing, false);
    set_if_changed(state.settings_open, false);
    set_if_changed(state.shortcuts_open, false);
    sync_query_params(state);
    true
}

/// Flip `editing = true` on the currently-selected card. Flushes any
/// pending priority burst first. No-op when no card is selected.
pub fn open_editor(state: &AppState) {
    if state.selected_card.with_untracked(Option::is_none) {
        return;
    }
    flush_pending_priority(state);
    state.editing.set(true);
}

/// Flip `editing = false`. Guards unsaved changes, flushes the priority
/// burst, and clears `has_unsaved_changes`. Returns `false` if the user
/// cancels.
pub fn close_editor(state: &AppState) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    flush_pending_priority(state);
    state.editing.set(false);
    state.has_unsaved_changes.set(false);
    true
}

/// Drop the creation flags and any prefill, without selecting a new
/// entity. Callers must guard unsaved changes themselves; no flush
/// because a not-yet-created card can't have a priority burst.
pub fn finish_creation_flow(state: &AppState) {
    state.creating_new.set(false);
    state.creating_new_tag.set(false);
    state.new_card_prefill.set(None);
}

thread_local! {
    static CLIENT: RefCell<Option<Rc<Client>>> = const { RefCell::new(None) };
}

pub fn set_client(client: Rc<Client>) {
    CLIENT.with(|c| *c.borrow_mut() = Some(client));
}

pub fn clear_client() {
    CLIENT.with(|c| *c.borrow_mut() = None);
}

/// Initial value for `AppState::detail_expanded` on mount. Narrow
/// viewports (≤ 768 px) start expanded so a phone user lands on the
/// same full-screen detail experience as before; wider viewports
/// start collapsed so desktop users keep the familiar side-panel
/// layout. Falls back to `false` if the window isn't available.
fn initial_detail_expanded_from_viewport() -> bool {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .map(|width| width <= 768.0)
        .unwrap_or(false)
}

pub fn get_client() -> Option<Rc<Client>> {
    CLIENT.with(|c| c.borrow().clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Syncing,
}

/// Where a newly created card should be placed in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCardPosition {
    Top,
    Bottom,
    /// Insert above the card with this UUID.
    Above(Uuid),
    /// Insert below the card with this UUID.
    Below(Uuid),
}

/// Type alias for the link graph cache: card ID → (blake3 content hash, reachable card IDs).
pub type LinkGraphCache = HashMap<Uuid, ([u8; 32], Vec<Uuid>)>;

/// Which keyboard sub-menu is currently open. When `Some`, the next keypress
/// is dispatched to the sub-menu handler instead of the normal shortcut map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubMenu {
    /// Due-date filter sub-menu.
    DueDateFilter,
    /// Sort order sub-menu.
    Sort,
    /// Linked-cards filter sub-menu.
    LinkedCards,
    /// Tag filter mode sub-menu (OR / AND / NOR / NAND).
    TagFilterMode,
}

/// Toast notification shown after a swipe action, allowing the user to undo.
#[derive(Clone)]
pub struct SwipeToast {
    /// Human-readable description of the action, e.g. "Blazed 🔥".
    pub message: String,
    /// Card state *before* the swipe action, used to revert on undo.
    pub original_card: Card,
    /// JS `setTimeout` handle so the timer can be cleared on undo.
    pub timeout_handle: i32,
}

/// Initial values for a *new* card, prefilled from another source (e.g. the
/// "New from this" version-history action). Consumed by the card editor
/// when `creating_new` becomes true.
#[derive(Clone, Debug)]
pub struct NewCardPrefill {
    pub content: String,
    pub tags: Vec<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
}

/// Global application state, provided via Leptos context.
#[derive(Clone, Copy)]
pub struct AppState {
    pub cards: RwSignal<Vec<Card>>,
    pub tags: RwSignal<Vec<Tag>>,
    pub root: RwSignal<Option<RootState>>,
    pub filter: RwSignal<CardFilter>,
    pub due_date_filter: RwSignal<DueDateFilter>,
    pub include_overdue: RwSignal<bool>,
    pub sort_order: RwSignal<SortOrder>,
    pub tag_filter: RwSignal<Vec<Uuid>>,
    pub tag_filter_mode: RwSignal<TagFilterMode>,
    pub no_tags_filter: RwSignal<bool>,
    pub search_query: RwSignal<String>,
    /// Currently selected card or tag UUID (`None` = empty detail panel).
    /// Visibility-restricted; mutate via [`set_selection`] (or, rarely,
    /// [`set_selection_without_flush`]).
    pub(in crate::state) selected_card: RwSignal<Option<Uuid>>,
    pub sidebar_visible: RwSignal<bool>,
    pub sidebar_width: RwSignal<f64>,
    pub detail_width: RwSignal<f64>,
    pub connection_status: RwSignal<ConnectionStatus>,
    pub server_url: RwSignal<String>,
    /// Visibility-restricted; mutate via [`start_new_card`].
    pub(in crate::state) creating_new: RwSignal<bool>,
    /// Visibility-restricted; mutate via [`start_new_tag`].
    pub(in crate::state) creating_new_tag: RwSignal<bool>,
    /// Optional prefilled values for the next new card. Cleared when the
    /// creating_new flag is lowered (save or cancel).
    pub new_card_prefill: RwSignal<Option<NewCardPrefill>>,
    /// Visibility-restricted; mutate via [`open_editor`] / [`close_editor`].
    pub(in crate::state) editing: RwSignal<bool>,
    pub has_unsaved_changes: RwSignal<bool>,
    pub last_synced: RwSignal<Option<DateTime<Utc>>>,
    /// Number of operations in the last sync (cards + tags + deletes).
    pub last_sync_ops: RwSignal<usize>,
    pub deleted_count: RwSignal<usize>,
    pub tick: RwSignal<u64>,
    /// Seconds remaining until the next automatic reconnection attempt.
    /// `0` means no countdown is active.
    pub reconnect_countdown: RwSignal<u32>,
    /// Duration of the last sync in milliseconds.
    pub last_sync_duration_ms: RwSignal<Option<u32>>,
    /// When set, the filtered view shows only cards whose UUIDs are in this list.
    /// Used for "show linked cards" — contains the source card + its linked UUIDs.
    pub linked_card_filter: RwSignal<Vec<Uuid>>,
    /// Device-local setting: show markdown preview by default when editing.
    pub show_preview: RwSignal<bool>,
    /// Device-local setting: primary toggle for the card-move debounce.
    /// When `false`, every move pushes immediately.
    pub priority_debounce_enabled: RwSignal<bool>,
    /// Device-local setting: delay (ms) that a card-move burst waits
    /// before flushing as a single coalesced push. Ignored when
    /// `priority_debounce_enabled` is `false`.
    pub priority_debounce_delay_ms: RwSignal<u32>,
    /// Whether the settings panel is open (shown in the detail panel area).
    pub settings_open: RwSignal<bool>,
    /// Device-local setting: periodically sync with server.
    pub auto_sync_enabled: RwSignal<bool>,
    /// Device-local setting: milliseconds between auto-syncs.
    pub auto_sync_interval_ms: RwSignal<u32>,
    /// Countdown to next auto-sync in milliseconds (0 = inactive/just synced).
    pub auto_sync_countdown_ms: RwSignal<u32>,
    /// Milliseconds remaining until the in-flight priority burst flushes.
    /// `0` = idle. Re-armed to the configured delay on each new move.
    pub priority_burst_countdown_ms: RwSignal<u32>,
    /// Device-local setting: enable keyboard shortcuts.
    pub keyboard_shortcuts_enabled: RwSignal<bool>,
    /// Device-local setting: include tags in search.
    pub search_tags: RwSignal<bool>,
    /// Device-local setting: UI scale percentage (100 = default).
    pub ui_scale: RwSignal<u32>,
    /// Device-local setting: UI density ("compact" or "cozy").
    pub ui_density: RwSignal<String>,
    /// Whether the keyboard shortcuts pane is open.
    pub shortcuts_open: RwSignal<bool>,
    /// In-flight priority-move burst (`None` = no burst pending). Only
    /// one burst exists at a time. Visibility-restricted; mutate via
    /// [`crate::state::pending_priority`] (`enqueue_move` / `flush` /
    /// `discard_for`).
    pub(in crate::state) pending_priority:
        RwSignal<Option<crate::state::pending_priority::PriorityBurst>>,
    /// `setTimeout` handle for the active priority-burst timer
    /// (`0` = no timer armed).
    pub priority_burst_timer_handle: RwSignal<i32>,
    /// Where the next new card should be placed.
    pub new_card_position: RwSignal<NewCardPosition>,
    /// Cards queued for push while offline. Flushed on reconnect.
    pub offline_queue: RwSignal<Vec<Card>>,
    /// Device-local setting: enable touch swipe gestures on cards.
    pub touch_swipe_enabled: RwSignal<bool>,
    /// Device-local setting: swipe right trigger threshold in px in
    /// `cycle` swipe-left mode.
    pub swipe_threshold_right_cycle: RwSignal<u32>,
    /// Device-local setting: swipe right trigger threshold in px in
    /// `levels` swipe-left mode.
    pub swipe_threshold_right_levels: RwSignal<u32>,
    /// Device-local setting: swipe left trigger threshold in px (used in
    /// `cycle` swipe-left mode).
    pub swipe_threshold_left_cycle: RwSignal<u32>,
    /// Device-local setting: swipe left trigger threshold in px in
    /// `levels` swipe-left mode. Doubles as the start of the Today zone —
    /// the additive zone widths extend outward from this point.
    pub swipe_threshold_left_levels: RwSignal<u32>,
    /// Device-local setting: swipe undo toast dismiss timeout in milliseconds.
    pub swipe_undo_timeout_ms: RwSignal<u32>,
    /// Device-local setting: swipe-left interaction mode.
    /// `"levels"` = swipe distance picks the action; `"cycle"` = each swipe
    /// advances through today / tomorrow / in-2-days / clear.
    pub swipe_left_mode: RwSignal<String>,
    /// Device-local setting: width (px) of the Today zone in levels-mode
    /// swipe-left. Zones extend outward from
    /// `swipe_threshold_left_levels` and are additive (Tomorrow zone
    /// starts at `threshold_l_levels + this_width`).
    pub swipe_levels_zone_today_width: RwSignal<u32>,
    /// Device-local setting: width (px) of the Tomorrow zone in
    /// levels-mode swipe-left.
    pub swipe_levels_zone_tomorrow_width: RwSignal<u32>,
    /// Device-local setting: width (px) of the In-2-days ("Soon") zone in
    /// levels-mode swipe-left. Beyond this zone the swipe enters the
    /// open-ended Clear-due region.
    pub swipe_levels_zone_soon_width: RwSignal<u32>,
    /// Last sync error message, displayed in the sync indicator.
    pub last_sync_error: RwSignal<Option<String>>,
    /// Device-local setting: clear tag search input after selecting a tag.
    pub clear_tag_search: RwSignal<bool>,
    /// Device-local setting: default sidebar width in px.
    pub default_sidebar_width: RwSignal<u32>,
    /// Device-local setting: default detail panel width in px (0 = auto).
    pub default_detail_width: RwSignal<u32>,
    /// Device-local setting: whether to override the default sidebar width.
    pub override_sidebar_width: RwSignal<bool>,
    /// Device-local setting: whether to override the default detail panel width.
    pub override_detail_width: RwSignal<bool>,
    /// Active swipe-action toast (message + undo state). `None` = hidden.
    pub swipe_toast: RwSignal<Option<SwipeToast>>,
    /// Device-local setting: show the Today quick-filter button.
    pub show_due_today_button: RwSignal<bool>,
    /// Device-local setting: expand linked cards recursively.
    pub recursive_links: RwSignal<bool>,
    /// Show transitive link count indicators in the card list.
    pub show_list_link_counts: RwSignal<bool>,
    /// Device-local setting: show the card-list relative-time label
    /// ("x ago") at all. Defaults to off — users who want it can
    /// opt in via settings.
    pub show_card_time: RwSignal<bool>,
    /// Device-local setting: extinguish a Blazed card when its due
    /// date is set or changed. Default on.
    pub extinguish_on_due_set: RwSignal<bool>,
    /// Device-local setting: also extinguish when the due date is
    /// cleared. Gated on `extinguish_on_due_set` — the parent must be
    /// on for this to fire. Default off.
    pub extinguish_on_due_clear: RwSignal<bool>,
    /// Device-local setting: clear a card's due date when blazing it.
    /// Only fires on the Extinguished → Blazed transition; blazing an
    /// already-Blazed card or extinguishing leaves the due date
    /// untouched. Default off.
    pub clear_due_on_blaze: RwSignal<bool>,
    /// Device-local setting: enable drag-and-drop card reordering.
    /// Off by default; routes the drop through the same priority-burst
    /// debounce as the Shift+J / Shift+K keyboard shortcuts.
    pub drag_and_drop_enabled: RwSignal<bool>,
    /// Device-local setting: drag-and-drop activation mode.
    /// `"anywhere"` (default, desktop-friendly) — pointerdown anywhere
    /// on the card + a small movement threshold starts the drag.
    /// `"handle"` (mobile-friendly) — only the card's leading number
    /// starts the drag, so native scroll and existing swipes still
    /// work on the rest of the card.
    pub drag_and_drop_mode: RwSignal<String>,
    /// ID of the card currently being dragged, or `None`. Session-only
    /// state — never persisted. The CardItem component reads this to
    /// dim the source via CSS and to gate the swipe handler.
    pub drag_active_card: RwSignal<Option<Uuid>>,
    /// Active drop target: the card the pointer is hovering and which
    /// edge (above / below) the source would land on. Per-card Memos
    /// on this signal mean only the formerly- and currently-targeted
    /// cards re-render on each drop-target change. Session-only.
    pub drag_drop_target: RwSignal<Option<(Uuid, DropEdge)>>,
    /// Session-only UI mode: detail panel takes the full main-layout
    /// area when true, otherwise sits as a side panel on the right.
    /// Default derives from viewport width at startup — phones get
    /// expanded, desktop gets collapsed — but the user can flip it
    /// at any time via the header toggle. Not persisted.
    pub detail_expanded: RwSignal<bool>,
    /// Background-computed link graph cache: card ID → (content_hash, reachable_ids).
    /// blake3 hash of content at computation time, used for selective invalidation.
    pub link_graph_cache: RwSignal<LinkGraphCache>,
    /// Progress of the background link computation: (processed, total). (0,0) = idle.
    pub link_cache_progress: RwSignal<(usize, usize)>,
    /// Active keyboard sub-menu (None = normal shortcut mode).
    pub sub_menu: RwSignal<Option<SubMenu>>,
    /// Set to `true` by the `d` keyboard shortcut to trigger delete confirmation
    /// in the CardDetail component. The component resets it to `false` after handling.
    pub delete_requested: RwSignal<bool>,
    /// Set to `true` by the `Ctrl+S` keyboard shortcut to trigger a
    /// save-and-stay-in-editor action while editing or creating a card.
    /// The CardEditor resets it to `false` after handling.
    pub save_requested: RwSignal<bool>,
    /// Brief toast message (e.g. "Copied!") that auto-dismisses. `None` = hidden.
    pub copy_toast: RwSignal<Option<String>>,
    /// Handle for the copy-toast auto-dismiss timeout so repeated copies reset the timer.
    pub copy_toast_timeout: RwSignal<Option<i32>>,
    /// Error toast message (e.g. "Can't delete while offline") that
    /// auto-dismisses. Rendered with a distinct, more prominent style
    /// than `copy_toast` so the user actually notices failed actions.
    pub error_toast: RwSignal<Option<String>>,
    /// Raw server config values from `/config` endpoint, keyed by config name.
    /// Used by the settings panel to show server-override layer.
    pub server_config: RwSignal<HashMap<String, String>>,
}

/// Reset all filter/view state to defaults and clear query params.
/// Prompts the user if there are unsaved changes; returns false if cancelled.
pub fn clear_all_state(state: &AppState) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    // Don't silently lose an in-flight move if the user hits reset mid-burst.
    flush_pending_priority(state);
    state.filter.set(CardFilter::Extinguished);
    state.due_date_filter.set(DueDateFilter::All);
    state.include_overdue.set(false);
    state.sort_order.set(SortOrder::default());
    state.tag_filter.set(Vec::new());
    state.tag_filter_mode.set(TagFilterMode::Or);
    state.no_tags_filter.set(false);
    state.search_query.set(String::new());
    state.selected_card.set(None);
    state.creating_new.set(false);
    state.creating_new_tag.set(false);
    state.editing.set(false);
    state.has_unsaved_changes.set(false);
    state.linked_card_filter.set(Vec::new());
    state.settings_open.set(false);
    state.shortcuts_open.set(false);
    sync_query_params(state);
    true
}

/// Apply the "Today" quick-filter: clears every other filter so the user
/// lands on a clean "what needs my attention today" view. Blaze filter
/// becomes Active, due-date becomes Today + include-overdue, and tag /
/// search / linked-card filters are all cleared. Shared by the Today
/// button in the filter bar and the `d` then `n` keyboard shortcut.
pub fn apply_today_quick_filter(state: &AppState) {
    state.filter.set(CardFilter::Extinguished);
    state.due_date_filter.set(DueDateFilter::Today);
    state.include_overdue.set(true);
    state.tag_filter.set(Vec::new());
    state.tag_filter_mode.set(TagFilterMode::Or);
    state.no_tags_filter.set(false);
    state.linked_card_filter.set(Vec::new());
    state.search_query.set(String::new());
    sync_query_params(state);
}

/// Apply a "show linked cards" filter from the already-assembled list of
/// UUIDs (`ids`). Callers build `ids` — including the source-card prepend —
/// and pass the finished `Vec`. Sets the linked-card filter to `ids`, shows
/// all cards, and clears the search / tag / no-tags filters so the linked
/// set isn't further narrowed. Shared by the "Filter Linked" button (and its
/// forward/back/direct variants) and the linked-cards keyboard sub-menu.
pub fn apply_linked_card_filter(state: &AppState, ids: Vec<Uuid>) {
    state.linked_card_filter.set(ids);
    state.filter.set(CardFilter::All);
    state.search_query.set(String::new());
    state.tag_filter.set(Vec::new());
    state.no_tags_filter.set(false);
    sync_query_params(state);
}

impl AppState {
    pub fn new() -> Self {
        let params = get_query_params();

        let viewport_width = web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(1024.0);

        let override_sidebar = settings::load_override_sidebar_width();
        let override_detail = settings::load_override_detail_width();
        let default_sidebar_w = if override_sidebar {
            settings::load_default_sidebar_width()
        } else {
            settings::DEFAULT_SIDEBAR_WIDTH
        };
        let initial_detail_width = if override_detail {
            let w = settings::load_default_detail_width();
            if w > 0 {
                (w as f64).clamp(280.0, 1200.0)
            } else {
                (viewport_width * 0.5).clamp(280.0, 800.0)
            }
        } else {
            (viewport_width * 0.5).clamp(280.0, 800.0)
        };

        // Hide sidebar by default on small viewports (matches the 768px CSS breakpoint)
        let initial_sidebar_visible = viewport_width > 768.0;

        Self {
            cards: RwSignal::new(Vec::new()),
            tags: RwSignal::new(Vec::new()),
            root: RwSignal::new(None),
            filter: RwSignal::new(parse_filter_from_params(&params)),
            due_date_filter: RwSignal::new(parse_due_date_filter_from_params(&params)),
            include_overdue: RwSignal::new(params.get("f.inc_overdue").as_deref() == Some("1")),
            sort_order: RwSignal::new(parse_sort_from_params(&params)),
            tag_filter: RwSignal::new({
                let tags = parse_tags_from_params(&params);
                if parse_no_tags_from_params(&params)
                    && !parse_tag_mode_from_params(&params).allows_no_tags()
                {
                    Vec::new()
                } else {
                    tags
                }
            }),
            tag_filter_mode: RwSignal::new({
                let mode = parse_tag_mode_from_params(&params);
                if parse_no_tags_from_params(&params) && !mode.allows_no_tags() {
                    TagFilterMode::Or
                } else {
                    mode
                }
            }),
            no_tags_filter: RwSignal::new(parse_no_tags_from_params(&params)),
            search_query: RwSignal::new(String::new()),
            selected_card: RwSignal::new(parse_selected_card_from_params(&params)),
            sidebar_visible: RwSignal::new(initial_sidebar_visible),
            sidebar_width: RwSignal::new(default_sidebar_w as f64),
            detail_width: RwSignal::new(initial_detail_width),
            connection_status: RwSignal::new(ConnectionStatus::Disconnected),
            server_url: RwSignal::new(derive_wt_url()),
            creating_new: RwSignal::new(false),
            creating_new_tag: RwSignal::new(false),
            new_card_prefill: RwSignal::new(None),
            editing: RwSignal::new(false),
            has_unsaved_changes: RwSignal::new(false),
            last_synced: RwSignal::new(None),
            last_sync_ops: RwSignal::new(0),
            deleted_count: RwSignal::new(0),
            tick: RwSignal::new(0),
            reconnect_countdown: RwSignal::new(0),
            last_sync_duration_ms: RwSignal::new(None),
            linked_card_filter: RwSignal::new(parse_linked_cards_from_params(&params)),
            show_preview: RwSignal::new(settings::load_show_preview()),
            priority_debounce_enabled: RwSignal::new(settings::load_priority_debounce_enabled()),
            priority_debounce_delay_ms: RwSignal::new(settings::load_priority_debounce_delay_ms()),
            settings_open: RwSignal::new(false),
            auto_sync_enabled: RwSignal::new(settings::load_auto_sync()),
            auto_sync_interval_ms: RwSignal::new(settings::load_auto_sync_interval_ms()),
            auto_sync_countdown_ms: RwSignal::new(0),
            priority_burst_countdown_ms: RwSignal::new(0),
            keyboard_shortcuts_enabled: RwSignal::new(settings::load_keyboard_shortcuts()),
            search_tags: RwSignal::new(settings::load_search_tags()),
            ui_scale: RwSignal::new(settings::load_ui_scale()),
            ui_density: RwSignal::new(settings::load_ui_density()),
            shortcuts_open: RwSignal::new(false),
            pending_priority: RwSignal::new(None),
            priority_burst_timer_handle: RwSignal::new(0),
            new_card_position: RwSignal::new(NewCardPosition::Bottom),
            offline_queue: RwSignal::new(Vec::new()),
            touch_swipe_enabled: RwSignal::new(settings::load_touch_swipe()),
            swipe_threshold_right_cycle: RwSignal::new(settings::load_swipe_threshold_right_cycle()),
            swipe_threshold_right_levels: RwSignal::new(
                settings::load_swipe_threshold_right_levels(),
            ),
            swipe_threshold_left_cycle: RwSignal::new(settings::load_swipe_threshold_left_cycle()),
            swipe_threshold_left_levels: RwSignal::new(settings::load_swipe_threshold_left_levels()),
            swipe_undo_timeout_ms: RwSignal::new(settings::load_swipe_undo_timeout_ms()),
            swipe_left_mode: RwSignal::new(settings::load_swipe_left_mode()),
            swipe_levels_zone_today_width: RwSignal::new(
                settings::load_swipe_levels_zone_today_width(),
            ),
            swipe_levels_zone_tomorrow_width: RwSignal::new(
                settings::load_swipe_levels_zone_tomorrow_width(),
            ),
            swipe_levels_zone_soon_width: RwSignal::new(
                settings::load_swipe_levels_zone_soon_width(),
            ),
            last_sync_error: RwSignal::new(None),
            clear_tag_search: RwSignal::new(settings::load_clear_tag_search()),
            default_sidebar_width: RwSignal::new(settings::load_default_sidebar_width()),
            default_detail_width: RwSignal::new(settings::load_default_detail_width()),
            override_sidebar_width: RwSignal::new(settings::load_override_sidebar_width()),
            override_detail_width: RwSignal::new(settings::load_override_detail_width()),
            swipe_toast: RwSignal::new(None),
            show_due_today_button: RwSignal::new(settings::load_show_due_today_button()),
            recursive_links: RwSignal::new(settings::load_recursive_links()),
            show_list_link_counts: RwSignal::new(settings::load_show_list_link_counts()),
            show_card_time: RwSignal::new(settings::load_show_card_time()),
            extinguish_on_due_set: RwSignal::new(settings::load_extinguish_on_due_set()),
            extinguish_on_due_clear: RwSignal::new(settings::load_extinguish_on_due_clear()),
            clear_due_on_blaze: RwSignal::new(settings::load_clear_due_on_blaze()),
            drag_and_drop_enabled: RwSignal::new(settings::load_drag_and_drop_enabled()),
            drag_and_drop_mode: RwSignal::new(settings::load_drag_and_drop_mode()),
            drag_active_card: RwSignal::new(None),
            drag_drop_target: RwSignal::new(None),
            detail_expanded: RwSignal::new(initial_detail_expanded_from_viewport()),
            link_graph_cache: RwSignal::new(HashMap::new()),
            link_cache_progress: RwSignal::new((0, 0)),
            sub_menu: RwSignal::new(None),
            delete_requested: RwSignal::new(false),
            save_requested: RwSignal::new(false),
            copy_toast: RwSignal::new(None),
            copy_toast_timeout: RwSignal::new(None),
            error_toast: RwSignal::new(None),
            server_config: RwSignal::new(HashMap::new()),
        }
    }

    /// Replace or insert a card in the local card list.
    pub fn upsert_card(&self, card: Card) {
        let card_id = card.id();
        self.cards.update(|cards| {
            cards.retain(|c| c.id() != card_id);
            cards.push(card);
        });
    }

    /// Whether priority reordering is allowed in the current state.
    ///
    /// Reordering only makes sense when the list is sorted by priority
    /// (the default order); any other sort would make positional moves
    /// misleading.
    pub fn reorder_allowed(self) -> bool {
        self.sort_order.get().is_default()
    }

    /// `blazed` value to use after a due-date mutation. Only ever
    /// flips Blazed → Extinguished.
    pub fn blazed_after_due_change(
        &self,
        current_blazed: bool,
        new_due: Option<DateTime<Utc>>,
    ) -> bool {
        if !current_blazed {
            return false;
        }
        let on_set = self.extinguish_on_due_set.get_untracked();
        let extinguish = if new_due.is_some() {
            on_set
        } else {
            on_set && self.extinguish_on_due_clear.get_untracked()
        };
        !extinguish
    }

    /// Due date to use after a blaze-state mutation. Mirrors
    /// [`Self::blazed_after_due_change`] in the opposite direction:
    /// only clears the due date on the Extinguished → Blazed
    /// transition, and only when the `clear_due_on_blaze` setting is
    /// enabled.
    pub fn due_after_blaze_change(
        &self,
        current_due: Option<DateTime<Utc>>,
        was_blazed: bool,
        new_blazed: bool,
    ) -> Option<DateTime<Utc>> {
        if !was_blazed && new_blazed && self.clear_due_on_blaze.get_untracked() {
            None
        } else {
            current_due
        }
    }

    // Read-only accessors for the `pub(in crate::state)` signals above:
    // components can `.get()` / `.get_untracked()` but not `.set()`.

    pub fn selected_card(&self) -> Signal<Option<Uuid>> {
        Signal::from(self.selected_card)
    }

    pub fn editing(&self) -> Signal<bool> {
        Signal::from(self.editing)
    }

    pub fn creating_new(&self) -> Signal<bool> {
        Signal::from(self.creating_new)
    }

    pub fn creating_new_tag(&self) -> Signal<bool> {
        Signal::from(self.creating_new_tag)
    }

    /// Derived signal: filtered cards based on current filter, tag selections, and search query.
    /// Cards sorted according to current sort order.
    pub fn filtered_cards(&self) -> Memo<Vec<Card>> {
        let cards = self.cards;
        let tags_signal = self.tags;
        let blaze_filter = self.filter;
        let due_date_filter = self.due_date_filter;
        let include_overdue = self.include_overdue;
        let tag_filter = self.tag_filter;
        let tag_filter_mode = self.tag_filter_mode;
        let no_tags_filter = self.no_tags_filter;
        let search_query = self.search_query;
        let sort_order = self.sort_order;
        let linked_card_filter = self.linked_card_filter;
        let search_tags = self.search_tags;

        Memo::new(move |_| {
            let mut result = cards.get();
            let all_tags = tags_signal.get();
            filter::apply_all_filters(
                &mut result,
                &linked_card_filter.get(),
                blaze_filter.get(),
                &search_query.get(),
                &tag_filter.get(),
                tag_filter_mode.get(),
                no_tags_filter.get(),
                search_tags.get(),
                &all_tags,
            );
            filter::apply_due_date_filter(
                &mut result,
                due_date_filter.get(),
                include_overdue.get(),
            );
            filter::sort_cards(&mut result, sort_order.get());
            result
        })
    }
}

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

/// Switch to viewing a specific card. Guards unsaved changes, closes any open
/// settings/shortcuts pane, clears editing state, and syncs query params.
/// Returns `false` if the user cancels (due to unsaved changes).
pub fn select_card_view(state: &AppState, card_id: Uuid) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    state.selected_card.set(Some(card_id));
    state.editing.set(false);
    state.creating_new.set(false);
    state.creating_new_tag.set(false);
    state.settings_open.set(false);
    state.shortcuts_open.set(false);
    sync_query_params(state);
    true
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

/// Auto-save status indicator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSaveStatus {
    Idle,
    Countdown(u32),
    Saving,
    Saved,
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
    pub selected_card: RwSignal<Option<Uuid>>,
    pub sidebar_visible: RwSignal<bool>,
    pub sidebar_width: RwSignal<f64>,
    pub detail_width: RwSignal<f64>,
    pub connection_status: RwSignal<ConnectionStatus>,
    pub server_url: RwSignal<String>,
    pub creating_new: RwSignal<bool>,
    pub creating_new_tag: RwSignal<bool>,
    /// Optional prefilled values for the next new card. Cleared when the
    /// creating_new flag is lowered (save or cancel).
    pub new_card_prefill: RwSignal<Option<NewCardPrefill>>,
    pub editing: RwSignal<bool>,
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
    /// Device-local setting: whether push debounce is enabled.
    pub debounce_enabled: RwSignal<bool>,
    /// Device-local setting: push debounce delay in seconds.
    pub debounce_delay_secs: RwSignal<u32>,
    /// Device-local setting: auto-save cards while editing.
    pub auto_save_enabled: RwSignal<bool>,
    /// Device-local setting: seconds to wait before auto-saving.
    pub auto_save_delay_secs: RwSignal<u32>,
    /// Auto-save status, visible globally in the header.
    pub auto_save_status: RwSignal<AutoSaveStatus>,
    /// Whether the settings panel is open (shown in the detail panel area).
    pub settings_open: RwSignal<bool>,
    /// Device-local setting: periodically sync with server.
    pub auto_sync_enabled: RwSignal<bool>,
    /// Device-local setting: seconds between auto-syncs.
    pub auto_sync_interval_secs: RwSignal<u32>,
    /// Countdown to next auto-sync (0 = inactive/just synced).
    pub auto_sync_countdown: RwSignal<u32>,
    /// Countdown to next debounced push (0 = idle).
    pub push_debounce_countdown: RwSignal<u32>,
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
    /// Pending card versions queued for debounced push.
    pub pending_versions: RwSignal<Vec<Card>>,
    /// ID of the card currently being debounced.
    pub pending_card_id: RwSignal<Option<Uuid>>,
    /// JS setTimeout handle for the active debounce timer.
    pub debounce_timeout_handle: RwSignal<i32>,
    /// Where the next new card should be placed.
    pub new_card_position: RwSignal<NewCardPosition>,
    /// Cards queued for push while offline. Flushed on reconnect.
    pub offline_queue: RwSignal<Vec<Card>>,
    /// Device-local setting: enable touch swipe gestures on cards.
    pub touch_swipe_enabled: RwSignal<bool>,
    /// Device-local setting: swipe right trigger threshold in px.
    pub swipe_threshold_right: RwSignal<u32>,
    /// Device-local setting: swipe left trigger threshold in px.
    pub swipe_threshold_left: RwSignal<u32>,
    /// Device-local setting: swipe undo toast dismiss timeout in milliseconds.
    pub swipe_undo_timeout_ms: RwSignal<u32>,
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
                    && parse_tag_mode_from_params(&params) == TagFilterMode::And
                {
                    Vec::new()
                } else {
                    tags
                }
            }),
            tag_filter_mode: RwSignal::new({
                let mode = parse_tag_mode_from_params(&params);
                if parse_no_tags_from_params(&params) && mode == TagFilterMode::And {
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
            debounce_enabled: RwSignal::new(settings::load_debounce_enabled()),
            debounce_delay_secs: RwSignal::new(settings::load_debounce_delay()),
            auto_save_enabled: RwSignal::new(settings::load_auto_save()),
            auto_save_delay_secs: RwSignal::new(settings::load_auto_save_delay()),
            auto_save_status: RwSignal::new(AutoSaveStatus::Idle),
            settings_open: RwSignal::new(false),
            auto_sync_enabled: RwSignal::new(settings::load_auto_sync()),
            auto_sync_interval_secs: RwSignal::new(settings::load_auto_sync_interval()),
            auto_sync_countdown: RwSignal::new(0),
            push_debounce_countdown: RwSignal::new(0),
            keyboard_shortcuts_enabled: RwSignal::new(settings::load_keyboard_shortcuts()),
            search_tags: RwSignal::new(settings::load_search_tags()),
            ui_scale: RwSignal::new(settings::load_ui_scale()),
            ui_density: RwSignal::new(settings::load_ui_density()),
            shortcuts_open: RwSignal::new(false),
            pending_versions: RwSignal::new(Vec::new()),
            pending_card_id: RwSignal::new(None),
            debounce_timeout_handle: RwSignal::new(0),
            new_card_position: RwSignal::new(NewCardPosition::Bottom),
            offline_queue: RwSignal::new(Vec::new()),
            touch_swipe_enabled: RwSignal::new(settings::load_touch_swipe()),
            swipe_threshold_right: RwSignal::new(settings::load_swipe_threshold_right()),
            swipe_threshold_left: RwSignal::new(settings::load_swipe_threshold_left()),
            swipe_undo_timeout_ms: RwSignal::new(settings::load_swipe_undo_timeout_ms()),
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
            detail_expanded: RwSignal::new(initial_detail_expanded_from_viewport()),
            link_graph_cache: RwSignal::new(HashMap::new()),
            link_cache_progress: RwSignal::new((0, 0)),
            sub_menu: RwSignal::new(None),
            delete_requested: RwSignal::new(false),
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

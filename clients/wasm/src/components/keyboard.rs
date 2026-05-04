//! Global keyboard shortcut handler for the WASM client.
//!
//! Shortcuts are suppressed while the user is typing in an input, textarea,
//! or contenteditable element, and can be disabled entirely in settings.
//!
//! Several keys open **sub-menus** — a small floating popup lists the
//! available follow-up keys.  Pressing one of those keys executes the
//! action and closes the sub-menu; Escape (or any unrecognised key)
//! dismisses the sub-menu without doing anything.

use crate::components::card_detail::apply_move_placement;
use crate::components::settings_panel::switch_to_pane;
use crate::state::store::{
    AppState, DueDateFilter, NewCardPosition, SortOrder, SubMenu, confirm_discard_changes,
    select_card_view, sync_query_params,
};
use crate::state::sync::push_card_or_queue;
use blazelist_client_lib::priority::{InsertPosition, move_card};
use blazelist_protocol::{CardFilter, Entity, Utc};
use chrono::Days;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// Register a global `keydown` listener that dispatches keyboard shortcuts.
///
/// Call this once from the top-level `App` component.
pub fn register_keyboard_shortcuts(state: AppState) {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let cb = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
        handle_keydown(ev, state);
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

    document
        .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
        .expect("failed to add keydown listener");
    cb.forget(); // lives for app lifetime
}

/// Returns `true` if the currently focused element is a text input, textarea,
/// or contenteditable element where keyboard shortcuts should be suppressed.
fn is_typing() -> bool {
    let active = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element());

    let Some(el) = active else {
        return false;
    };

    let tag = el.tag_name().to_uppercase();
    if tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT" {
        return true;
    }

    // contenteditable
    if let Some(attr) = el.get_attribute("contenteditable")
        && (attr == "true" || attr.is_empty())
    {
        return true;
    }

    false
}

/// Returns `true` if the currently focused element is the search input.
fn is_search_focused() -> bool {
    active_element_has_class("search-input")
}

/// Returns `true` if the currently focused element is the sidebar tag search.
fn is_tag_search_focused() -> bool {
    active_element_has_class("tag-search-input")
}

fn active_element_has_class(class: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .and_then(|el| el.dyn_into::<web_sys::Element>().ok())
        .map(|el| el.class_list().contains(class))
        .unwrap_or(false)
}

fn handle_keydown(ev: web_sys::KeyboardEvent, state: AppState) {
    let key = ev.key();

    // Escape always works, even when typing or shortcuts disabled
    if key == "Escape" {
        // If focused on any search input, just blur it
        if is_search_focused() || is_tag_search_focused() {
            blur_active_element();
            ev.prevent_default();
            return;
        }
        // If a sub-menu is open, just close it
        if state.sub_menu.get_untracked().is_some() {
            state.sub_menu.set(None);
            ev.prevent_default();
            return;
        }
        handle_escape(state);
        ev.prevent_default();
        return;
    }

    // --- Sub-menu dispatch (highest priority after Escape) ---
    if let Some(menu) = state.sub_menu.get_untracked() {
        // Dismiss on q/Esc (Esc already handled above).
        if key == "q" {
            state.sub_menu.set(None);
            ev.prevent_default();
            return;
        }
        // Ignore modifier-only keys (Shift, Control, Alt, Meta) so that
        // e.g. pressing Shift before a capital letter doesn't dismiss.
        if matches!(
            key.as_str(),
            "Shift" | "Control" | "Alt" | "Meta" | "CapsLock"
        ) {
            return;
        }
        let handled = match menu {
            SubMenu::DueDateFilter => handle_due_date_filter_submenu(&key, state),
            SubMenu::Sort => handle_sort_submenu(&key, state),
            SubMenu::LinkedCards => handle_linked_cards_submenu(&key, state),
        };
        // Close only if the key was recognized (or q/Esc above).
        // Unrecognized keys are silently ignored so the user can retry.
        if handled {
            state.sub_menu.set(None);
            ev.prevent_default();
        }
        return;
    }

    // Enter while search input is focused: blur and select first card
    if key == "Enter" && is_search_focused() {
        blur_active_element();
        select_first_card(state);
        ev.prevent_default();
        return;
    }

    // Enter while tag search is focused: toggle first matching tag and blur
    if key == "Enter" && is_tag_search_focused() {
        blur_active_element();
        ev.prevent_default();
        // The tag_sidebar's own keydown handler already toggles the tag,
        // so we just need to blur here. But if clear_tag_search is off,
        // the sidebar handler won't fire blur, so we handle it globally.
        return;
    }

    // Don't handle shortcuts when typing in inputs
    if is_typing() {
        return;
    }

    // Never intercept browser/OS shortcuts (Ctrl+F, Alt+D, Cmd+C, etc.).
    // Shift is allowed since we use capital letters (B, G, J, K, N, etc.).
    if ev.ctrl_key() || ev.alt_key() || ev.meta_key() {
        return;
    }

    // ? and , toggle panes — work even while editing (guarded by switch_to_pane)
    if key == "?" {
        if state.shortcuts_open.get_untracked() {
            state.shortcuts_open.set(false);
        } else {
            switch_to_pane(&state, false, true);
        }
        ev.prevent_default();
        return;
    }

    if key == "," {
        if state.settings_open.get_untracked() {
            state.settings_open.set(false);
        } else {
            switch_to_pane(&state, true, false);
        }
        ev.prevent_default();
        return;
    }

    if !state.keyboard_shortcuts_enabled.get_untracked() {
        return;
    }

    // Don't handle shortcuts while editing or creating
    if state.editing.get_untracked() || state.creating_new.get_untracked() {
        return;
    }

    match key.as_str() {
        // Navigation
        "j" => {
            select_next_card(state);
            ev.prevent_default();
        }
        "k" => {
            select_prev_card(state);
            ev.prevent_default();
        }

        // Go to top card
        "g" => {
            select_first_card(state);
            ev.prevent_default();
        }

        // Go to bottom card
        "G" => {
            select_last_card(state);
            ev.prevent_default();
        }

        // New card at bottom
        "n" => {
            start_new_card(state, NewCardPosition::Bottom);
            ev.prevent_default();
        }

        // New card at top
        "N" => {
            start_new_card(state, NewCardPosition::Top);
            ev.prevent_default();
        }

        // New card below selected (no-op without selection)
        "o" => {
            if let Some(id) = state.selected_card.get_untracked() {
                start_new_card(state, NewCardPosition::Below(id));
            }
            ev.prevent_default();
        }

        // New card above selected (no-op without selection)
        "O" => {
            if let Some(id) = state.selected_card.get_untracked() {
                start_new_card(state, NewCardPosition::Above(id));
            }
            ev.prevent_default();
        }

        // Create new tag
        "Y" => {
            if !confirm_discard_changes(&state) {
                ev.prevent_default();
                return;
            }
            state.selected_card.set(None);
            state.creating_new.set(false);
            state.editing.set(false);
            state.creating_new_tag.set(true);
            state.settings_open.set(false);
            state.shortcuts_open.set(false);
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Edit selected card
        "e" => {
            if state.selected_card.get_untracked().is_some() {
                state.editing.set(true);
                ev.prevent_default();
            }
        }

        // Blaze / extinguish selected card
        "B" => {
            toggle_blaze(state);
            ev.prevent_default();
        }

        // Copy selected card ID to clipboard
        "y" => {
            if let Some(id) = state.selected_card.get_untracked() {
                let id_str = id.to_string();
                copy_to_clipboard(&id_str);
                let id_preview: String = id_str.chars().take(8).collect();
                let card_preview = state
                    .cards
                    .get_untracked()
                    .iter()
                    .find(|c| c.id() == id)
                    .and_then(|c| blazelist_client_lib::display::card_preview(c.content(), 30));
                let msg = match card_preview {
                    Some(p) => format!("Copied {id_preview}\u{2026} \u{2014} {p}"),
                    None => format!("Copied {id_preview}\u{2026}"),
                };
                show_copy_toast(state, &msg);
            }
            ev.prevent_default();
        }

        // --- Direct filter shortcuts ---

        // Filter: show active (non-blazed) cards
        "a" => {
            state.filter.set(CardFilter::Extinguished);
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Filter: show all cards
        "A" => {
            state.filter.set(CardFilter::All);
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Filter: show blazed cards only
        "b" => {
            state.filter.set(CardFilter::Blazed);
            sync_query_params(&state);
            ev.prevent_default();
        }

        // --- Sub-menu openers ---

        // Open due-date-filter sub-menu
        "d" => {
            state.sub_menu.set(Some(SubMenu::DueDateFilter));
            ev.prevent_default();
        }

        // Open sort sub-menu
        "s" => {
            state.sub_menu.set(Some(SubMenu::Sort));
            ev.prevent_default();
        }

        // Focus search input
        "/" | "f" => {
            focus_search_input();
            ev.prevent_default();
        }

        // Focus sidebar tag search
        "F" => {
            focus_tag_search();
            ev.prevent_default();
        }

        // Move card up one position
        "K" => {
            move_card_up(state);
            ev.prevent_default();
        }

        // Move card down one position
        "J" => {
            move_card_down(state);
            ev.prevent_default();
        }

        // Set due date to today
        "t" => {
            set_due_date_shortcut(state, DueDateShortcut::Today);
            ev.prevent_default();
        }

        // Set due date to tomorrow
        "T" => {
            set_due_date_shortcut(state, DueDateShortcut::Tomorrow);
            ev.prevent_default();
        }

        // Clear due date
        "C" => {
            set_due_date_shortcut(state, DueDateShortcut::Clear);
            ev.prevent_default();
        }

        // Minimize detail panel (collapse to side-panel layout)
        "m" => {
            state.detail_expanded.set(false);
            ev.prevent_default();
        }

        // Maximize detail panel (expand to fullscreen layout)
        "M" => {
            state.detail_expanded.set(true);
            ev.prevent_default();
        }

        // Toggle tag-filter mode (OR / AND) — moved from `m` when the
        // detail-panel min/max shortcut took it over.
        "v" => {
            state.tag_filter_mode.update(|m| {
                *m = m.toggle();
            });
            if state.tag_filter_mode.get_untracked()
                == blazelist_client_lib::filter::TagFilterMode::And
            {
                state.no_tags_filter.set(false);
            }
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Toggle "no tags" filter — moved from `M` when the
        // detail-panel min/max shortcut took it over.
        "V" => {
            let new_val = !state.no_tags_filter.get_untracked();
            state.no_tags_filter.set(new_val);
            if new_val {
                state.tag_filter.set(Vec::new());
                state
                    .tag_filter_mode
                    .set(blazelist_client_lib::filter::TagFilterMode::Or);
            }
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Toggle include-overdue
        "i" => {
            let cur = state.include_overdue.get_untracked();
            state.include_overdue.set(!cur);
            sync_query_params(&state);
            ev.prevent_default();
        }

        // Open linked-cards filter sub-menu (requires selected card with links)
        "l" => {
            if state.selected_card.get_untracked().is_some() {
                state.sub_menu.set(Some(SubMenu::LinkedCards));
            }
            ev.prevent_default();
        }

        // Browser history back
        "h" => {
            if let Some(window) = web_sys::window() {
                let _ = window.history().and_then(|h| h.back());
            }
            ev.prevent_default();
        }

        // Toggle sidebar
        "x" => {
            let vis = state.sidebar_visible.get_untracked();
            state.sidebar_visible.set(!vis);
            ev.prevent_default();
        }

        // Reset all filters & sorting
        "r" => {
            state.filter.set(CardFilter::Extinguished);
            state
                .due_date_filter
                .set(crate::state::store::DueDateFilter::All);
            state.include_overdue.set(false);
            state.tag_filter.set(Vec::new());
            state
                .tag_filter_mode
                .set(blazelist_client_lib::filter::TagFilterMode::Or);
            state.no_tags_filter.set(false);
            state.linked_card_filter.set(Vec::new());
            state
                .sort_order
                .set(blazelist_client_lib::filter::SortOrder::default());
            state.search_query.set(String::new());
            sync_query_params(&state);
            ev.prevent_default();
        }

        _ => {}
    }
}

// -- Sub-menu handlers --------------------------------------------------------

/// Handle a key inside the **due-date filter** sub-menu.
/// Returns `true` if the key was recognised.
fn handle_due_date_filter_submenu(key: &str, state: AppState) -> bool {
    let filter = match key {
        "a" => DueDateFilter::All,
        "o" => DueDateFilter::Overdue,
        "t" => DueDateFilter::Today,
        "u" => DueDateFilter::TodayAndUpcoming,
        "m" => DueDateFilter::UpcomingTomorrow,
        "w" => DueDateFilter::UpcomingWeek,
        "2" => DueDateFilter::UpcomingTwoWeeks,
        "i" => {
            let cur = state.include_overdue.get_untracked();
            state.include_overdue.set(!cur);
            sync_query_params(&state);
            return true;
        }
        _ => return false,
    };
    state.due_date_filter.set(filter);
    sync_query_params(&state);
    true
}

/// Handle a key inside the **sort** sub-menu.
/// Returns `true` if the key was recognised.
fn handle_sort_submenu(key: &str, state: AppState) -> bool {
    let order = match key {
        "p" => SortOrder::Priority,
        "P" => SortOrder::PriorityReverse,
        "m" => SortOrder::ModifiedAt,
        "M" => SortOrder::ModifiedAtReverse,
        "c" => SortOrder::CreatedAt,
        "C" => SortOrder::CreatedAtReverse,
        "t" => SortOrder::Title,
        "T" => SortOrder::TitleReverse,
        "d" => SortOrder::DueDate,
        "D" => SortOrder::DueDateReverse,
        _ => return false,
    };
    state.sort_order.set(order);
    sync_query_params(&state);
    true
}

/// Handle a key inside the **linked-cards filter** sub-menu.
/// Returns `true` if the key was recognised.
fn handle_linked_cards_submenu(key: &str, state: AppState) -> bool {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return false,
    };
    let all_cards = state.cards.get_untracked();
    let card = match all_cards.iter().find(|c| c.id() == card_id) {
        Some(c) => c,
        None => return false,
    };
    let content = card.content();
    let forward_ids = blazelist_client_lib::display::extract_card_links(content, card_id);
    let back_ids = blazelist_client_lib::display::extract_back_links(card_id, &all_cards);
    let forward_set: std::collections::HashSet<uuid::Uuid> = forward_ids.iter().copied().collect();

    match key {
        // All linked cards (including transitive if recursive is enabled)
        "a" => {
            let mut all_linked = forward_ids.clone();
            for id in &back_ids {
                if !forward_set.contains(id) {
                    all_linked.push(*id);
                }
            }
            if state.recursive_links.get_untracked() {
                let expanded = state
                    .link_graph_cache
                    .get_untracked()
                    .get(&card_id)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| {
                        blazelist_client_lib::display::expand_linked_cards(card_id, &all_cards)
                    });
                let direct_set: std::collections::HashSet<uuid::Uuid> =
                    all_linked.iter().copied().collect();
                all_linked.extend(expanded.into_iter().filter(|id| !direct_set.contains(id)));
            }
            all_linked.insert(0, card_id);
            state.linked_card_filter.set(all_linked);
            state.filter.set(CardFilter::All);
            state.search_query.set(String::new());
            state.tag_filter.set(Vec::new());
            state.no_tags_filter.set(false);
            sync_query_params(&state);
            true
        }
        // Forward links only
        "f" => {
            let mut ids = forward_ids;
            ids.insert(0, card_id);
            state.linked_card_filter.set(ids);
            state.filter.set(CardFilter::All);
            state.search_query.set(String::new());
            state.tag_filter.set(Vec::new());
            state.no_tags_filter.set(false);
            sync_query_params(&state);
            true
        }
        // Back links only
        "b" => {
            let mut ids = back_ids;
            ids.insert(0, card_id);
            state.linked_card_filter.set(ids);
            state.filter.set(CardFilter::All);
            state.search_query.set(String::new());
            state.tag_filter.set(Vec::new());
            state.no_tags_filter.set(false);
            sync_query_params(&state);
            true
        }
        // Direct links (forward + back, no transitive)
        "d" => {
            let mut ids = forward_ids;
            for id in &back_ids {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
            ids.insert(0, card_id);
            state.linked_card_filter.set(ids);
            state.filter.set(CardFilter::All);
            state.search_query.set(String::new());
            state.tag_filter.set(Vec::new());
            state.no_tags_filter.set(false);
            sync_query_params(&state);
            true
        }
        // Clear linked card filter
        "c" => {
            state.linked_card_filter.set(Vec::new());
            sync_query_params(&state);
            true
        }
        _ => false,
    }
}

// -- Shared helpers -----------------------------------------------------------

fn handle_escape(state: AppState) {
    // Priority: close edit/create -> close settings/shortcuts -> close detail -> clear search -> clear filters
    if state.editing.get_untracked() || state.creating_new.get_untracked() {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.editing.set(false);
        state.creating_new.set(false);
        state.has_unsaved_changes.set(false);
        sync_query_params(&state);
        return;
    }

    if state.settings_open.get_untracked() {
        state.settings_open.set(false);
        return;
    }

    if state.shortcuts_open.get_untracked() {
        state.shortcuts_open.set(false);
        return;
    }

    if state.selected_card.get_untracked().is_some() {
        state.selected_card.set(None);
        sync_query_params(&state);
        return;
    }

    // Clear search first
    if !state.search_query.get_untracked().is_empty() {
        state.search_query.set(String::new());
        blur_active_element();
        sync_query_params(&state);
        return;
    }

    // Then clear all filters and sorting
    let has_filters = state.filter.get_untracked() != CardFilter::Extinguished
        || state.due_date_filter.get_untracked() != crate::state::store::DueDateFilter::All
        || state.include_overdue.get_untracked()
        || !state.tag_filter.get_untracked().is_empty()
        || state.no_tags_filter.get_untracked()
        || !state.linked_card_filter.get_untracked().is_empty()
        || !state.sort_order.get_untracked().is_default();

    if has_filters {
        state.filter.set(CardFilter::Extinguished);
        state
            .due_date_filter
            .set(crate::state::store::DueDateFilter::All);
        state.include_overdue.set(false);
        state.tag_filter.set(Vec::new());
        state
            .tag_filter_mode
            .set(crate::state::store::TagFilterMode::Or);
        state.no_tags_filter.set(false);
        state.linked_card_filter.set(Vec::new());
        state
            .sort_order
            .set(blazelist_client_lib::filter::SortOrder::default());
        sync_query_params(&state);
    }
}

fn blur_active_element() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        && let Ok(html_el) = el.dyn_into::<web_sys::HtmlElement>()
    {
        html_el.blur().ok();
    }
}

fn start_new_card(state: AppState, position: NewCardPosition) {
    if !confirm_discard_changes(&state) {
        return;
    }
    state.selected_card.set(None);
    state.editing.set(false);
    state.settings_open.set(false);
    state.shortcuts_open.set(false);
    state.new_card_position.set(position);
    state.creating_new.set(true);
    sync_query_params(&state);
}

fn select_first_card(state: AppState) {
    let filtered = state.filtered_cards().get_untracked();
    if let Some(first) = filtered.first() {
        select_card_view(&state, first.id());
    }
}

fn select_last_card(state: AppState) {
    let filtered = state.filtered_cards().get_untracked();
    if let Some(last) = filtered.last() {
        select_card_view(&state, last.id());
    }
}

fn select_next_card(state: AppState) {
    let filtered = state.filtered_cards().get_untracked();
    if filtered.is_empty() {
        return;
    }

    let current = state.selected_card.get_untracked();
    let next_id = match current {
        None => filtered.first().map(|c| c.id()),
        Some(id) => {
            let pos = filtered.iter().position(|c| c.id() == id);
            match pos {
                Some(i) if i + 1 < filtered.len() => Some(filtered[i + 1].id()),
                Some(_) => Some(id),
                None => filtered.first().map(|c| c.id()),
            }
        }
    };

    if let Some(id) = next_id {
        select_card_view(&state, id);
    }
}

fn select_prev_card(state: AppState) {
    let filtered = state.filtered_cards().get_untracked();
    if filtered.is_empty() {
        return;
    }

    let current = state.selected_card.get_untracked();
    let next_id = match current {
        None => filtered.last().map(|c| c.id()),
        Some(id) => {
            let pos = filtered.iter().position(|c| c.id() == id);
            match pos {
                Some(0) => Some(id),
                Some(i) => Some(filtered[i - 1].id()),
                None => filtered.last().map(|c| c.id()),
            }
        }
    };

    if let Some(id) = next_id {
        select_card_view(&state, id);
    }
}

fn toggle_blaze(state: AppState) {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return,
    };

    let card = state
        .cards
        .get_untracked()
        .into_iter()
        .find(|c| c.id() == card_id);
    let Some(card) = card else { return };

    let next = card.next(
        card.content().to_string(),
        card.priority(),
        card.tags().to_vec(),
        !card.blazed(),
        Utc::now(),
        card.due_date(),
    );
    state.upsert_card(next.clone());

    leptos::task::spawn_local(async move {
        push_card_or_queue(&state, next).await;
    });
}

pub(crate) fn show_copy_toast(state: AppState, msg: &str) {
    // Cancel any previous dismiss timer so rapid copies reset the countdown.
    if let Some(prev) = state.copy_toast_timeout.get_untracked() {
        let _ = web_sys::window().unwrap().clear_timeout_with_handle(prev);
    }
    state.copy_toast.set(Some(msg.to_string()));
    let cb = Closure::once(move || {
        state.copy_toast_timeout.set(None);
        state.copy_toast.set(None);
    });
    let func = cb.into_js_value();
    let handle = web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(func.unchecked_ref(), 1500)
        .unwrap();
    state.copy_toast_timeout.set(Some(handle));
}

pub(crate) fn copy_to_clipboard(text: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.navigator().clipboard().write_text(text);
    }
}

fn focus_tag_search() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector(".tag-search-input").ok())
        .flatten()
        && let Ok(input) = el.dyn_into::<web_sys::HtmlElement>()
    {
        input.focus().ok();
    }
}

fn focus_search_input() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector(".search-input").ok())
        .flatten()
        && let Ok(input) = el.dyn_into::<web_sys::HtmlElement>()
    {
        input.focus().ok();
    }
}

// -- Card movement shortcuts --------------------------------------------------

fn move_card_up(state: AppState) {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return,
    };
    let card = match state
        .cards
        .get_untracked()
        .into_iter()
        .find(|c| c.id() == card_id)
    {
        Some(c) => c,
        None => return,
    };
    let filtered = state.filtered_cards().get_untracked();
    let idx = match filtered.iter().position(|c| c.id() == card_id) {
        Some(i) => i,
        None => return,
    };
    if idx == 0 {
        return;
    }
    let placement = move_card(&filtered, card_id, InsertPosition::At(idx - 1));
    apply_move_placement(placement, &card, &filtered, state);
}

fn move_card_down(state: AppState) {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return,
    };
    let card = match state
        .cards
        .get_untracked()
        .into_iter()
        .find(|c| c.id() == card_id)
    {
        Some(c) => c,
        None => return,
    };
    let filtered = state.filtered_cards().get_untracked();
    let idx = match filtered.iter().position(|c| c.id() == card_id) {
        Some(i) => i,
        None => return,
    };
    if idx >= filtered.len() - 1 {
        return;
    }
    let placement = move_card(&filtered, card_id, InsertPosition::At(idx + 1));
    apply_move_placement(placement, &card, &filtered, state);
}

// -- Due date shortcuts -------------------------------------------------------

enum DueDateShortcut {
    Today,
    Tomorrow,
    Clear,
}

fn set_due_date_shortcut(state: AppState, shortcut: DueDateShortcut) {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return,
    };

    let card = match state
        .cards
        .get_untracked()
        .into_iter()
        .find(|c| c.id() == card_id)
    {
        Some(c) => c,
        None => return,
    };

    let new_due = match shortcut {
        DueDateShortcut::Today => {
            let today = Utc::now().date_naive();
            Some(today.and_hms_opt(12, 0, 0).unwrap().and_utc())
        }
        DueDateShortcut::Tomorrow => {
            let tomorrow = Utc::now().date_naive() + Days::new(1);
            Some(tomorrow.and_hms_opt(12, 0, 0).unwrap().and_utc())
        }
        DueDateShortcut::Clear => None,
    };

    let next = card.next(
        card.content().to_string(),
        card.priority(),
        card.tags().to_vec(),
        card.blazed(),
        Utc::now(),
        new_due,
    );
    state.upsert_card(next.clone());

    leptos::task::spawn_local(async move {
        push_card_or_queue(&state, next).await;
    });
}

// =============================================================================
// Sub-menu popup component
// =============================================================================

/// Describes a single key→action pair for the sub-menu popup.
struct SubMenuItem {
    key: &'static str,
    label: &'static str,
}

/// Returns the list of items for the given sub-menu.
fn sub_menu_items(menu: SubMenu) -> (&'static str, Vec<SubMenuItem>) {
    match menu {
        SubMenu::DueDateFilter => (
            "Due date filter",
            vec![
                SubMenuItem {
                    key: "a",
                    label: "All",
                },
                SubMenuItem {
                    key: "o",
                    label: "Overdue",
                },
                SubMenuItem {
                    key: "t",
                    label: "Today",
                },
                SubMenuItem {
                    key: "u",
                    label: "Today & upcoming",
                },
                SubMenuItem {
                    key: "m",
                    label: "Tomorrow",
                },
                SubMenuItem {
                    key: "w",
                    label: "Next 7 days",
                },
                SubMenuItem {
                    key: "2",
                    label: "Next 14 days",
                },
                SubMenuItem {
                    key: "i",
                    label: "Toggle include overdue",
                },
            ],
        ),
        SubMenu::LinkedCards => (
            "Linked card filter",
            vec![
                SubMenuItem {
                    key: "a",
                    label: "All linked",
                },
                SubMenuItem {
                    key: "f",
                    label: "Forward links only",
                },
                SubMenuItem {
                    key: "b",
                    label: "Back links only",
                },
                SubMenuItem {
                    key: "d",
                    label: "Direct (forward + back)",
                },
                SubMenuItem {
                    key: "c",
                    label: "Clear filter",
                },
            ],
        ),
        SubMenu::Sort => (
            "Sort order",
            vec![
                SubMenuItem {
                    key: "p",
                    label: "Priority",
                },
                SubMenuItem {
                    key: "P",
                    label: "Priority (reverse)",
                },
                SubMenuItem {
                    key: "m",
                    label: "Last modified",
                },
                SubMenuItem {
                    key: "M",
                    label: "Last modified (reverse)",
                },
                SubMenuItem {
                    key: "c",
                    label: "Created",
                },
                SubMenuItem {
                    key: "C",
                    label: "Created (reverse)",
                },
                SubMenuItem {
                    key: "t",
                    label: "Title (A-Z)",
                },
                SubMenuItem {
                    key: "T",
                    label: "Title (Z-A)",
                },
                SubMenuItem {
                    key: "d",
                    label: "Due date",
                },
                SubMenuItem {
                    key: "D",
                    label: "Due date (reverse)",
                },
            ],
        ),
    }
}

/// Floating popup shown when a keyboard sub-menu is active.
///
/// Renders nothing when `state.sub_menu` is `None`.
#[component]
pub fn SubMenuPopup() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let dismiss = move |_| {
        state.sub_menu.set(None);
    };

    move || {
        let menu = state.sub_menu.get();
        menu.map(|m| {
            let (title, items) = sub_menu_items(m);
            view! {
                <div class="submenu-backdrop" on:click=dismiss>
                    <div class="submenu-popup" on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()>
                        <div class="submenu-title">{title}</div>
                        <ul class="submenu-list">
                            {items.into_iter().map(|item| {
                                view! {
                                    <li class="submenu-item">
                                        <kbd>{item.key}</kbd>
                                        <span>{item.label}</span>
                                    </li>
                                }
                            }).collect::<Vec<_>>()}
                        </ul>
                        <div class="submenu-hint"><kbd>"q"</kbd>" or "<kbd>"Esc"</kbd>" to cancel"</div>
                    </div>
                </div>
            }
        })
    }
}

// =============================================================================
// Shortcuts panel (detail pane)
// =============================================================================

/// Renders the keyboard shortcuts pane in the detail panel area.
#[component]
pub fn ShortcutsPanel() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let on_close = move |_| {
        state.shortcuts_open.set(false);
    };

    view! {
        <div class="settings-page">
            <div class="detail-header">
                <span class="detail-status">"Keyboard Shortcuts"</span>
                <button class="detail-close" on:click=on_close>"x"</button>
            </div>
            <div class="help-body">
                // --- Navigation ---
                <h3 class="help-section-title">"Navigation"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"j"</kbd></td><td>"Select next card"</td></tr>
                        <tr><td><kbd>"k"</kbd></td><td>"Select previous card"</td></tr>
                        <tr><td><kbd>"g"</kbd></td><td>"Go to first card"</td></tr>
                        <tr><td><kbd>"G"</kbd></td><td>"Go to last card"</td></tr>
                        <tr><td><kbd>"f"</kbd>" / "<kbd>"/"</kbd></td><td>"Focus search"</td></tr>
                        <tr><td><kbd>"F"</kbd></td><td>"Focus sidebar tag search"</td></tr>
                        <tr><td><kbd>"Enter"</kbd></td><td>"Confirm search & select first card"</td></tr>
                        <tr><td><kbd>"h"</kbd></td><td>"Go back (browser history)"</td></tr>
                    </tbody>
                </table>

                // --- Card actions ---
                <h3 class="help-section-title">"Card actions"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"n"</kbd></td><td>"New card (bottom)"</td></tr>
                        <tr><td><kbd>"N"</kbd></td><td>"New card (top)"</td></tr>
                        <tr><td><kbd>"o"</kbd></td><td>"New card below selected"</td></tr>
                        <tr><td><kbd>"O"</kbd></td><td>"New card above selected"</td></tr>
                        <tr><td><kbd>"Y"</kbd></td><td>"New tag"</td></tr>
                        <tr><td><kbd>"e"</kbd></td><td>"Edit selected card"</td></tr>
                        <tr><td><kbd>"y"</kbd></td><td>"Copy card ID to clipboard"</td></tr>
                        <tr><td><kbd>"B"</kbd></td><td>"Blaze / extinguish"</td></tr>
                        <tr><td><kbd>"J"</kbd></td><td>"Move card down"</td></tr>
                        <tr><td><kbd>"K"</kbd></td><td>"Move card up"</td></tr>
                        <tr><td><kbd>"t"</kbd></td><td>"Set due date to today"</td></tr>
                        <tr><td><kbd>"T"</kbd></td><td>"Set due date to tomorrow"</td></tr>
                        <tr><td><kbd>"C"</kbd></td><td>"Clear due date"</td></tr>
                    </tbody>
                </table>

                // --- Sub-menus ---
                <h3 class="help-section-title">"Sub-menus (press key, then choose)"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Opens"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"d"</kbd></td><td>"Due date filter — "<kbd>"a"</kbd>" All  "<kbd>"o"</kbd>" Overdue  "<kbd>"t"</kbd>" Today  "<kbd>"u"</kbd>" Today+  "<kbd>"U"</kbd>" Upcoming  "<kbd>"m"</kbd>" Tomorrow  "<kbd>"w"</kbd>" Week  "<kbd>"2"</kbd>" 2 Weeks  "<kbd>"i"</kbd>" Toggle overdue"</td></tr>
                        <tr><td><kbd>"s"</kbd></td><td>"Sort — "<kbd>"p"</kbd>"/"<kbd>"P"</kbd>" Priority  "<kbd>"m"</kbd>"/"<kbd>"M"</kbd>" Modified  "<kbd>"c"</kbd>"/"<kbd>"C"</kbd>" Created  "<kbd>"t"</kbd>"/"<kbd>"T"</kbd>" Title  "<kbd>"d"</kbd>"/"<kbd>"D"</kbd>" Due date"</td></tr>
                        <tr><td><kbd>"l"</kbd></td><td>"Linked cards — "<kbd>"a"</kbd>" All  "<kbd>"f"</kbd>" Forward  "<kbd>"b"</kbd>" Back  "<kbd>"d"</kbd>" Direct  "<kbd>"c"</kbd>" Clear"</td></tr>
                    </tbody>
                </table>

                // --- Filters & toggles ---
                <h3 class="help-section-title">"Filters & toggles"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"a"</kbd></td><td>"Show active cards"</td></tr>
                        <tr><td><kbd>"A"</kbd></td><td>"Show all cards"</td></tr>
                        <tr><td><kbd>"b"</kbd></td><td>"Show blazed cards"</td></tr>
                        <tr><td><kbd>"v"</kbd></td><td>"Toggle tag-filter mode (OR / AND)"</td></tr>
                        <tr><td><kbd>"V"</kbd></td><td>"Toggle \"no tags\" filter"</td></tr>
                        <tr><td><kbd>"i"</kbd></td><td>"Toggle include-overdue"</td></tr>
                        <tr><td><kbd>"r"</kbd></td><td>"Reset all filters, sorting & search"</td></tr>
                        <tr><td><kbd>"x"</kbd></td><td>"Toggle sidebar"</td></tr>
                        <tr><td><kbd>"m"</kbd></td><td>"Minimize detail panel (side-panel layout)"</td></tr>
                        <tr><td><kbd>"M"</kbd></td><td>"Maximize detail panel (fullscreen layout)"</td></tr>
                    </tbody>
                </table>

                // --- General ---
                <h3 class="help-section-title">"General"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>","</kbd></td><td>"Toggle settings"</td></tr>
                        <tr><td><kbd>"?"</kbd></td><td>"Toggle this shortcuts panel"</td></tr>
                        <tr><td><kbd>"Esc"</kbd></td><td>"Close panel / clear search / clear filters & sorting"</td></tr>
                    </tbody>
                </table>

                // --- Context-specific ---
                <h3 class="help-section-title">"While editing"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"Ctrl+Enter"</kbd></td><td>"Save / create card"</td></tr>
                        <tr><td><kbd>"Esc"</kbd></td><td>"Cancel editing (confirms if unsaved)"</td></tr>
                        <tr><td><kbd>"Enter"</kbd></td><td>"Toggle first matching tag (in tag search)"</td></tr>
                    </tbody>
                </table>

                <h3 class="help-section-title">"Sidebar tag search"</h3>
                <table class="help-table">
                    <thead><tr><th>"Key"</th><th>"Action"</th></tr></thead>
                    <tbody>
                        <tr><td><kbd>"Enter"</kbd></td><td>"Toggle first matching tag filter"</td></tr>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

//! Global keyboard shortcut handler for the WASM client.
//!
//! Shortcuts are suppressed while the user is typing in an input, textarea,
//! or contenteditable element, and can be disabled entirely in settings.

use crate::components::card_detail::apply_move_placement;
use crate::components::settings_panel::switch_to_pane;
use crate::state::store::{
    AppState, NewCardPosition, confirm_discard_changes, select_card_view, sync_query_params,
};
use crate::state::sync::push_card_or_queue;
use blazelist_client_lib::priority::{InsertPosition, move_card};
use blazelist_protocol::{CardFilter, Entity, Utc};
use chrono::Days;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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
    if let Some(attr) = el.get_attribute("contenteditable") {
        if attr == "true" || attr == "" {
            return true;
        }
    }

    false
}

/// Returns `true` if the currently focused element is the search input.
fn is_search_focused() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .and_then(|el| el.dyn_into::<web_sys::Element>().ok())
        .map(|el| el.class_list().contains("search-input"))
        .unwrap_or(false)
}

fn handle_keydown(ev: web_sys::KeyboardEvent, state: AppState) {
    let key = ev.key();

    // Escape always works, even when typing or shortcuts disabled
    if key == "Escape" {
        handle_escape(state);
        ev.prevent_default();
        return;
    }

    // Enter while search input is focused: blur and select first card
    if key == "Enter" && is_search_focused() {
        blur_active_element();
        select_first_card(state);
        ev.prevent_default();
        return;
    }

    // Don't handle shortcuts when typing in inputs
    if is_typing() {
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

        // Edit selected card
        "e" => {
            if state.selected_card.get_untracked().is_some() {
                state.editing.set(true);
                ev.prevent_default();
            }
        }

        // Blaze / extinguish selected card
        "b" => {
            toggle_blaze(state);
            ev.prevent_default();
        }

        // Cycle blaze filter: Active -> All -> Blazed -> Active
        "f" => {
            cycle_blaze_filter(state);
            ev.prevent_default();
        }

        // Focus search input
        "/" => {
            focus_search_input();
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

        _ => {}
    }
}

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
        state.due_date_filter.set(crate::state::store::DueDateFilter::All);
        state.include_overdue.set(false);
        state.tag_filter.set(Vec::new());
        state.tag_filter_mode.set(crate::state::store::TagFilterMode::Or);
        state.no_tags_filter.set(false);
        state.linked_card_filter.set(Vec::new());
        state.sort_order.set(blazelist_client_lib::filter::SortOrder::default());
        sync_query_params(&state);
    }
}

fn blur_active_element() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
    {
        if let Ok(html_el) = el.dyn_into::<web_sys::HtmlElement>() {
            html_el.blur().ok();
        }
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

fn cycle_blaze_filter(state: AppState) {
    let current = state.filter.get_untracked();
    let next = match current {
        CardFilter::Extinguished => CardFilter::All,
        CardFilter::All => CardFilter::Blazed,
        CardFilter::Blazed => CardFilter::Extinguished,
    };
    state.filter.set(next);
    sync_query_params(&state);
}

fn focus_search_input() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector(".search-input").ok())
        .flatten()
    {
        if let Ok(input) = el.dyn_into::<web_sys::HtmlElement>() {
            input.focus().ok();
        }
    }
}

// -- Card movement shortcuts --------------------------------------------------

fn move_card_up(state: AppState) {
    let card_id = match state.selected_card.get_untracked() {
        Some(id) => id,
        None => return,
    };
    let card = match state.cards.get_untracked().into_iter().find(|c| c.id() == card_id) {
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
    let card = match state.cards.get_untracked().into_iter().find(|c| c.id() == card_id) {
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

    let card = match state.cards.get_untracked().into_iter().find(|c| c.id() == card_id) {
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
                <table class="help-table">
                    <thead>
                        <tr><th>"Key"</th><th>"Action"</th></tr>
                    </thead>
                    <tbody>
                        <tr><td><kbd>"j"</kbd></td><td>"Select next card"</td></tr>
                        <tr><td><kbd>"k"</kbd></td><td>"Select previous card"</td></tr>
                        <tr><td><kbd>"g"</kbd></td><td>"Go to first card"</td></tr>
                        <tr><td><kbd>"G"</kbd>" (shift)"</td><td>"Go to last card"</td></tr>
                        <tr><td><kbd>"J"</kbd>" (shift)"</td><td>"Move card down"</td></tr>
                        <tr><td><kbd>"K"</kbd>" (shift)"</td><td>"Move card up"</td></tr>
                        <tr><td><kbd>"n"</kbd></td><td>"New card (bottom)"</td></tr>
                        <tr><td><kbd>"N"</kbd>" (shift)"</td><td>"New card (top)"</td></tr>
                        <tr><td><kbd>"o"</kbd></td><td>"New card below selected"</td></tr>
                        <tr><td><kbd>"O"</kbd>" (shift)"</td><td>"New card above selected"</td></tr>
                        <tr><td><kbd>"e"</kbd></td><td>"Edit selected card"</td></tr>
                        <tr><td><kbd>"b"</kbd></td><td>"Blaze / extinguish"</td></tr>
                        <tr><td><kbd>"t"</kbd></td><td>"Set due date to today"</td></tr>
                        <tr><td><kbd>"T"</kbd>" (shift)"</td><td>"Set due date to tomorrow"</td></tr>
                        <tr><td><kbd>"C"</kbd>" (shift)"</td><td>"Clear due date"</td></tr>
                        <tr><td><kbd>"f"</kbd></td><td>"Cycle filter (Active / All / Blazed)"</td></tr>
                        <tr><td><kbd>"/"</kbd></td><td>"Focus search"</td></tr>
                        <tr><td><kbd>"Enter"</kbd></td><td>"Confirm search and select first card"</td></tr>
                        <tr><td><kbd>","</kbd></td><td>"Toggle settings"</td></tr>
                        <tr><td><kbd>"Esc"</kbd></td><td>"Close panel / clear search / clear filters & sorting"</td></tr>
                        <tr><td><kbd>"?"</kbd></td><td>"Toggle this shortcuts panel"</td></tr>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

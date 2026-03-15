use crate::state::settings;
use crate::state::store::{confirm_discard_changes, sync_query_params, AppState};
use leptos::prelude::*;

/// Helper: check for unsaved changes and clear pane state before opening a new pane.
/// Returns `false` if the user cancels.
pub fn switch_to_pane(state: &AppState, open_settings: bool, open_shortcuts: bool) -> bool {
    if !confirm_discard_changes(state) {
        return false;
    }
    state.selected_card.set(None);
    state.creating_new.set(false);
    state.creating_new_tag.set(false);
    state.editing.set(false);
    state.has_unsaved_changes.set(false);
    state.settings_open.set(open_settings);
    state.shortcuts_open.set(open_shortcuts);
    sync_query_params(state);
    true
}

/// Settings gear button in the header. Toggles the settings panel open/closed.
#[component]
pub fn SettingsButton() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let toggle = move |_| {
        let opening = !state.settings_open.get_untracked();
        if opening {
            switch_to_pane(&state, true, false);
        } else {
            state.settings_open.set(false);
        }
    };

    view! {
        <button class="settings-btn" on:click=toggle title="Settings">
            {"\u{2699}"}
        </button>
    }
}

/// Full settings panel rendered in the detail panel area.
#[component]
pub fn SettingsPanel() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let on_close = move |_| {
        state.settings_open.set(false);
    };

    let on_toggle_keyboard_shortcuts = move |_| {
        let new_val = !state.keyboard_shortcuts_enabled.get_untracked();
        state.keyboard_shortcuts_enabled.set(new_val);
        settings::save_keyboard_shortcuts(new_val);
    };

    let on_toggle_touch_swipe = move |_| {
        let new_val = !state.touch_swipe_enabled.get_untracked();
        state.touch_swipe_enabled.set(new_val);
        settings::save_touch_swipe(new_val);
    };


    let on_toggle_show_preview = move |_| {
        let new_val = !state.show_preview.get_untracked();
        state.show_preview.set(new_val);
        settings::save_show_preview(new_val);
    };

    let on_toggle_auto_save = move |_| {
        let new_val = !state.auto_save_enabled.get_untracked();
        state.auto_save_enabled.set(new_val);
        settings::save_auto_save(new_val);
    };

    let on_change_delay = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(secs) = val.parse::<u32>() {
            let secs = secs.max(1).min(60);
            state.auto_save_delay_secs.set(secs);
            settings::save_auto_save_delay(secs);
        }
    };

    let on_toggle_auto_sync = move |_| {
        let new_val = !state.auto_sync_enabled.get_untracked();
        state.auto_sync_enabled.set(new_val);
        settings::save_auto_sync(new_val);
        if !new_val {
            state.auto_sync_countdown.set(0);
        }
    };

    let on_toggle_debounce = move |_| {
        let new_val = !state.debounce_enabled.get_untracked();
        state.debounce_enabled.set(new_val);
        settings::save_debounce_enabled(new_val);
    };

    let on_change_debounce = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(secs) = val.parse::<u32>() {
            let secs = secs.max(1).min(10);
            state.debounce_delay_secs.set(secs);
            settings::save_debounce_delay(secs);
        }
    };

    let on_change_sync_interval = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(secs) = val.parse::<u32>() {
            let secs = secs.max(5).min(300);
            state.auto_sync_interval_secs.set(secs);
            settings::save_auto_sync_interval(secs);
        }
    };

    let on_toggle_search_tags = move |_| {
        let new_val = !state.search_tags.get_untracked();
        state.search_tags.set(new_val);
        settings::save_search_tags(new_val);
    };

    let on_change_ui_scale = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(pct) = val.parse::<u32>() {
            let pct = pct.max(75).min(200);
            state.ui_scale.set(pct);
            settings::save_ui_scale(pct);
            apply_ui_scale(pct);
        }
    };

    let on_change_density = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        state.ui_density.set(val.clone());
        settings::save_ui_density(&val);
        apply_ui_density(&val);
    };

    let on_open_shortcuts = move |_| {
        switch_to_pane(&state, false, true);
    };

    view! {
        <div class="settings-page">
            <div class="detail-header">
                <span class="detail-status">"Settings"</span>
                <button class="detail-close" on:click=on_close>"x"</button>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Periodic sync check"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.auto_sync_enabled.get()
                        on:change=on_toggle_auto_sync
                    />
                </label>
                <div class="settings-hint">
                    "Periodically verify local state matches the server, catching any missed real-time updates"
                </div>
                {move || state.auto_sync_enabled.get().then(|| view! {
                    <div class="settings-sub-item">
                        <span class="settings-label">"Interval (seconds)"</span>
                        <input
                            type="number"
                            class="settings-number"
                            min="5"
                            max="300"
                            prop:value=move || state.auto_sync_interval_secs.get().to_string()
                            on:change=on_change_sync_interval
                        />
                    </div>
                })}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Auto-save while editing"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.auto_save_enabled.get()
                        on:change=on_toggle_auto_save
                    />
                </label>
                <div class="settings-hint">
                    "Automatically save card changes after a delay"
                </div>
                {move || state.auto_save_enabled.get().then(|| view! {
                    <div class="settings-sub-item">
                        <span class="settings-label">"Delay (seconds)"</span>
                        <input
                            type="number"
                            class="settings-number"
                            min="1"
                            max="60"
                            prop:value=move || state.auto_save_delay_secs.get().to_string()
                            on:change=on_change_delay
                        />
                    </div>
                })}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Push debounce"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.debounce_enabled.get()
                        on:change=on_toggle_debounce
                    />
                </label>
                <div class="settings-hint">
                    "Delay before pushing card changes to the server. When disabled, changes are pushed instantly."
                </div>
                {move || state.debounce_enabled.get().then(|| view! {
                    <div class="settings-sub-item">
                        <span class="settings-label">"Delay (seconds)"</span>
                        <input
                            type="number"
                            class="settings-number"
                            min="1"
                            max="10"
                            prop:value=move || state.debounce_delay_secs.get().to_string()
                            on:change=on_change_debounce
                        />
                    </div>
                })}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Show preview when editing"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.show_preview.get()
                        on:change=on_toggle_show_preview
                    />
                </label>
                <div class="settings-hint">
                    "Show markdown preview alongside the editor by default"
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Include tags in search"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.search_tags.get()
                        on:change=on_toggle_search_tags
                    />
                </label>
                <div class="settings-hint">
                    "Search matches card content and tag names"
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Keyboard shortcuts"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.keyboard_shortcuts_enabled.get()
                        on:change=on_toggle_keyboard_shortcuts
                    />
                </label>
                <div class="settings-hint">
                    <button class="settings-link-btn" on:click=on_open_shortcuts>"View shortcuts"</button>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-item">
                    <span class="settings-label">"UI scale"</span>
                    <select
                        class="settings-select"
                        on:change=on_change_ui_scale
                        prop:value=move || state.ui_scale.get().to_string()
                    >
                        <option value="75">"75%"</option>
                        <option value="80">"80%"</option>
                        <option value="90">"90%"</option>
                        <option value="100">"100% (default)"</option>
                        <option value="110">"110%"</option>
                        <option value="125">"125%"</option>
                        <option value="150">"150%"</option>
                        <option value="175">"175%"</option>
                        <option value="200">"200%"</option>
                    </select>
                </div>
                <div class="settings-hint">
                    "Scale the entire UI"
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-item">
                    <span class="settings-label">"Density"</span>
                    <select
                        class="settings-select"
                        on:change=on_change_density
                        prop:value=move || state.ui_density.get()
                    >
                        <option value="compact">"Compact"</option>
                        <option value="cozy">"Cozy"</option>
                    </select>
                </div>
                <div class="settings-hint">
                    "Cozy mode adds more spacing and larger tag dots"
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Touch swipe gestures"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.touch_swipe_enabled.get()
                        on:change=on_toggle_touch_swipe
                    />
                </label>
                <div class="settings-hint">
                    "Swipe right to blaze, swipe left to set due date to today/tomorrow"
                </div>
            </div>
        </div>
    }
}

/// Apply the UI scale to the root element.
pub fn apply_ui_scale(pct: u32) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = doc.document_element() {
            let _ = root
                .unchecked_ref::<web_sys::HtmlElement>()
                .style()
                .set_property("font-size", &format!("{}%", pct));
        }
    }
}

/// Apply the UI density class to the root element.
pub fn apply_ui_density(density: &str) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = doc.document_element() {
            let cl = root.class_list();
            let _ = cl.remove_1("density-compact");
            let _ = cl.remove_1("density-cozy");
            let class = format!("density-{density}");
            let _ = cl.add_1(&class);
        }
    }
}

use wasm_bindgen::JsCast;

use crate::state::settings;
use crate::state::store::{confirm_discard_changes, AppState};
use leptos::prelude::*;

/// Settings gear button in the header. Toggles the settings panel open/closed.
#[component]
pub fn SettingsButton() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let toggle = move |_| {
        let opening = !state.settings_open.get_untracked();
        if opening {
            if !confirm_discard_changes(&state) {
                return;
            }
            state.selected_card.set(None);
            state.creating_new.set(false);
            state.creating_new_tag.set(false);
            state.editing.set(false);
            state.has_unsaved_changes.set(false);
        }
        state.settings_open.set(opening);
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

    let on_toggle_drag_drop = move |_| {
        let new_val = !state.drag_drop_reorder.get_untracked();
        state.drag_drop_reorder.set(new_val);
        settings::save_drag_drop_reorder(new_val);
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
                    <span class="settings-label">"Keyboard shortcuts"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.keyboard_shortcuts_enabled.get()
                        on:change=on_toggle_keyboard_shortcuts
                    />
                </label>
                <div class="settings-hint">
                    "Press ? to see available shortcuts"
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Drag & drop reorder"</span>
                    <input
                        type="checkbox"
                        class="toggle-checkbox"
                        prop:checked=move || state.drag_drop_reorder.get()
                        on:change=on_toggle_drag_drop
                    />
                </label>
                <div class="settings-hint">
                    "May impact performance with many cards"
                </div>
            </div>
        </div>
    }
}

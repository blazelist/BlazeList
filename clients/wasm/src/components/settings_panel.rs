use crate::state::settings;
use crate::state::store::{AppState, confirm_discard_changes, sync_query_params};
use crate::storage;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "confirm")]
    fn js_confirm(message: &str) -> bool;
}

/// Render the env-var info line below a setting with three layers:
/// 1. Compile-time default
/// 2. Server override (from /config, if set)
/// 3. Current user value (reactive, updates when toggled)
fn env_info(
    state: AppState,
    env_var: &'static str,
    default_val: String,
    current: Signal<String>,
) -> impl IntoView {
    // Config key: strip "BLAZELIST_DEFAULT_" prefix and lowercase.
    let config_key: String = env_var
        .strip_prefix("BLAZELIST_DEFAULT_")
        .unwrap_or(env_var)
        .to_lowercase();

    let default_for_server = default_val.clone();
    let default_for_user = default_val.clone();

    view! {
        <div class="settings-env">
            <span class="settings-env-var">{env_var}</span>
            <span class="settings-env-detail">"default: "<code>{default_val}</code></span>
            {move || {
                let server = state
                    .server_config
                    .get()
                    .get(&config_key)
                    .cloned();
                server
                    .filter(|s| *s != default_for_server)
                    .map(|s| {
                        view! { <span class="settings-env-server">"server: "<code>{s}</code></span> }
                    })
            }}
            {move || {
                let cur = current.get();
                (cur != default_for_user).then(|| {
                    view! { <span class="settings-env-user">"current: "<code>{cur}</code></span> }
                })
            }}
        </div>
    }
}

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

    // --- Toggle handlers ---
    let on_toggle_auto_sync = move |_| {
        let v = !state.auto_sync_enabled.get_untracked();
        state.auto_sync_enabled.set(v);
        settings::save_auto_sync(v);
        if !v {
            state.auto_sync_countdown.set(0);
        }
    };
    let on_change_sync_interval = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(s) = val.parse::<u32>() {
            let s = s.clamp(5, 300);
            state.auto_sync_interval_secs.set(s);
            settings::save_auto_sync_interval(s);
        }
    };
    let on_toggle_auto_save = move |_| {
        let v = !state.auto_save_enabled.get_untracked();
        state.auto_save_enabled.set(v);
        settings::save_auto_save(v);
    };
    let on_change_delay = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(s) = val.parse::<u32>() {
            let s = s.clamp(1, 60);
            state.auto_save_delay_secs.set(s);
            settings::save_auto_save_delay(s);
        }
    };
    let on_toggle_debounce = move |_| {
        let v = !state.debounce_enabled.get_untracked();
        state.debounce_enabled.set(v);
        settings::save_debounce_enabled(v);
    };
    let on_change_debounce = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(s) = val.parse::<u32>() {
            let s = s.clamp(1, 10);
            state.debounce_delay_secs.set(s);
            settings::save_debounce_delay(s);
        }
    };
    let on_toggle_show_preview = move |_| {
        let v = !state.show_preview.get_untracked();
        state.show_preview.set(v);
        settings::save_show_preview(v);
    };
    let on_toggle_search_tags = move |_| {
        let v = !state.search_tags.get_untracked();
        state.search_tags.set(v);
        settings::save_search_tags(v);
    };
    let on_toggle_clear_tag_search = move |_| {
        let v = !state.clear_tag_search.get_untracked();
        state.clear_tag_search.set(v);
        settings::save_clear_tag_search(v);
    };
    let on_toggle_show_due_today_button = move |_| {
        let v = !state.show_due_today_button.get_untracked();
        state.show_due_today_button.set(v);
        settings::save_show_due_today_button(v);
    };
    let on_toggle_recursive_links = move |_| {
        let v = !state.recursive_links.get_untracked();
        state.recursive_links.set(v);
        settings::save_recursive_links(v);
        if !v {
            state.link_graph_cache.set(std::collections::HashMap::new());
            state.link_cache_progress.set((0, 0));
            leptos::task::spawn_local(async { storage::clear_link_cache().await });
        }
    };
    let on_toggle_show_list_link_counts = move |_| {
        let v = !state.show_list_link_counts.get_untracked();
        state.show_list_link_counts.set(v);
        settings::save_show_list_link_counts(v);
        if !v {
            state.link_graph_cache.set(std::collections::HashMap::new());
            state.link_cache_progress.set((0, 0));
            leptos::task::spawn_local(async { storage::clear_link_cache().await });
        }
    };
    let on_toggle_keyboard_shortcuts = move |_| {
        let v = !state.keyboard_shortcuts_enabled.get_untracked();
        state.keyboard_shortcuts_enabled.set(v);
        settings::save_keyboard_shortcuts(v);
    };
    let on_toggle_touch_swipe = move |_| {
        let v = !state.touch_swipe_enabled.get_untracked();
        state.touch_swipe_enabled.set(v);
        settings::save_touch_swipe(v);
    };
    let on_change_swipe_threshold_right = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(px) = val.parse::<u32>() {
            let px = px.clamp(40, 150);
            state.swipe_threshold_right.set(px);
            settings::save_swipe_threshold_right(px);
        }
    };
    let on_change_swipe_threshold_left = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(px) = val.parse::<u32>() {
            let px = px.clamp(40, 150);
            state.swipe_threshold_left.set(px);
            settings::save_swipe_threshold_left(px);
        }
    };
    let on_change_swipe_undo_timeout = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(ms) = val.parse::<u32>() {
            let ms = ms.clamp(500, 30_000);
            state.swipe_undo_timeout_ms.set(ms);
            settings::save_swipe_undo_timeout_ms(ms);
        }
    };
    let on_change_ui_scale = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(pct) = val.parse::<u32>() {
            let pct = pct.clamp(50, 300);
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
    let on_toggle_show_card_time = move |_| {
        let v = !state.show_card_time.get_untracked();
        state.show_card_time.set(v);
        settings::save_show_card_time(v);
        apply_show_card_time(v);
    };
    let on_toggle_override_sidebar = move |_| {
        let v = !state.override_sidebar_width.get_untracked();
        state.override_sidebar_width.set(v);
        settings::save_override_sidebar_width(v);
        if v && !settings::has_default_sidebar_width() {
            let current = state.sidebar_width.get_untracked() as u32;
            state.default_sidebar_width.set(current);
            settings::save_default_sidebar_width(current);
        }
    };
    let on_toggle_override_detail = move |_| {
        let v = !state.override_detail_width.get_untracked();
        state.override_detail_width.set(v);
        settings::save_override_detail_width(v);
        if v && !settings::has_default_detail_width() {
            let current = state.detail_width.get_untracked() as u32;
            state.default_detail_width.set(current);
            settings::save_default_detail_width(current);
        }
    };
    let on_change_sidebar_width = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(px) = val.parse::<u32>() {
            let px = px.clamp(80, 400);
            state.default_sidebar_width.set(px);
            settings::save_default_sidebar_width(px);
        }
    };
    let on_change_detail_width = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(px) = val.parse::<u32>() {
            let px = if px == 0 { 0 } else { px.clamp(280, 1200) };
            state.default_detail_width.set(px);
            settings::save_default_detail_width(px);
        }
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

            // ── Sync & Saving ──
            <div class="settings-section-title">"Sync & Saving"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Periodic sync check"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.auto_sync_enabled.get()
                        on:change=on_toggle_auto_sync />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_AUTO_SYNC", settings::DEFAULT_AUTO_SYNC.to_string(), Signal::derive(move || state.auto_sync_enabled.get().to_string()))}
                <div class=move || if state.auto_sync_enabled.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Interval (seconds)"</span>
                        <input type="number" class="settings-number" min="5" max="300"
                            prop:value=move || state.auto_sync_interval_secs.get().to_string()
                            on:change=on_change_sync_interval />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL", settings::DEFAULT_AUTO_SYNC_INTERVAL.to_string(), Signal::derive(move || state.auto_sync_interval_secs.get().to_string()))}
                    </div>
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Auto-save while editing"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.auto_save_enabled.get()
                        on:change=on_toggle_auto_save />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_AUTO_SAVE", settings::DEFAULT_AUTO_SAVE.to_string(), Signal::derive(move || state.auto_save_enabled.get().to_string()))}
                <div class=move || if state.auto_save_enabled.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Delay (seconds)"</span>
                        <input type="number" class="settings-number" min="1" max="60"
                            prop:value=move || state.auto_save_delay_secs.get().to_string()
                            on:change=on_change_delay />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_AUTO_SAVE_DELAY", settings::DEFAULT_AUTO_SAVE_DELAY.to_string(), Signal::derive(move || state.auto_save_delay_secs.get().to_string()))}
                    </div>
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Push debounce"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.debounce_enabled.get()
                        on:change=on_toggle_debounce />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_DEBOUNCE_ENABLED", settings::DEFAULT_DEBOUNCE_ENABLED.to_string(), Signal::derive(move || state.debounce_enabled.get().to_string()))}
                <div class=move || if state.debounce_enabled.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Delay (seconds)"</span>
                        <input type="number" class="settings-number" min="1" max="10"
                            prop:value=move || state.debounce_delay_secs.get().to_string()
                            on:change=on_change_debounce />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_DEBOUNCE_DELAY", settings::DEFAULT_DEBOUNCE_DELAY.to_string(), Signal::derive(move || state.debounce_delay_secs.get().to_string()))}
                    </div>
                </div>
            </div>

            // ── Editor ──
            <div class="settings-section-title">"Editor"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Show markdown preview"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.show_preview.get()
                        on:change=on_toggle_show_preview />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_SHOW_PREVIEW", settings::DEFAULT_SHOW_PREVIEW.to_string(), Signal::derive(move || state.show_preview.get().to_string()))}
            </div>

            // ── Search & Filtering ──
            <div class="settings-section-title">"Search & Filtering"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Include tags in search"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.search_tags.get()
                        on:change=on_toggle_search_tags />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_SEARCH_TAGS", settings::DEFAULT_SEARCH_TAGS.to_string(), Signal::derive(move || state.search_tags.get().to_string()))}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Clear tag search on select"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.clear_tag_search.get()
                        on:change=on_toggle_clear_tag_search />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH", settings::DEFAULT_CLEAR_TAG_SEARCH.to_string(), Signal::derive(move || state.clear_tag_search.get().to_string()))}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Show Today quick-filter button"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.show_due_today_button.get()
                        on:change=on_toggle_show_due_today_button />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_SHOW_DUE_TODAY_BUTTON", settings::DEFAULT_SHOW_DUE_TODAY_BUTTON.to_string(), Signal::derive(move || state.show_due_today_button.get().to_string()))}
            </div>

            // ── Linked Cards ──
            <div class="settings-section-title">"Linked Cards"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Recursive linked cards"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.recursive_links.get()
                        on:change=on_toggle_recursive_links />
                </label>
                <div class="settings-hint">
                    "Expand linked cards transitively through chains of links"
                </div>
                {env_info(state, "BLAZELIST_DEFAULT_RECURSIVE_LINKS", settings::DEFAULT_RECURSIVE_LINKS.to_string(), Signal::derive(move || state.recursive_links.get().to_string()))}
                <div class=move || if state.recursive_links.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Show transitive counts in card list"</span>
                        <input type="checkbox" class="toggle-checkbox"
                            prop:checked=move || state.show_list_link_counts.get()
                            on:change=on_toggle_show_list_link_counts />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_SHOW_LIST_LINK_COUNTS", settings::DEFAULT_SHOW_LIST_LINK_COUNTS.to_string(), Signal::derive(move || state.show_list_link_counts.get().to_string()))}
                    </div>
                </div>
            </div>

            // ── Input ──
            <div class="settings-section-title">"Input"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Keyboard shortcuts"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.keyboard_shortcuts_enabled.get()
                        on:change=on_toggle_keyboard_shortcuts />
                </label>
                <div class="settings-hint">
                    <button class="settings-link-btn" on:click=on_open_shortcuts>"View shortcuts"</button>
                </div>
                {env_info(state, "BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS", settings::DEFAULT_KEYBOARD_SHORTCUTS.to_string(), Signal::derive(move || state.keyboard_shortcuts_enabled.get().to_string()))}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Touch swipe gestures"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.touch_swipe_enabled.get()
                        on:change=on_toggle_touch_swipe />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_TOUCH_SWIPE", settings::DEFAULT_TOUCH_SWIPE.to_string(), Signal::derive(move || state.touch_swipe_enabled.get().to_string()))}
                <div class=move || if state.touch_swipe_enabled.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Swipe right distance (px)"</span>
                        <input type="number" class="settings-number" min="40" max="150"
                            prop:value=move || state.swipe_threshold_right.get().to_string()
                            on:change=on_change_swipe_threshold_right />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT", settings::DEFAULT_SWIPE_THRESHOLD_RIGHT.to_string(), Signal::derive(move || state.swipe_threshold_right.get().to_string()))}
                    </div>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Swipe left distance (px)"</span>
                        <input type="number" class="settings-number" min="40" max="150"
                            prop:value=move || state.swipe_threshold_left.get().to_string()
                            on:change=on_change_swipe_threshold_left />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT", settings::DEFAULT_SWIPE_THRESHOLD_LEFT.to_string(), Signal::derive(move || state.swipe_threshold_left.get().to_string()))}
                    </div>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Swipe undo timeout (ms)"</span>
                        <input type="number" class="settings-number" min="500" max="30000" step="500"
                            prop:value=move || state.swipe_undo_timeout_ms.get().to_string()
                            on:change=on_change_swipe_undo_timeout />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS", settings::DEFAULT_SWIPE_UNDO_TIMEOUT_MS.to_string(), Signal::derive(move || state.swipe_undo_timeout_ms.get().to_string()))}
                    </div>
                </div>
            </div>

            // ── Appearance ──
            <div class="settings-section-title">"Appearance"</div>

            <div class="settings-section">
                <div class="settings-item">
                    <span class="settings-label">"UI scale (%)"</span>
                    <input type="number" class="settings-number" min="50" max="300"
                        prop:value=move || state.ui_scale.get().to_string()
                        on:change=on_change_ui_scale />
                </div>
                {env_info(state, "BLAZELIST_DEFAULT_UI_SCALE", settings::DEFAULT_UI_SCALE.to_string(), Signal::derive(move || state.ui_scale.get().to_string()))}
            </div>

            <div class="settings-section">
                <div class="settings-item">
                    <span class="settings-label">"Density"</span>
                    <select class="settings-select"
                        on:change=on_change_density
                        prop:value=move || state.ui_density.get()>
                        <option value="compact">"Compact"</option>
                        <option value="cozy">"Cozy"</option>
                    </select>
                </div>
                {env_info(state, "BLAZELIST_DEFAULT_UI_DENSITY", settings::DEFAULT_UI_DENSITY.to_string(), Signal::derive(move || state.ui_density.get().to_string()))}
            </div>

            // ── Layout ──
            <div class="settings-section-title">"Layout"</div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Show card last-edited time"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.show_card_time.get()
                        on:change=on_toggle_show_card_time />
                </label>
                <div class="settings-hint">
                    "Show the relative-time label (\"x ago\") on the right of each card row. Off by default."
                </div>
                {env_info(state, "BLAZELIST_DEFAULT_SHOW_CARD_TIME", settings::DEFAULT_SHOW_CARD_TIME.to_string(), Signal::derive(move || state.show_card_time.get().to_string()))}
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Override sidebar width"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.override_sidebar_width.get()
                        on:change=on_toggle_override_sidebar />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH", settings::DEFAULT_OVERRIDE_SIDEBAR_WIDTH.to_string(), Signal::derive(move || state.override_sidebar_width.get().to_string()))}
                <div class=move || if state.override_sidebar_width.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Width (px)"</span>
                        <input type="number" class="settings-number" min="80" max="400" step="10"
                            prop:value=move || state.default_sidebar_width.get().to_string()
                            on:change=on_change_sidebar_width />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_SIDEBAR_WIDTH", settings::DEFAULT_SIDEBAR_WIDTH.to_string(), Signal::derive(move || state.default_sidebar_width.get().to_string()))}
                    </div>
                </div>
            </div>

            <div class="settings-section">
                <label class="settings-item">
                    <span class="settings-label">"Override detail panel width"</span>
                    <input type="checkbox" class="toggle-checkbox"
                        prop:checked=move || state.override_detail_width.get()
                        on:change=on_toggle_override_detail />
                </label>
                {env_info(state, "BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH", settings::DEFAULT_OVERRIDE_DETAIL_WIDTH.to_string(), Signal::derive(move || state.override_detail_width.get().to_string()))}
                <div class=move || if state.override_detail_width.get() { "" } else { "settings-disabled" }>
                    <div class="settings-sub-item">
                        <span class="settings-label">"Width (px)"</span>
                        <input type="number" class="settings-number" min="280" max="1200" step="10"
                            prop:value=move || state.default_detail_width.get().to_string()
                            on:change=on_change_detail_width />
                    </div>
                    <div class="settings-sub-env">
                        {env_info(state, "BLAZELIST_DEFAULT_DETAIL_WIDTH", settings::DEFAULT_DETAIL_WIDTH.to_string(), Signal::derive(move || state.default_detail_width.get().to_string()))}
                    </div>
                </div>
            </div>

            // ── Danger Zone ──
            <div class="settings-section-title">"Danger Zone"</div>
            <div class="settings-section">
                <button class="settings-reset-btn" on:click=move |_| {
                    if js_confirm("Clear all local data and reload?\n\nAll cached cards, tags, history, and link graph data will be cleared. The app will perform a full sync from the server.") {
                        leptos::task::spawn_local(async move {
                            storage::clear_all_caches().await;
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().reload();
                            }
                        });
                    }
                }>"Clear local cache"</button>
                <div class="settings-hint">
                    "Clears all locally cached data and reloads for a full re-sync"
                </div>
            </div>
            <div class="settings-section">
                <button class="settings-reset-btn" on:click=move |_| {
                    if js_confirm("Reset all settings to defaults?\n\nThis will clear all saved preferences and reload the page.") {
                        settings::clear_all_settings();
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().reload();
                        }
                    }
                }>"Reset all settings to defaults"</button>
                <div class="settings-hint">
                    "Clears all saved preferences and reloads the page"
                </div>
            </div>
        </div>
    }
}

/// Apply the UI scale to the root element.
pub fn apply_ui_scale(pct: u32) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document())
        && let Some(root) = doc.document_element()
    {
        let _ = root
            .unchecked_ref::<web_sys::HtmlElement>()
            .style()
            .set_property("font-size", &format!("{}%", pct));
    }
}

/// Apply the UI density class to the root element.
pub fn apply_ui_density(density: &str) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document())
        && let Some(root) = doc.document_element()
    {
        let cl = root.class_list();
        let _ = cl.remove_1("density-compact");
        let _ = cl.remove_1("density-cozy");
        let class = format!("density-{density}");
        let _ = cl.add_1(&class);
    }
}

/// Toggle the `show-card-time` class on the root element. When absent
/// (the default), CSS hides `.card-time` across all viewports; when
/// present, the element is visible.
pub fn apply_show_card_time(enabled: bool) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document())
        && let Some(root) = doc.document_element()
    {
        let cl = root.class_list();
        if enabled {
            let _ = cl.add_1("show-card-time");
        } else {
            let _ = cl.remove_1("show-card-time");
        }
    }
}

use wasm_bindgen::JsCast;

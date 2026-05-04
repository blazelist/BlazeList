use crate::components::settings_panel::SettingsButton;
use crate::components::sync_indicator::SyncIndicator;
use crate::state::store::{AppState, clear_all_state};
use leptos::prelude::*;

#[component]
pub fn Header() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let on_title_click = move |_| {
        clear_all_state(&state);
    };

    let toggle_sidebar = move |_| {
        state.sidebar_visible.update(|v| *v = !*v);
    };

    let toggle_detail_expanded = move |_| {
        state.detail_expanded.update(|v| *v = !*v);
    };

    let detail_toggle_label = move || {
        if state.detail_expanded.get() {
            // Collapse-to-side-panel glyph.
            "\u{2922}"
        } else {
            // Expand-to-fullscreen glyph.
            "\u{26F6}"
        }
    };

    let detail_toggle_title = move || {
        if state.detail_expanded.get() {
            "Collapse detail to side panel"
        } else {
            "Expand detail to fullscreen"
        }
    };

    view! {
        <header class="app-header">
            <div class="header-left">
                <button class="sidebar-toggle-btn" on:click=toggle_sidebar
                    title="Toggle sidebar"
                >
                    {"\u{2630}"}
                </button>
                <button class="detail-expand-btn" on:click=toggle_detail_expanded
                    title=detail_toggle_title
                >
                    {detail_toggle_label}
                </button>
                <h1 class="app-title" on:click=on_title_click>"BlazeList"</h1>
            </div>
            <div class="header-right">
                <SettingsButton />
                <SyncIndicator />
            </div>
        </header>
    }
}

use crate::components::icons::{IconCollapse, IconFullscreen, IconMenu};
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

    let detail_toggle_icon = move || {
        if state.detail_expanded.get() {
            view! { <IconCollapse /> }.into_any()
        } else {
            view! { <IconFullscreen /> }.into_any()
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
                    <IconMenu />
                </button>
                <button class="detail-expand-btn" on:click=toggle_detail_expanded
                    title=detail_toggle_title
                >
                    {detail_toggle_icon}
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

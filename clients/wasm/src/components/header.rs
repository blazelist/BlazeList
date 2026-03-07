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

    view! {
        <header class="app-header">
            <div class="header-left">
                <button class="sidebar-toggle-btn" on:click=toggle_sidebar
                    title="Toggle sidebar"
                >
                    {move || if state.sidebar_visible.get() { "\u{2630}" } else { "\u{2630}" }}
                </button>
                <h1 class="app-title" on:click=on_title_click>"BlazeList"</h1>
            </div>
            <SyncIndicator />
        </header>
    }
}

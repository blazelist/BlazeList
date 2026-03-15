use crate::app::request_reconnect;
use crate::state::store::{AppState, ConnectionStatus, get_client};
use crate::state::sync::incremental_sync;
use leptos::prelude::*;

#[component]
pub fn SyncIndicator() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let status_text = move || match state.connection_status.get() {
        ConnectionStatus::Connected => "Connected".to_string(),
        ConnectionStatus::Connecting => "Connecting...".to_string(),
        ConnectionStatus::Syncing => "Syncing...".to_string(),
        ConnectionStatus::Disconnected => {
            let secs = state.reconnect_countdown.get();
            if secs > 0 {
                format!("Disconnected \u{00b7} Retrying in {secs}s")
            } else {
                "Disconnected".to_string()
            }
        }
    };

    let status_class = move || match state.connection_status.get() {
        ConnectionStatus::Connected => "status-connected",
        ConnectionStatus::Connecting => "status-connecting",
        ConnectionStatus::Syncing => "status-syncing",
        ConnectionStatus::Disconnected => "status-disconnected",
    };

    let is_clickable = move || {
        matches!(
            state.connection_status.get(),
            ConnectionStatus::Connected | ConnectionStatus::Disconnected
        )
    };

    let on_click = move |_| match state.connection_status.get() {
        ConnectionStatus::Connected => {
            let Some(client) = get_client() else { return };
            leptos::task::spawn_local(async move {
                if let Err(e) = incremental_sync(&client, &state).await {
                    log::error!("Manual synchronization failed: {e}");
                }
            });
        }
        ConnectionStatus::Disconnected => {
            request_reconnect();
        }
        _ => {}
    };

    let indicator_class = move || {
        if is_clickable() {
            "sync-indicator sync-clickable"
        } else {
            "sync-indicator"
        }
    };

    let title_text = move || match state.connection_status.get() {
        ConnectionStatus::Connected => "Click to sync",
        ConnectionStatus::Disconnected => "Click to reconnect",
        _ => "",
    };

    let debounce_text = move || {
        let countdown = state.push_debounce_countdown.get();
        if countdown > 0 {
            Some(format!("Pushing in {countdown}s"))
        } else {
            None
        }
    };

    let auto_sync_text = move || {
        let countdown = state.auto_sync_countdown.get();
        if countdown > 0 && state.auto_sync_enabled.get() {
            Some(format!("Syncing in {countdown}s"))
        } else {
            None
        }
    };

    view! {
        <div class=indicator_class on:click=on_click
            title=title_text
        >
            {move || {
                debounce_text().map(|text| view! {
                    <span class="sync-detail">{text}</span>
                    <span class="sync-sep">{"\u{00b7}"}</span>
                })
            }}
            {move || {
                auto_sync_text().map(|text| view! {
                    <span class="sync-detail">{text}</span>
                    <span class="sync-sep">{"\u{00b7}"}</span>
                })
            }}
            <span class=status_class>{status_text}</span>
            <span
                class="sync-btn"
                title="Sync now"
                style:visibility=move || if is_clickable() { "visible" } else { "hidden" }
            >{"\u{21bb}"}</span>
        </div>
    }
}

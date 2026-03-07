use crate::app::request_reconnect;
use crate::state::store::{AppState, ConnectionStatus, format_relative_time, get_client};
use crate::state::sync::incremental_sync;
use leptos::prelude::*;

#[component]
pub fn SyncIndicator() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let status_text = move || match state.connection_status.get() {
        ConnectionStatus::Connected => "Connected".to_string(),
        ConnectionStatus::Connecting => "Connecting...".to_string(),
        ConnectionStatus::Syncing => "Syncing...".to_string(),
        ConnectionStatus::Disconnected => "Disconnected".to_string(),
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

    let sync_time_text = move || {
        // Read tick to re-evaluate periodically
        let _ = state.tick.get();
        state.last_synced.get().map(|ts| format_relative_time(&ts))
    };

    let on_click = move |_| match state.connection_status.get() {
        ConnectionStatus::Connected => {
            let Some(client) = get_client() else { return };
            leptos::task::spawn_local(async move {
                if let Err(e) = incremental_sync(&client, &state).await {
                    log::error!("Manual sync failed: {e}");
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

    view! {
        <div class=indicator_class on:click=on_click
            title=title_text
        >
            <span class=status_class>{status_text}</span>
            {move || sync_time_text().map(|t| view! {
                <span class="sync-sep">{"\u{00b7}"}</span>
                <span class="sync-time">{t}</span>
            })}
            {move || matches!(state.connection_status.get(), ConnectionStatus::Connected).then(|| view! {
                <span class="sync-btn" title="Sync now">{"\u{21bb}"}</span>
            })}
        </div>
    }
}

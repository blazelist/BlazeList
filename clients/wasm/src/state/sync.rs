use crate::state::store::{AppState, ConnectionStatus};
use crate::transport::client::Client;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::sync;
use blazelist_protocol::{NonNegativeI64, ZERO_HASH};
use leptos::prelude::*;
use std::rc::Rc;

/// Perform initial sync: fetch everything from the server via GetChangesSince(0).
pub async fn initial_sync(client: &Client, state: &AppState) -> Result<(), String> {
    state.connection_status.set(ConnectionStatus::Syncing);

    let changes = client
        .get_changes_since(NonNegativeI64::MIN, ZERO_HASH)
        .await
        .map_err(|e| e.to_string())?;

    state.cards.set(changes.cards);
    state.tags.set(changes.tags);
    state.deleted_count.set(changes.deleted.len());
    state.root.set(Some(changes.root));
    state.connection_status.set(ConnectionStatus::Connected);
    state.last_synced.set(Some(blazelist_protocol::Utc::now()));

    Ok(())
}

/// Perform incremental sync using GetChangesSince.
pub async fn incremental_sync(client: &Client, state: &AppState) -> Result<(), String> {
    let root = state.root.get_untracked();
    let Some(root) = root else {
        // No root state yet, do full sync
        return initial_sync(client, state).await;
    };

    state.connection_status.set(ConnectionStatus::Syncing);

    let changes = client
        .get_changes_since(root.sequence, root.hash)
        .await
        .map_err(|e| e.to_string())?;

    // Apply changes using shared SDK logic
    let deleted_in_changeset = changes.deleted.len();
    let current_cards = state.cards.get_untracked();
    let current_tags = state.tags.get_untracked();
    state
        .cards
        .set(sync::apply_card_changeset(current_cards, &changes));
    state
        .tags
        .set(sync::apply_tag_changeset(current_tags, &changes));
    state.root.set(Some(changes.root));
    state.connection_status.set(ConnectionStatus::Connected);
    state.last_synced.set(Some(blazelist_protocol::Utc::now()));
    state.deleted_count.update(|c| *c += deleted_in_changeset);

    Ok(())
}

/// Start a subscription for real-time notifications.
/// Spawns a local task that reads notifications and triggers incremental sync.
pub fn start_subscription(client: Rc<Client>, state: AppState) {
    leptos::task::spawn_local(async move {
        let mut handle = match client.subscribe().await {
            Ok(h) => h,
            Err(e) => {
                log::error!("Failed to subscribe: {e}");
                return;
            }
        };

        loop {
            match handle.next_notification().await {
                Ok(server_root) => {
                    // Check if we're already up to date
                    let local_root = state.root.get_untracked();
                    if let Some(local) = local_root {
                        if local.hash == server_root.hash && local.sequence == server_root.sequence
                        {
                            continue;
                        }
                    }
                    // Trigger incremental sync
                    if let Err(e) = incremental_sync(&client, &state).await {
                        log::error!("Incremental sync failed: {e}");
                    }
                }
                Err(e) => {
                    log::error!("Subscribe stream error: {e}");
                    state.connection_status.set(ConnectionStatus::Disconnected);
                    break;
                }
            }
        }
    });
}

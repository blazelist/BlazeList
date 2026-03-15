use crate::state::store::{AppState, ConnectionStatus};
use crate::storage;
use crate::transport::client::Client;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::error::ClientError;
use blazelist_client_lib::sync;
use blazelist_protocol::{DeletedEntity, NonNegativeI64, ProtocolError, ZERO_HASH};
use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// Track deleted entities across syncs so they can be persisted to OPFS.
// The WASM UI only needs the count, but the local DB stores the full list
// to prevent re-creation from stale state during future offline edits.
thread_local! {
    static DELETED_ENTITIES: RefCell<Vec<DeletedEntity>> = RefCell::new(Vec::new());
}

/// Load persisted state from OPFS into [`AppState`].
///
/// Returns `true` if the local DB contained data (non-empty root sequence),
/// allowing the UI to render immediately before the network sync completes.
pub async fn load_local_state(state: &AppState) -> bool {
    let local = storage::load().await;
    let has_data = local.root.sequence != NonNegativeI64::MIN;

    if has_data {
        state.cards.set(local.cards);
        state.tags.set(local.tags);
        state.deleted_count.set(local.deleted_entities.len());
        state.root.set(Some(local.root));
        DELETED_ENTITIES.with(|de| *de.borrow_mut() = local.deleted_entities);
        log::info!("Loaded local state from Origin Private File System");
    }

    has_data
}

/// Persist the current [`AppState`] to OPFS.
async fn save_local_state(state: &AppState) {
    let root = state.root.get_untracked();
    let Some(root) = root else { return };

    let deleted_entities = DELETED_ENTITIES.with(|de| de.borrow().clone());

    let local = storage::LocalState {
        cards: state.cards.get_untracked(),
        tags: state.tags.get_untracked(),
        deleted_entities,
        root,
    };
    storage::save(&local).await;
}

/// Perform initial sync: fetch everything from the server via GetChangesSince(0).
pub async fn initial_sync(client: &Client, state: &AppState) -> Result<(), String> {
    state.connection_status.set(ConnectionStatus::Syncing);
    let t0 = js_sys::Date::now();

    let changes = client
        .get_changes_since(NonNegativeI64::MIN, ZERO_HASH)
        .await
        .map_err(|e| e.to_string())?;

    DELETED_ENTITIES.with(|de| *de.borrow_mut() = changes.deleted.clone());
    let ops = changes.cards.len() + changes.tags.len() + changes.deleted.len();
    state.cards.set(changes.cards);
    state.tags.set(changes.tags);
    state.deleted_count.set(changes.deleted.len());
    state.root.set(Some(changes.root));
    state.connection_status.set(ConnectionStatus::Connected);
    state.last_synced.set(Some(blazelist_protocol::Utc::now()));
    state.last_sync_ops.set(ops);
    state.last_sync_duration_ms.set(Some((js_sys::Date::now() - t0).round() as u32));
    state.auto_sync_countdown.set(state.auto_sync_interval_secs.get_untracked());

    save_local_state(state).await;

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
    let t0 = js_sys::Date::now();

    let result = client.get_changes_since(root.sequence, root.hash).await;

    match result {
        Ok(changes) => {
            // Accumulate deleted entities for local persistence.
            let deleted_in_changeset = changes.deleted.len();
            DELETED_ENTITIES.with(|de| de.borrow_mut().extend(changes.deleted.clone()));

            // Apply changes using shared SDK logic
            let current_cards = state.cards.get_untracked();
            let current_tags = state.tags.get_untracked();
            state
                .cards
                .set(sync::apply_card_changeset(current_cards, &changes));
            state
                .tags
                .set(sync::apply_tag_changeset(current_tags, &changes));
            let ops = changes.cards.len() + changes.tags.len() + deleted_in_changeset;
            state.root.set(Some(changes.root));
            state.connection_status.set(ConnectionStatus::Connected);
            state.last_synced.set(Some(blazelist_protocol::Utc::now()));
            state.last_sync_ops.set(ops);
            state.deleted_count.update(|c| *c += deleted_in_changeset);
            state.last_sync_duration_ms.set(Some((js_sys::Date::now() - t0).round() as u32));
            state.auto_sync_countdown.set(state.auto_sync_interval_secs.get_untracked());

            save_local_state(state).await;

            Ok(())
        }
        Err(ClientError::Protocol(ProtocolError::RootHashMismatch { .. })) => {
            // Local state is out of sync with the server.
            // Wipe the local DB and history cache, then perform a full re-sync.
            log::warn!("Root hash mismatch — wiping local database and performing full synchronization");
            storage::clear().await;
            storage::clear_history_cache().await;
            DELETED_ENTITIES.with(|de| de.borrow_mut().clear());
            state.root.set(None);
            initial_sync(client, state).await
        }
        Err(e) => {
            state.connection_status.set(ConnectionStatus::Connected);
            Err(e.to_string())
        }
    }
}

/// Run the subscription loop, reading notifications until the stream breaks.
///
/// Returns `Ok` only if the stream closes cleanly (unlikely in practice),
/// or `Err` when the connection is lost. The caller is responsible for
/// reconnection.
pub async fn run_subscription(client: Rc<Client>, state: &AppState) -> Result<(), String> {
    let mut handle = client
        .subscribe()
        .await
        .map_err(|e| format!("Failed to subscribe: {e}"))?;

    loop {
        match handle.next_notification().await {
            Ok(server_root) => {
                // Check if we're already up to date
                let local_root = state.root.get_untracked();
                if let Some(local) = local_root {
                    if local.hash == server_root.hash && local.sequence == server_root.sequence {
                        continue;
                    }
                }
                // Trigger incremental sync
                if let Err(e) = incremental_sync(&client, state).await {
                    return Err(format!("Incremental sync failed: {e}"));
                }
            }
            Err(e) => {
                return Err(format!("Subscribe stream error: {e}"));
            }
        }
    }
}

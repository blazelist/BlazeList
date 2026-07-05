use crate::components::toast::show_error_toast;
use crate::state::settings;
use crate::state::store::{AppState, ConnectionStatus, get_client};
use crate::storage;
use crate::transport::client::Client;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::error::ClientError;
use blazelist_client_lib::filter::sort_by_priority;
use blazelist_client_lib::priority::{Placement, build_shifted_versions, resolve_collision};
use blazelist_client_lib::sync;
use blazelist_protocol::{
    Card, DeletedEntity, Entity, NonNegativeI64, ProtocolError, PushError, PushItem, ZERO_HASH,
};
use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

// Track deleted entities across syncs so they can be persisted to OPFS.
// The WASM UI only needs the count, but the local DB stores the full list
// to prevent re-creation from stale state during future offline edits.
thread_local! {
    static DELETED_ENTITIES: RefCell<Vec<DeletedEntity>> = const { RefCell::new(Vec::new()) };
}

/// Load persisted state from OPFS into [`AppState`].
///
/// Returns `true` if the local DB contained data (non-empty root sequence),
/// allowing the UI to render immediately before the network sync completes.
pub async fn load_local_state(state: &AppState) -> bool {
    tracing::info!("load_local_state: starting");
    let local = storage::load().await;
    let has_data = local.root.sequence != NonNegativeI64::MIN;

    if has_data {
        state.cards.set(local.cards);
        state.tags.set(local.tags);
        state.deleted_count.set(local.deleted_entities.len());
        state.root.set(Some(local.root));
        DELETED_ENTITIES.with(|de| *de.borrow_mut() = local.deleted_entities);
        tracing::info!("load_local_state: populated state from OPFS");
    } else {
        tracing::info!(
            "load_local_state: DB had no usable data (root sequence is MIN) \
             — leaving state at defaults until sync"
        );
    }

    has_data
}

/// Build the client-side cache schema stamp.
///
/// Combines the WASM crate version (covers client-lib and storage layout
/// changes) with the protocol version (covers wire/serialization format
/// changes). Crucially **client-only** — no server version — so the check
/// is self-contained and works offline.
///
/// Format: `"{wasm_version}+{protocol_version}"`.
pub fn build_schema_stamp() -> String {
    let wasm_version = env!("CARGO_PKG_VERSION");
    let protocol_version = blazelist_protocol::PROTOCOL_VERSION_STR;
    format!("{wasm_version}+{protocol_version}")
}

/// Returns `true` if the stored schema stamp matches the running build,
/// meaning OPFS caches written by a previous session can still be trusted.
/// Returns `false` if there is no stored stamp (fresh install) or if the
/// stamp is from a different client version.
pub fn cache_schema_matches() -> bool {
    let expected = build_schema_stamp();
    settings::load_cache_schema().as_deref() == Some(&expected)
}

/// Persist the current [`AppState`] to OPFS and update the cache schema
/// stamp. The stamp write is what makes future reloads trust the file.
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

    // Stamp the cache with the current client schema so future reloads
    // know the data on disk matches the running build. A fresh install
    // has no stamp, so the first successful save writes it.
    settings::save_cache_schema(&build_schema_stamp());
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
    state.last_synced.set(Some(blazelist_protocol::Utc::now()));
    state.last_sync_ops.set(ops);
    state
        .last_sync_duration_ms
        .set(Some((js_sys::Date::now() - t0).round() as u32));
    state
        .auto_sync_countdown_ms
        .set(state.auto_sync_interval_ms.get_untracked());
    state.last_sync_error.set(None);

    save_local_state(state).await;

    // Pre-fetch all histories in the background for offline access.
    spawn_prefetch_all_histories();

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

            // Collect IDs of changed cards/tags for incremental history pre-fetch.
            let changed_card_ids: Vec<Uuid> = changes.cards.iter().map(|c| c.id()).collect();
            let changed_tag_ids: Vec<Uuid> = changes.tags.iter().map(|t| t.id()).collect();

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
            state.last_synced.set(Some(blazelist_protocol::Utc::now()));
            state.last_sync_ops.set(ops);
            state.deleted_count.update(|c| *c += deleted_in_changeset);
            state
                .last_sync_duration_ms
                .set(Some((js_sys::Date::now() - t0).round() as u32));
            state
                .auto_sync_countdown_ms
                .set(state.auto_sync_interval_ms.get_untracked());
            state.last_sync_error.set(None);

            save_local_state(state).await;

            // Pre-fetch histories for changed entities in the background.
            if !changed_card_ids.is_empty() || !changed_tag_ids.is_empty() {
                let card_ids = if changed_card_ids.is_empty() {
                    None
                } else {
                    Some(changed_card_ids)
                };
                let tag_ids = if changed_tag_ids.is_empty() {
                    None
                } else {
                    Some(changed_tag_ids)
                };
                spawn_prefetch_histories(card_ids, tag_ids);
            }

            // Flush any offline-queued cards now that we have a working connection.
            flush_offline_queue(client, state).await;

            Ok(())
        }
        Err(ClientError::Protocol(ProtocolError::RootHashMismatch { .. })) => {
            // Local state is out of sync with the server. Force a full
            // resync, but do NOT eagerly delete blazelist.db — if the
            // follow-up initial_sync fails (e.g., network drop), an
            // eagerly-wiped DB would leave the user with an empty app.
            // Let initial_sync + save_local_state overwrite the DB
            // atomically on success; the old data stays as a fallback
            // on failure. The history cache is derived and safe to
            // rebuild lazily.
            tracing::warn!("Root hash mismatch — forcing full synchronization");
            storage::clear_history_cache().await;
            DELETED_ENTITIES.with(|de| de.borrow_mut().clear());
            state.root.set(None);
            let result = initial_sync(client, state).await;
            if result.is_ok() {
                flush_offline_queue(client, state).await;
            }
            result
        }
        Err(e) => {
            let msg = e.to_string();
            state.last_sync_error.set(Some(msg.clone()));
            Err(msg)
        }
    }
}

/// Rebuild `card` at the resolved `priority`, preserving its content/tags/etc.
///
/// With an `ancestor`, chain a new version onto it (rebase the edit onto the
/// server's latest), stamping `modified_at` to now. Without one, recreate the
/// card as a fresh first version, **preserving the original `created_at`** so
/// the recreated card keeps its original creation timestamp. This now/created
/// distinction between the `.next()` and `.first()` paths is intentional.
fn rebuild_card(card: &Card, priority: i64, ancestor: Option<&Card>) -> Card {
    match ancestor {
        Some(a) => a.next(
            card.content().to_string(),
            priority,
            card.tags().to_vec(),
            card.blazed(),
            blazelist_protocol::Utc::now(),
            card.due_date(),
        ),
        None => Card::first(
            card.id(),
            card.content().to_string(),
            priority,
            card.tags().to_vec(),
            card.blazed(),
            card.created_at(),
            card.due_date(),
        ),
    }
}

/// Push a card to the server, falling back to the offline queue on failure.
///
/// If the client is `None` (never connected) or the push fails with a
/// connection error, the card is added to the offline queue and persisted
/// to OPFS.  The queue is flushed automatically on the next successful sync.
pub async fn push_card_or_queue(state: &AppState, card: Card) {
    let card_id = card.id();
    if let Some(client) = get_client() {
        match client.push_card(card.clone()).await {
            Ok(_) => {
                clear_queued_card(state, card_id).await;
                return;
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(
                PushError::CardAncestorMismatch(server_card),
            ))) => {
                // Server has a newer version — rebase our edit on top of it
                // so content is preserved instead of silently lost in the queue.
                tracing::info!(card_id = %card.id(), "Ancestor mismatch, rebasing edit");
                let rebased = rebuild_card(&card, card.priority(), Some(&server_card));
                match client.push_card(rebased.clone()).await {
                    Ok(_) => {
                        state.upsert_card(rebased);
                        clear_queued_card(state, card_id).await;
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "Rebased push also failed, queuing for later");
                    }
                }
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(
                PushError::HashVerificationFailed,
            ))) => {
                // Server doesn't have this card — recreate it as a first
                // version so the user's content is preserved.
                tracing::info!(card_id = %card.id(), "Hash verification failed, recreating card");
                let recreated = rebuild_card(&card, card.priority(), None);
                match client.push_card(recreated.clone()).await {
                    Ok(_) => {
                        state.upsert_card(recreated);
                        clear_queued_card(state, card_id).await;
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "Recreated card push also failed, queuing");
                        // Queue the recreated version (ZERO_HASH ancestor) so
                        // the flush reconciliation won't drop it.
                        state.offline_queue.update(|q| {
                            q.retain(|c| c.id() != card_id);
                            q.push(recreated);
                        });
                        storage::save_offline_queue(&state.offline_queue.get_untracked()).await;
                        // Persist the main OPFS DB too, so offline edits
                        // survive an app restart (queue alone is not enough —
                        // on restart the UI hydrates from blazelist.db).
                        save_local_state(state).await;
                        return;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(%e, "Push failed, queuing for later");
            }
        }
    }
    // Replace any existing entry for the same card (keep only the latest version).
    state.offline_queue.update(|q| {
        q.retain(|c| c.id() != card_id);
        q.push(card);
    });
    storage::save_offline_queue(&state.offline_queue.get_untracked()).await;
    // Persist the main OPFS DB too, so offline edits survive an app
    // restart (queue alone is not enough — on restart the UI hydrates
    // from blazelist.db).
    save_local_state(state).await;
}

/// Remove a card from the offline queue if present, and persist the change.
async fn clear_queued_card(state: &AppState, card_id: uuid::Uuid) {
    let had_queued = state
        .offline_queue
        .with_untracked(|q| q.iter().any(|c| c.id() == card_id));
    if had_queued {
        state
            .offline_queue
            .update(|q| q.retain(|c| c.id() != card_id));
        storage::save_offline_queue(&state.offline_queue.get_untracked()).await;
    }
}

/// Reconcile and flush the offline queue after a successful sync.
///
/// First drops any queued cards the server already has at a strictly newer
/// version (server is source of truth), skipping brand-new cards that have
/// never been pushed.  Then pushes whatever remains.
pub async fn flush_offline_queue(client: &Client, state: &AppState) {
    let queue = state.offline_queue.get_untracked();
    if queue.is_empty() {
        return;
    }

    // Reconcile: drop queued cards the server already has at a strictly newer
    // version.  Brand-new cards (ZERO_HASH ancestor) are always kept.
    let local_cards = state.cards.get_untracked();
    let before = queue.len();
    let queue = sync::reconcile_offline_queue(queue, &local_cards);
    if queue.len() < before {
        tracing::info!(
            dropped = before - queue.len(),
            remaining = queue.len(),
            "Reconciled offline queue",
        );
    }

    if queue.is_empty() {
        state.offline_queue.set(Vec::new());
        storage::save_offline_queue(&[]).await;
        return;
    }

    let total = queue.len();
    tracing::info!(total, "Flushing offline queued cards");
    let mut remaining = Vec::new();
    let mut hit_connection_error = false;

    for card in queue {
        if hit_connection_error {
            remaining.push(card);
            continue;
        }
        match client.push_card(card.clone()).await {
            Ok(_) => {
                // Ensure the card is in local state — may have been wiped
                // by initial_sync (RootHashMismatch recovery) before this
                // flush ran.
                state.upsert_card(card);
            }
            Err(ClientError::ConnectionLost) => {
                tracing::warn!("Connection lost during flush, keeping remaining cards queued");
                remaining.push(card);
                hit_connection_error = true;
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(
                PushError::DuplicatePriority { .. },
            ))) => {
                tracing::warn!(card_id = %card.id(), "Priority collision, recomputing");
                let mut cards = state.cards.get_untracked();
                cards.retain(|c| c.id() != card.id());
                sort_by_priority(&mut cards);

                let is_existing = card.ancestor_hash() != ZERO_HASH;

                // For existing cards, fetch the server's current version to use
                // as ancestor so the version chain is preserved.
                let server_ancestor = if is_existing {
                    match client.get_card(card.id()).await {
                        Ok(sc) => Some(sc),
                        Err(ClientError::ConnectionLost) => {
                            remaining.push(card);
                            hit_connection_error = true;
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!(%e, "Could not fetch server card for rebase, keeping queued");
                            remaining.push(card);
                            continue;
                        }
                    }
                } else {
                    None
                };

                let placement = resolve_collision(&cards, card.priority());
                match placement {
                    Placement::Simple(new_priority) => {
                        let retry = rebuild_card(&card, new_priority, server_ancestor.as_ref());
                        match client.push_card(retry.clone()).await {
                            Ok(_) => {
                                state.upsert_card(retry);
                            }
                            Err(ClientError::ConnectionLost) => {
                                remaining.push(card);
                                hit_connection_error = true;
                            }
                            Err(_) => {
                                remaining.push(card);
                            }
                        }
                    }
                    Placement::Rebalanced { priority, shifted } => {
                        let retry = rebuild_card(&card, priority, server_ancestor.as_ref());
                        let shifted_cards = build_shifted_versions(&shifted, &cards);
                        let mut items: Vec<PushItem> = shifted_cards
                            .iter()
                            .map(|c| PushItem::Cards(vec![c.clone()]))
                            .collect();
                        items.push(PushItem::Cards(vec![retry.clone()]));
                        match client.push_batch(items).await {
                            Ok(_) => {
                                for sc in &shifted_cards {
                                    state.upsert_card(sc.clone());
                                }
                                state.upsert_card(retry);
                            }
                            Err(ClientError::ConnectionLost) => {
                                remaining.push(card);
                                hit_connection_error = true;
                            }
                            Err(_) => {
                                remaining.push(card);
                            }
                        }
                    }
                }
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(
                PushError::CardAncestorMismatch(server_card),
            ))) => {
                // The queued card was built on a stale ancestor hash. Rebase
                // the user's edits onto the server's latest version so content
                // is preserved instead of silently dropped.
                tracing::info!(
                    card_id = %card.id(),
                    "Rebasing queued edit onto server version",
                );
                let rebased = rebuild_card(&card, card.priority(), Some(&server_card));
                match client.push_card(rebased.clone()).await {
                    Ok(_) => {
                        state.upsert_card(rebased);
                    }
                    Err(ClientError::ConnectionLost) => {
                        remaining.push(card);
                        hit_connection_error = true;
                    }
                    Err(e) => {
                        // The rebased version was rejected by the
                        // server (e.g. validation error, permission
                        // denied). The card is already replaced in
                        // `state.cards` by the preceding sync, so the
                        // UI now shows the server's version. Surface
                        // a toast so the user knows their offline
                        // edit is gone instead of silently dropping.
                        tracing::warn!(%e, "Rebased push failed, dropping card");
                        show_error_toast(
                            *state,
                            "An offline edit was dropped: server rejected the rebased version",
                            3000,
                        );
                    }
                }
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(
                PushError::HashVerificationFailed,
            ))) => {
                // Server doesn't have this card at all.  Recreate it as a
                // first version so the user's content is preserved.
                tracing::info!(
                    card_id = %card.id(),
                    "Hash verification failed during flush, recreating card",
                );
                let recreated = rebuild_card(&card, card.priority(), None);
                match client.push_card(recreated.clone()).await {
                    Ok(_) => {
                        state.upsert_card(recreated);
                    }
                    Err(ClientError::ConnectionLost) => {
                        remaining.push(recreated);
                        hit_connection_error = true;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "Recreated card push also failed, keeping queued");
                        remaining.push(recreated);
                    }
                }
            }
            Err(ClientError::Protocol(ProtocolError::PushFailed(PushError::AlreadyDeleted))) => {
                tracing::info!(card_id = %card.id(), "Card deleted on server, dropping from queue");
            }
            Err(e) => {
                // Unhandled error — keep the card queued and stop processing
                // so the next sync cycle's reconciliation can resolve it.
                tracing::warn!(%e, "Keeping queued card for retry");
                remaining.push(card);
                hit_connection_error = true;
            }
        }
    }

    let flushed = remaining.len() < total;
    state.offline_queue.set(remaining.clone());
    storage::save_offline_queue(&remaining).await;

    // Persist the updated card list so OPFS reflects successfully pushed cards.
    if flushed {
        save_local_state(state).await;
    }
}

/// Spawn a non-blocking background task to pre-fetch all card and tag
/// histories from the server and populate the OPFS-backed history cache.
///
/// Used after `initial_sync` so the full history is available offline.
fn spawn_prefetch_all_histories() {
    spawn_prefetch_histories(None, None);
}

/// Spawn a non-blocking background task to pre-fetch histories for specific
/// (or all) cards and tags from the server and merge into the history cache.
///
/// Pass `None` for a parameter to fetch all entities of that type.
fn spawn_prefetch_histories(card_ids: Option<Vec<Uuid>>, tag_ids: Option<Vec<Uuid>>) {
    leptos::task::spawn_local(async move {
        let Some(client) = get_client() else {
            return;
        };

        // Fetch card histories.
        match client.get_all_card_histories(None, card_ids).await {
            Ok(histories) => {
                for (id, history) in histories {
                    storage::update_cached_card_history(id, history);
                }
            }
            Err(e) => {
                tracing::debug!(%e, "Background card history pre-fetch failed");
            }
        }

        // Fetch tag histories.
        match client.get_all_tag_histories(None, tag_ids).await {
            Ok(histories) => {
                for (id, history) in histories {
                    storage::update_cached_tag_history(id, history);
                }
            }
            Err(e) => {
                tracing::debug!(%e, "Background tag history pre-fetch failed");
            }
        }

        // Fetch sequence history.
        match client.get_sequence_history().await {
            Ok(history) => {
                storage::update_cached_sequence_history(history);
            }
            Err(e) => {
                tracing::debug!(%e, "Background sequence history pre-fetch failed");
            }
        }

        // Persist everything to OPFS in one write.
        storage::save_history_cache().await;
        tracing::debug!("History pre-fetch complete");
    });
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
                if let Some(local) = local_root
                    && local.hash == server_root.hash
                    && local.sequence == server_root.sequence
                {
                    continue;
                }
                // Trigger incremental sync
                if let Err(e) = incremental_sync(&client, state).await {
                    return Err(format!("Incremental sync failed: {e}"));
                }
                state.connection_status.set(ConnectionStatus::Connected);
            }
            Err(e) => {
                return Err(format!("Subscribe stream error: {e}"));
            }
        }
    }
}

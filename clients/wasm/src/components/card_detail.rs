use crate::components::card_editor::CardEditor;
use crate::components::hooks::{
    ConfirmDeletePrompt, TagColorPicker, handle_code_copy_click, parse_hex_color,
    use_click_outside_close,
};
use crate::components::keyboard::{copy_to_clipboard, show_copy_toast};
use crate::components::link_indicators::link_indicators_view;
use crate::components::tag_detail::TagDetail;
use crate::components::timestamp::Timestamp;
use crate::components::toast::show_error_toast;
use crate::components::version_history::VersionHistory;
use crate::state::store::{
    AppState, DueDatePreset, DueDateStatus, NewCardPosition, confirm_discard_changes,
    due_date_status, format_due_date_badge, format_due_date_display, get_client, select_card_view,
    sync_query_params, tag_chip_style,
};
use crate::state::sync::{push_card_or_queue, push_versions_or_queue};
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::display::LinkCounts;
use blazelist_client_lib::error::ClientError;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, move_card,
};
use blazelist_client_lib::tag_graph::TagGraph;
use blazelist_protocol::{Card, CardFilter, Entity, PushItem, Tag, Utc};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

const CARD_LINK_PREVIEW_MAX_WIDTH: usize = 80;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setTimeout")]
    fn set_timeout_js(handler: &js_sys::Function, timeout: i32) -> i32;
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout_js(handle: i32);
}

fn render_markdown(
    content: &str,
    card_ids: &std::collections::HashSet<uuid::Uuid>,
    card_previews: &std::collections::HashMap<uuid::Uuid, String>,
    blazed_ids: &std::collections::HashSet<uuid::Uuid>,
) -> String {
    let html =
        comrak::markdown_to_html(content, &blazelist_client_lib::display::markdown_options());
    // comrak renders checkboxes with disabled="" — remove it so clicks fire
    let html = html.replace(" disabled=\"\"", "");
    let html = blazelist_client_lib::display::linkify_card_uuids_with_previews(
        &html,
        card_ids,
        card_previews,
        blazed_ids,
    );
    blazelist_client_lib::display::wrap_code_blocks_with_copy_button(&html)
}

/// Flush pending debounced versions (fire-and-forget).
pub(crate) fn flush_pending(state: &AppState) {
    let handle = state.debounce_timeout_handle;
    let pending = state.pending_versions;
    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
        handle.set(0);
        state.push_debounce_countdown.set(0);
    }
    let versions = pending.get_untracked();
    pending.set(Vec::new());
    if !versions.is_empty() {
        let state = *state;
        leptos::task::spawn_local(async move {
            push_versions_or_queue(&state, versions).await;
        });
    }
}

/// Drain pending debounced versions and return them.
fn drain_pending(state: &AppState) -> Vec<Card> {
    let handle = state.debounce_timeout_handle;
    let pending = state.pending_versions;
    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
        handle.set(0);
        state.push_debounce_countdown.set(0);
    }
    let versions = pending.get_untracked();
    pending.set(Vec::new());
    versions
}

/// Schedule a debounced push of a card version.
pub(crate) fn schedule_push(updated: Card, state: AppState) {
    let pending = state.pending_versions;
    let pending_card_id = state.pending_card_id;
    let handle = state.debounce_timeout_handle;

    let card_id = updated.id();
    state.cards.update(|cards| {
        cards.retain(|c| c.id() != card_id);
        cards.push(updated.clone());
    });

    if pending_card_id.get_untracked() != Some(card_id) {
        let old_versions = pending.get_untracked();
        if !old_versions.is_empty() {
            pending.set(Vec::new());
            leptos::task::spawn_local(async move {
                push_versions_or_queue(&state, old_versions).await;
            });
        }
    }
    pending_card_id.set(Some(card_id));
    pending.update(|v| v.push(updated));

    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
    }

    // When debounce is disabled, push immediately without a timeout.
    if !state.debounce_enabled.get_untracked() {
        let versions = pending.get_untracked();
        pending.set(Vec::new());
        handle.set(0);
        pending_card_id.set(None);
        state.push_debounce_countdown.set(0);
        if !versions.is_empty() {
            leptos::task::spawn_local(async move {
                push_versions_or_queue(&state, versions).await;
            });
        }
        return;
    }

    let cb = Closure::once(move || {
        let versions = pending.get_untracked();
        pending.set(Vec::new());
        handle.set(0);
        pending_card_id.set(None);
        state.push_debounce_countdown.set(0);
        if !versions.is_empty() {
            leptos::task::spawn_local(async move {
                push_versions_or_queue(&state, versions).await;
            });
        }
    });
    let func = cb.into_js_value();
    let delay_secs = state.debounce_delay_secs.get_untracked();
    let delay_ms = delay_secs as i32 * 1000;
    state.push_debounce_countdown.set(delay_secs);
    let new_handle = set_timeout_js(func.unchecked_ref(), delay_ms);
    handle.set(new_handle);
}

/// Apply a move placement result: update the moved card locally and push
/// shifted cards via batch if rebalancing occurred.
pub(crate) fn apply_move_placement(
    placement: Placement,
    current: &Card,
    all_cards: &[Card],
    state: AppState,
) {
    match placement {
        Placement::Simple(new_priority) => {
            let updated = current.next(
                current.content().to_string(),
                new_priority,
                current.tags().to_vec(),
                current.blazed(),
                Utc::now(),
                current.due_date(),
            );
            schedule_push(updated, state);
        }
        Placement::Rebalanced { priority, shifted } => {
            let updated = current.next(
                current.content().to_string(),
                priority,
                current.tags().to_vec(),
                current.blazed(),
                Utc::now(),
                current.due_date(),
            );
            let shifted_cards = build_shifted_versions(&shifted, all_cards);

            // Update local state
            state.upsert_card(updated.clone());
            for sc in &shifted_cards {
                state.upsert_card(sc.clone());
            }

            // Push batch: shifted cards + moved card.
            // Batch pushes require a connection; queue the moved card on failure.
            leptos::task::spawn_local(async move {
                if let Some(client) = get_client() {
                    let mut items: Vec<PushItem> = shifted_cards
                        .into_iter()
                        .map(|c| PushItem::Cards(vec![c]))
                        .collect();
                    items.push(PushItem::Cards(vec![updated.clone()]));
                    match client.push_batch(items).await {
                        Ok(_) => return,
                        Err(e) => tracing::warn!(%e, "Batch push failed, queuing moved card"),
                    }
                }
                push_card_or_queue(&state, updated).await;
            });
        }
    }
}

#[component]
pub fn CardDetail() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let confirm_delete = RwSignal::new(0u8);
    let move_to_input = RwSignal::new(String::new());
    let due_preset = RwSignal::new(DueDatePreset::Today);
    let due_dropdown_open = RwSignal::new(false);

    let due_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(due_dropdown_open, due_group_ref);

    on_cleanup(move || {
        let old = state.debounce_timeout_handle.get_untracked();
        if old != 0 {
            clear_timeout_js(old);
        }
        state.pending_versions.set(Vec::new());
    });

    // Watch for keyboard-triggered delete request.
    Effect::new(move |_| {
        if state.delete_requested.get() {
            state.delete_requested.set(false);
            // Only trigger if a card (not tag) is selected and not editing.
            if state.selected_card.get_untracked().is_some()
                && !state.editing.get_untracked()
                && !state.creating_new.get_untracked()
            {
                confirm_delete.set(1);
            }
        }
    });

    let close_action = move || {
        if !confirm_discard_changes(&state) {
            return;
        }
        flush_pending(&state);
        state.selected_card.set(None);
        state.creating_new.set(false);
        state.creating_new_tag.set(false);
        state.editing.set(false);
        state.has_unsaved_changes.set(false);
        confirm_delete.set(0);
        sync_query_params(&state);
    };
    let on_close = move |_: web_sys::MouseEvent| close_action();

    view! {
        {move || {
            // --- Creating new card ---
            if state.creating_new.get() {
                let position_hint = move || {
                    let pos = state.new_card_position.get();
                    // Use get_untracked to avoid the parent DynChild tracking
                    // `state.cards` — auto-sync replacing cards must not
                    // destroy the editor and lose unsaved changes.
                    let cards = state.cards.get_untracked();
                    match pos {
                        NewCardPosition::Bottom => "Adding to bottom".to_string(),
                        NewCardPosition::Top => "Adding to top".to_string(),
                        NewCardPosition::Above(id) => {
                            let preview = cards.iter().find(|c| c.id() == id)
                                .and_then(|c| blazelist_client_lib::display::card_preview(c.content(), 40));
                            match preview {
                                Some(p) => format!("Adding above \u{201c}{p}\u{201d}"),
                                None => "Adding above selected".to_string(),
                            }
                        }
                        NewCardPosition::Below(id) => {
                            let preview = cards.iter().find(|c| c.id() == id)
                                .and_then(|c| blazelist_client_lib::display::card_preview(c.content(), 40));
                            match preview {
                                Some(p) => format!("Adding below \u{201c}{p}\u{201d}"),
                                None => "Adding below selected".to_string(),
                            }
                        }
                    }
                };
                // If auto-save has already persisted this card, recover it
                // so the editor survives a potential DynChild re-render.
                let existing_card = state.selected_card.get_untracked()
                    .and_then(|id| state.cards.get_untracked().into_iter().find(|c| c.id() == id));
                let on_save_cb = move || {
                    state.creating_new.set(false);
                    state.new_card_prefill.set(None);
                };
                let on_cancel_cb = Callback::new(move |_: ()| {
                    if !confirm_discard_changes(&state) { return; }
                    state.creating_new.set(false);
                    state.new_card_prefill.set(None);
                });
                let editor_view = if let Some(card) = existing_card {
                    view! { <CardEditor editing_card=card on_save=on_save_cb on_cancel=on_cancel_cb /> }.into_any()
                } else {
                    view! { <CardEditor on_save=on_save_cb on_cancel=on_cancel_cb /> }.into_any()
                };
                return Some(view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <div class="detail-header-left">
                                {move || if state.selected_card.get().is_some() {
                                    view! { <span class="detail-status editing">"Editing"</span> }.into_any()
                                } else {
                                    view! { <span class="detail-status active">"New Card"</span> }.into_any()
                                }}
                                {move || state.selected_card.get().is_none().then(|| view! {
                                    <span class="new-card-position-hint">{position_hint}</span>
                                })}
                                {move || state.has_unsaved_changes.get().then(|| view! {
                                    <span class="unsaved-indicator">"(unsaved)"</span>
                                })}
                            </div>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        {editor_view}
                    </div>
                }.into_any());
            }

            // --- Creating new tag ---
            if state.creating_new_tag.get() {
                return Some(view! {
                    <div class="card-detail">
                        <NewTagForm on_close=move |()| close_action() />
                    </div>
                }.into_any());
            }

            // --- Selected card ---
            let selected_id = state.selected_card.get();
            selected_id?;
            let selected_id = selected_id.unwrap();

            // Check if the selected ID is a tag — render TagDetail without
            // reactively tracking `cards` or `tags` so that auto-sync
            // updating the signals does not destroy the component and
            // lose unsaved edits.
            if state.tags.get_untracked().iter().any(|t| t.id() == selected_id) {
                return Some(view! {
                    <div class="card-detail">
                        <TagDetail />
                    </div>
                }.into_any());
            }

            // Don't reactively track `cards` — auto-sync updating the
            // signal would destroy the component and lose version history
            // expansion state (and editor content when editing).
            let editing_now = state.editing.get_untracked();
            let card = state.cards.get_untracked()
                .into_iter().find(|c| c.id() == selected_id)
                .or_else(|| {
                    // Tracked fallback: subscribe so the closure re-runs
                    // when cards arrive (e.g., page reload before sync
                    // finishes).  Once found via untracked above, this
                    // path is never reached and the dependency is dropped.
                    state.cards.get()
                        .into_iter().find(|c| c.id() == selected_id)
                });

            if card.is_none() {
                // Tracked fallback: subscribes to tags so the closure
                // re-runs when tags arrive (e.g., page reload before
                // sync finishes).  Once found, the untracked early-return
                // above takes over and this subscription is dropped.
                if state.tags.get().iter().any(|t| t.id() == selected_id) {
                    return Some(view! {
                        <div class="card-detail">
                            <TagDetail />
                        </div>
                    }.into_any());
                }
                let id_str = selected_id.to_string();
                return Some(view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <span class="detail-status deleted">"Not Found"</span>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        <div class="card-content deleted-notice">
                            <p>"Entity not found. It may have been deleted."</p>
                        </div>
                        <div class="detail-meta">
                            <div class="meta-row">
                                <span class="meta-label">"ID"</span>
                                <span class="meta-value">{id_str}</span>
                            </div>
                        </div>
                    </div>
                }.into_any());
            }

            let card = card.unwrap();
            {
            let card_id = card.id();
            // Memo that reactively tracks the blazed status of this card so
            // that the status badge and blaze/extinguish button update when
            // the card is blazed via button click, keyboard shortcut, or swipe
            // without re-rendering the entire DynChild (which would lose
            // editor state, version-history expansion, etc.).
            let is_blazed = Memo::new(move |_| {
                state.cards.get()
                    .iter()
                    .find(|c| c.id() == card_id)
                    .map(|c| c.blazed())
                    .unwrap_or(false)
            });
            let content_raw = card.content().to_string();
            let all_cards_snapshot = state.cards.get_untracked();

            // Build card ID set for linkifying UUIDs in the markdown view.
            let known_card_ids: std::collections::HashSet<uuid::Uuid> =
                all_cards_snapshot.iter().map(|c| c.id()).collect();
            let card_link_previews: std::collections::HashMap<uuid::Uuid, String> =
                all_cards_snapshot
                    .iter()
                    .map(|c| {
                        (
                            c.id(),
                            blazelist_client_lib::display::card_preview(
                                c.content(),
                                CARD_LINK_PREVIEW_MAX_WIDTH,
                            )
                                .unwrap_or_else(|| "(empty)".to_string()),
                        )
                    })
                    .collect();
            let blazed_card_ids: std::collections::HashSet<uuid::Uuid> =
                all_cards_snapshot.iter().filter(|c| c.blazed()).map(|c| c.id()).collect();
            let content_html = render_markdown(&content_raw, &known_card_ids, &card_link_previews, &blazed_card_ids);

            let task_progress = blazelist_client_lib::display::task_progress(&content_raw);
            let content_node_ref = NodeRef::<leptos::html::Div>::new();
            let priority_raw = card.priority();
            let priority_pct = blazelist_client_lib::priority::priority_percentage(priority_raw);
            let count = i64::from(card.count());
            let created = card.created_at();
            let modified = card.modified_at();
            let id_str = card_id.to_string();
            let id_str_copy = id_str.clone();
            let copy_toast_msg = {
                let id_preview: String = id_str.chars().take(8).collect();
                let card_preview = blazelist_client_lib::display::card_preview(&content_raw, 30);
                match card_preview {
                    Some(p) => format!("Copied {id_preview}\u{2026} \u{2014} {p}"),
                    None => format!("Copied {id_preview}\u{2026}"),
                }
            };

            let card_tag_ids = card.tags().to_vec();
            let all_tags = state.tags.get_untracked();
            let mut card_tags_with_ids: Vec<(uuid::Uuid, String, Option<rgb::RGB8>)> = card_tag_ids.iter().filter_map(|tid| {
                let tag = all_tags.iter().find(|t| t.id() == *tid)?;
                Some((*tid, tag.title().to_string(), tag.color()))
            }).collect();
            card_tags_with_ids.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

            let reorder_disabled;
            let filtered;
            if editing_now {
                reorder_disabled = true;
                filtered = Vec::new();
            } else {
                reorder_disabled = !state.reorder_allowed();
                filtered = state.filtered_cards().get();
            }
            let filtered_pos = filtered.iter().position(|c| c.id() == card_id);
            let is_at_top = filtered_pos == Some(0);
            let is_at_bottom = filtered_pos == Some(filtered.len().saturating_sub(1));
            let in_filtered = filtered_pos.is_some() && !reorder_disabled;
            let current_position = filtered_pos.map(|i| i + 1).unwrap_or(0);
            let total_cards = filtered.len();
            if in_filtered {
                move_to_input.set(current_position.to_string());
            } else {
                move_to_input.set(String::new());
            }

            // Memo that reactively tracks the due date of this card so
            // that the due date display, date picker, and clear button
            // update after setting/clearing via button, shortcut, or swipe
            // without re-rendering the entire DynChild.
            let due_date_opt = Memo::new(move |_| {
                state.cards.get()
                    .iter()
                    .find(|c| c.id() == card_id)
                    .and_then(|c| c.due_date())
            });

            // Helper to set due date on a card (creates new version and pushes)
            let set_due_date = move |new_due: Option<chrono::DateTime<Utc>>| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                let Some(current) = current else { return };
                if current.due_date() == new_due { return; }
                let next = current.next(
                    current.content().to_string(),
                    current.priority(),
                    current.tags().to_vec(),
                    current.blazed(),
                    Utc::now(),
                    new_due,
                );
                state.upsert_card(next.clone());
                let state = state;
                leptos::task::spawn_local(async move {
                    push_card_or_queue(&state, next).await;
                });
            };

            let on_blaze = move |_| {
                let pending = drain_pending(&state);
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                let Some(current) = current else { return };
                let next = current.next(
                    current.content().to_string(),
                    current.priority(),
                    current.tags().to_vec(),
                    !current.blazed(),
                    Utc::now(),
                    current.due_date(),
                );
                state.upsert_card(next.clone());
                let state = state;
                leptos::task::spawn_local(async move {
                    if !pending.is_empty() {
                        push_versions_or_queue(&state, pending).await;
                    }
                    push_card_or_queue(&state, next).await;
                });
            };

            let on_delete_click = move |_| {
                confirm_delete.set(1);
            };

            let do_confirm_delete = move || {
                let pending = drain_pending(&state);
                let state = state;
                confirm_delete.set(0);
                leptos::task::spawn_local(async move {
                    if !pending.is_empty() {
                        push_versions_or_queue(&state, pending).await;
                    }
                    // Delete requires a live connection (not queued offline).
                    let Some(client) = get_client() else {
                        show_error_toast(state, "Can't delete cards while offline", 3000);
                        return;
                    };
                    match client.delete_card(card_id).await {
                        Ok(_) => {}
                        Err(ClientError::ConnectionLost) => {
                            show_error_toast(
                                state,
                                "Can't delete cards while offline",
                                3000,
                            );
                            return;
                        }
                        Err(e) => {
                            tracing::error!(%e, "Failed to delete card");
                            show_error_toast(
                                state,
                                &format!("Failed to delete card: {e}"),
                                3000,
                            );
                            return;
                        }
                    }
                    state.cards.update(|cards| cards.retain(|c| c.id() != card_id));
                    state.selected_card.set(None);
                    sync_query_params(&state);
                });
            };

            let do_cancel_delete = move || {
                confirm_delete.set(0);
            };

            let filtered_cards_memo = state.filtered_cards();

            let on_move_top = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let placement = move_card(&filtered, card_id, InsertPosition::Top);
                    apply_move_placement(placement, &current, &filtered, state);
                }
            };

            let on_move_up = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let idx = match filtered.iter().position(|c| c.id() == card_id) {
                        Some(i) => i,
                        None => return,
                    };
                    if idx == 0 { return; }
                    // After removing the card, position idx-1 in the reduced list
                    let placement = move_card(&filtered, card_id, InsertPosition::At(idx - 1));
                    apply_move_placement(placement, &current, &filtered, state);
                }
            };

            let on_move_down = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let idx = match filtered.iter().position(|c| c.id() == card_id) {
                        Some(i) => i,
                        None => return,
                    };
                    if idx >= filtered.len() - 1 { return; }
                    // After removing the card, the card at idx+1 shifts to idx,
                    // so we target idx+1 in the reduced list.
                    let placement = move_card(&filtered, card_id, InsertPosition::At(idx + 1));
                    apply_move_placement(placement, &current, &filtered, state);
                }
            };

            let on_move_bottom = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let placement = move_card(&filtered, card_id, InsertPosition::Bottom);
                    apply_move_placement(placement, &current, &filtered, state);
                }
            };
            let on_move_to = move |_| {
                let input_val = move_to_input.get_untracked();
                let filtered = filtered_cards_memo.get_untracked();
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let total = filtered.len();
                    if total == 0 { return; }
                    let target_pos: usize = input_val.trim().parse().unwrap_or(1).max(1).min(total);
                    let cur_idx = match filtered.iter().position(|c| c.id() == card_id) {
                        Some(i) => i,
                        None => return,
                    };
                    if target_pos - 1 == cur_idx { return; }
                    let placement = move_card(&filtered, card_id, InsertPosition::At(target_pos - 1));
                    apply_move_placement(placement, &current, &filtered, state);
                }
            };

            let on_edit = move |_| {
                flush_pending(&state);
                state.editing.set(true);
            };

            let on_content_click = move |ev: web_sys::MouseEvent| {
                let target = match ev.target() {
                    Some(t) => t,
                    None => return,
                };

                // Check for card UUID link click.
                if let Ok(el) = target.clone().dyn_into::<web_sys::HtmlElement>()
                    && let Ok(Some(link_el)) = el.closest(".card-uuid-link")
                        && let Some(card_id_str) = link_el.get_attribute("data-card-id")
                        && let Ok(target_id) = card_id_str.parse::<uuid::Uuid>()
                    {
                        if !confirm_discard_changes(&state) {
                            return;
                        }
                        flush_pending(&state);
                        state.selected_card.set(Some(target_id));
                        state.editing.set(false);
                        sync_query_params(&state);
                        return;
                    }

                // Check for code-block copy button click.
                if handle_code_copy_click(&ev) {
                    return;
                }

                // Checkbox toggle handling.
                // Accept clicks on the checkbox itself or anywhere on its parent <li>.
                let input: web_sys::HtmlInputElement =
                    if let Ok(inp) = target.clone().dyn_into::<web_sys::HtmlInputElement>() {
                        if inp.type_() == "checkbox" {
                            inp
                        } else {
                            return;
                        }
                    } else if let Ok(el) = target.dyn_into::<web_sys::HtmlElement>() {
                        match el
                            .closest("li")
                            .ok()
                            .flatten()
                            .and_then(|li| {
                                li.query_selector("input[type=\"checkbox\"]").ok().flatten()
                            })
                            .and_then(|cb| cb.dyn_into::<web_sys::HtmlInputElement>().ok())
                        {
                            Some(inp) => inp,
                            None => return,
                        }
                    } else {
                        return;
                    };
                ev.prevent_default();

                // Find this checkbox's index among all checkboxes in the container
                let container = match content_node_ref.get() {
                    Some(el) => el,
                    None => return,
                };
                let node_list = match container
                    .query_selector_all("input[type=\"checkbox\"]")
                {
                    Ok(nl) => nl,
                    Err(_) => return,
                };
                let mut cb_index = None;
                let input_node: &web_sys::Node = input.as_ref();
                for i in 0..node_list.length() {
                    if let Some(node) = node_list.item(i)
                        && node == *input_node {
                            cb_index = Some(i as usize);
                            break;
                        }
                }
                let cb_index = match cb_index {
                    Some(i) => i,
                    None => return,
                };

                let current_card = match state
                    .cards
                    .get_untracked()
                    .into_iter()
                    .find(|c| c.id() == card_id)
                {
                    Some(c) => c,
                    None => return,
                };
                let new_content = match blazelist_client_lib::display::toggle_task_item(
                    current_card.content(),
                    cb_index,
                ) {
                    Some(c) => c,
                    None => return,
                };
                let updated = current_card.next(
                    new_content,
                    current_card.priority(),
                    current_card.tags().to_vec(),
                    current_card.blazed(),
                    Utc::now(),
                    current_card.due_date(),
                );
                schedule_push(updated, state);
            };
            let card_for_editor = card.clone();

            // Reactive closure for the linked cards section: reads
            // recursive_links.get() so only this section re-renders
            // when the setting is toggled — not the whole DynChild.
            // Captures are untracked snapshots — same staleness trade-off
            // as the rest of the detail panel (intentional for editor
            // isolation; refreshed when selected_card changes).
            let linked_cards_section = {
                let content_raw_links = content_raw.clone();
                let all_cards_links = all_cards_snapshot.clone();
                let blazed_ids_links = blazed_card_ids.clone();
                move || {
                    let recursive = state.recursive_links.get();

                    let forward_ids = blazelist_client_lib::display::extract_card_links(&content_raw_links, card_id);
                    let back_ids = blazelist_client_lib::display::extract_back_links(card_id, &all_cards_links);

                    let mut all_linked_ids = forward_ids.clone();
                    let forward_set: std::collections::HashSet<uuid::Uuid> = forward_ids.iter().copied().collect();
                    for id in &back_ids {
                        if !forward_set.contains(id) {
                            all_linked_ids.push(*id);
                        }
                    }

                    if recursive {
                        let expanded = state
                            .link_graph_cache
                            .get_untracked()
                            .get(&card_id)
                            .map(|(_, v)| v.clone())
                            .unwrap_or_else(|| {
                                let result = blazelist_client_lib::display::expand_linked_cards(
                                    card_id,
                                    &all_cards_links,
                                );
                                let hash = *blake3::hash(content_raw_links.as_bytes()).as_bytes();
                                state.link_graph_cache.update(|cache| {
                                    cache.insert(card_id, (hash, result.clone()));
                                });
                                result
                            });
                        let direct_set: std::collections::HashSet<uuid::Uuid> =
                            all_linked_ids.iter().copied().collect();
                        all_linked_ids
                            .extend(expanded.into_iter().filter(|id| !direct_set.contains(id)));
                    }

                    let back_set: std::collections::HashSet<uuid::Uuid> = back_ids.iter().copied().collect();

                    // Sort so the linked-cards list mirrors the indicator
                    // order: mutual → forward-only → back-only → transitive.
                    // Within each group, preserve the original ordering.
                    let direction_rank = |id: &uuid::Uuid| -> u8 {
                        let is_fwd = forward_set.contains(id);
                        let is_bck = back_set.contains(id);
                        match (is_fwd, is_bck) {
                            (true, true) => 0,
                            (true, false) => 1,
                            (false, true) => 2,
                            (false, false) => 3,
                        }
                    };
                    let mut sorted_ids = all_linked_ids.clone();
                    sorted_ids.sort_by_key(direction_rank);

                    let linked_cards_with_preview = blazelist_client_lib::display::resolve_linked_cards(&sorted_ids, &all_cards_links, 500);

                    let forward_only_count = forward_ids.iter().filter(|id| !back_set.contains(id)).count();
                    let back_only_count = back_ids.iter().filter(|id| !forward_set.contains(id)).count();
                    let mutual_count = forward_ids.iter().filter(|id| back_set.contains(id)).count();
                    let transitive_count = all_linked_ids.len() - forward_ids.len() - back_only_count;

                    (!linked_cards_with_preview.is_empty()).then(|| {
                        let links = linked_cards_with_preview.clone();
                        let all_linked_ids_for_filter = all_linked_ids.clone();
                        let forward_ids_for_filter = forward_ids.clone();
                        let back_ids_for_filter = back_ids.clone();
                        let back_set_clone = back_set.clone();
                        let forward_set_clone = forward_set.clone();
                        let filter_dropdown_open = RwSignal::new(false);
                        let summary = link_indicators_view(LinkCounts {
                            forward: forward_only_count,
                            back: back_only_count,
                            mutual: mutual_count,
                            transitive: transitive_count,
                        });
                        view! {
                            <div class="detail-section">
                                <div class="detail-linked-cards">
                                    <div class="linked-cards-header">
                                        <span class="meta-label">"Linked Cards"</span>
                                        {summary}
                                    </div>
                                    <ul class="linked-card-list">
                                        {links.into_iter().map(|(lid, preview)| {
                                            let short_id = format!("{}\u{2026}", &lid.to_string()[..8]);
                                            let is_forward = forward_set_clone.contains(&lid);
                                            let is_back = back_set_clone.contains(&lid);
                                            let is_lid_blazed = blazed_ids_links.contains(&lid);
                                            let (direction, dir_class, dir_tip) = match (is_forward, is_back) {
                                                (true, true) => ("\u{2194}", "linked-card-direction dir-mutual", "Mutual link"),
                                                (true, false) => ("\u{2192}", "linked-card-direction dir-forward", "Forward link"),
                                                (false, true) => ("\u{2190}", "linked-card-direction dir-back", "Back link"),
                                                _ => ("\u{22EF}", "linked-card-direction dir-transitive", "Transitive link"),
                                            };
                                            let item_class = if is_lid_blazed { "linked-card-item blazed" } else { "linked-card-item" };
                                            let full_id = lid.to_string();
                                            view! {
                                                <li class=item_class on:click=move |_| {
                                                    if !confirm_discard_changes(&state) {
                                                        return;
                                                    }
                                                    flush_pending(&state);
                                                    state.selected_card.set(Some(lid));
                                                    state.editing.set(false);
                                                    sync_query_params(&state);
                                                } title=full_id>
                                                    <span class=dir_class title=dir_tip>{direction}</span>
                                                    <span class="linked-card-id">{short_id}</span>
                                                    <span class="linked-card-preview">{preview}</span>
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                    <div class="filter-linked-group">
                                        <button class="btn-filter-linked" on:click=move |_| {
                                            let mut ids = all_linked_ids_for_filter.clone();
                                            ids.insert(0, card_id);
                                            state.linked_card_filter.set(ids);
                                            state.filter.set(CardFilter::All);
                                            state.search_query.set(String::new());
                                            state.tag_filter.set(Vec::new());
                                            state.no_tags_filter.set(false);
                                            sync_query_params(&state);
                                        } title="Filter by all linked cards">"Filter Linked"</button>
                                        <button class="btn-filter-linked-dropdown" on:click=move |_| {
                                            filter_dropdown_open.update(|v| *v = !*v);
                                        }>{move || if filter_dropdown_open.get() { "\u{25B4}" } else { "\u{25BE}" }}</button>
                                        {move || filter_dropdown_open.get().then(|| {
                                            let fwd = forward_ids_for_filter.clone();
                                            let bck = back_ids_for_filter.clone();
                                            let direct_ids = {
                                                let mut d = fwd.clone();
                                                for id in &bck {
                                                    if !d.contains(id) {
                                                        d.push(*id);
                                                    }
                                                }
                                                d
                                            };
                                            view! {
                                                <div class="filter-linked-dropdown">
                                                    <button class="filter-linked-option" on:click=move |_| {
                                                        let mut ids = fwd.clone();
                                                        ids.insert(0, card_id);
                                                        state.linked_card_filter.set(ids);
                                                        state.filter.set(CardFilter::All);
                                                        state.search_query.set(String::new());
                                                        state.tag_filter.set(Vec::new());
                                                        state.no_tags_filter.set(false);
                                                        filter_dropdown_open.set(false);
                                                        sync_query_params(&state);
                                                    }>"\u{2192} Forward links only"</button>
                                                    <button class="filter-linked-option" on:click=move |_| {
                                                        let mut ids = bck.clone();
                                                        ids.insert(0, card_id);
                                                        state.linked_card_filter.set(ids);
                                                        state.filter.set(CardFilter::All);
                                                        state.search_query.set(String::new());
                                                        state.tag_filter.set(Vec::new());
                                                        state.no_tags_filter.set(false);
                                                        filter_dropdown_open.set(false);
                                                        sync_query_params(&state);
                                                    }>"\u{2190} Back links only"</button>
                                                    <button class="filter-linked-option" on:click=move |_| {
                                                        let mut ids = direct_ids.clone();
                                                        ids.insert(0, card_id);
                                                        state.linked_card_filter.set(ids);
                                                        state.filter.set(CardFilter::All);
                                                        state.search_query.set(String::new());
                                                        state.tag_filter.set(Vec::new());
                                                        state.no_tags_filter.set(false);
                                                        filter_dropdown_open.set(false);
                                                        sync_query_params(&state);
                                                    }>"\u{2194} Direct (forward + back)"</button>
                                                </div>
                                            }
                                        })}
                                    </div>
                                </div>
                            </div>
                        }
                    })
                }
            };

            let result = if state.editing.get() {
                view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <div class="detail-header-left">
                                <span class="detail-status editing">"Editing"</span>
                                {move || state.has_unsaved_changes.get().then(|| view! {
                                    <span class="unsaved-indicator">"(unsaved)"</span>
                                })}
                            </div>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        <CardEditor
                            editing_card=card_for_editor
                            on_save=move || state.editing.set(false)
                            on_cancel=Callback::new(move |_: ()| {
                                if !confirm_discard_changes(&state) { return; }
                                state.editing.set(false);
                            })
                        />
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <span class=move || if is_blazed.get() { "detail-status blazed" } else { "detail-status active" }>{move || if is_blazed.get() { "Blazed" } else { "Active" }}</span>
                            <div class="detail-header-nav">
                                <button
                                    class="detail-nav-btn"
                                    title="Previous card (k)"
                                    on:click=move |_| {
                                        let filtered = state.filtered_cards().get_untracked();
                                        let pos = filtered.iter().position(|c| c.id() == card_id);
                                        if let Some(i) = pos && i > 0 {
                                            select_card_view(&state, filtered[i - 1].id());
                                        }
                                    }
                                    disabled=move || {
                                        let filtered = state.filtered_cards().get_untracked();
                                        filtered.first().map(|c| c.id()) == Some(card_id)
                                    }
                                >"\u{2039}"</button>
                                <button
                                    class="detail-nav-btn"
                                    title="Next card (j)"
                                    on:click=move |_| {
                                        let filtered = state.filtered_cards().get_untracked();
                                        let pos = filtered.iter().position(|c| c.id() == card_id);
                                        if let Some(i) = pos && i + 1 < filtered.len() {
                                            select_card_view(&state, filtered[i + 1].id());
                                        }
                                    }
                                    disabled=move || {
                                        let filtered = state.filtered_cards().get_untracked();
                                        filtered.last().map(|c| c.id()) == Some(card_id)
                                    }
                                >"\u{203A}"</button>
                            </div>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        <div class="card-content" node_ref=content_node_ref inner_html=content_html on:click=on_content_click></div>
                        {task_progress.map(|(done, total)| view! {
                            <div class="detail-task-progress">
                                <span class="meta-label">"Tasks"</span>
                                <span class="meta-value">{format!("{done}/{total}")}</span>
                            </div>
                        })}

                        // ── Tags (inline, only when present) ──
                        {(!card_tags_with_ids.is_empty()).then(|| {
                            let tags = card_tags_with_ids.clone();
                            view! {
                                <div class="detail-tag-chips">
                                    {tags.into_iter().map(|(tag_id, name, color)| {
                                        let on_tag_click = move |_| {
                                            let graph = TagGraph::from_tags(&state.tags.get_untracked());
                                            let to_add = graph.closure_of(&[tag_id]);
                                            state.tag_filter.update(|tags| {
                                                for id in to_add {
                                                    if !tags.contains(&id) {
                                                        tags.push(id);
                                                    }
                                                }
                                            });
                                            sync_query_params(&state);
                                        };
                                        let style = tag_chip_style(&color);
                                        view! {
                                            <span class="tag-chip" style=style on:click=on_tag_click title="Click to filter by this tag">{name}</span>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }
                        })}

                        // ── Controls section (action buttons + due date) ──
                        <div class="detail-section">
                            <div class="card-actions">
                                <div class="action-row nav-row">
                                    <button class="btn-move" on:click=on_move_top prop:disabled={is_at_top || !in_filtered} title="Move to top">{"\u{2912}"}</button>
                                    <button class="btn-move" on:click=on_move_up prop:disabled={is_at_top || !in_filtered} title="Move up one">{"\u{2191}"}</button>
                                    <button class="btn-move" on:click=on_move_down prop:disabled={is_at_bottom || !in_filtered} title="Move down one">{"\u{2193}"}</button>
                                    <button class="btn-move" on:click=on_move_bottom prop:disabled={is_at_bottom || !in_filtered} title="Move to bottom">{"\u{2913}"}</button>
                                    <input
                                        class="move-to-input"
                                        type="number"
                                        min="1"
                                        max=total_cards.to_string()
                                        prop:value=move || move_to_input.get()
                                        prop:disabled={!in_filtered}
                                        on:input=move |ev| move_to_input.set(event_target_value(&ev))
                                    />
                                    <span class="move-to-total">{format!("/ {total_cards}")}</span>
                                    <button class="btn-go" on:click=on_move_to prop:disabled={!in_filtered}>"Move"</button>
                                </div>
                                <div class="action-row cmd-row">
                                    <button class="btn-edit" on:click=on_edit>"Edit"</button>
                                    <button class=move || if is_blazed.get() { "btn-extinguish" } else { "btn-blaze" } on:click=on_blaze>{move || if is_blazed.get() { "Extinguish" } else { "Blaze" }}</button>
                                    {move || {
                                        if confirm_delete.get() > 0 {
                                            let card_label = move || {
                                                let preview = state.cards.get_untracked().into_iter()
                                                    .find(|c| c.id() == card_id)
                                                    .and_then(|c| blazelist_client_lib::display::card_preview(c.content(), 120))
                                                    .unwrap_or_else(|| "(empty)".to_string());
                                                format!("Card: {preview}")
                                            };
                                            view! {
                                                <ConfirmDeletePrompt
                                                    step=confirm_delete
                                                    first_prompt=|| "Delete?".to_string()
                                                    entity_label=card_label
                                                    on_confirm=do_confirm_delete
                                                    on_cancel=do_cancel_delete
                                                />
                                            }.into_any()
                                        } else {
                                            view! {
                                                <button class="btn-delete" on:click=on_delete_click>"Delete"</button>
                                            }.into_any()
                                        }
                                    }}
                                </div>
                            </div>
                            <div class="detail-tags">
                                <span class="meta-label">"Due date"</span>
                            <div class="due-date-controls">
                                {move || match due_date_opt.get() {
                                    Some(d) => {
                                        let (_badge_text, badge_class) = format_due_date_badge(&d);
                                        let cls = format!("due-date-current {badge_class}");
                                        let display = format_due_date_display(&d);
                                        view! { <span class=cls>{display}</span> }.into_any()
                                    }
                                    None => view! {
                                        <span class="due-date-current due-not-set">"Not set"</span>
                                    }.into_any(),
                                }}
                                <div class="due-date-dropdown-group" node_ref=due_group_ref>
                                    <button class="due-date-quick-btn" on:click={
                                        let set_due_date = set_due_date;
                                        move |_| {
                                            let smart = match due_date_opt.get_untracked() {
                                                Some(d) if matches!(due_date_status(&d), DueDateStatus::Today) => DueDatePreset::Tomorrow,
                                                _ => DueDatePreset::Today,
                                            };
                                            due_preset.set(smart);
                                            set_due_date(Some(smart.resolve()));
                                        }
                                    }>{move || {
                                        match due_date_opt.get() {
                                            Some(d) if matches!(due_date_status(&d), DueDateStatus::Today) => DueDatePreset::Tomorrow.label(),
                                            _ => DueDatePreset::Today.label(),
                                        }
                                    }}</button>
                                    <button class="due-date-dropdown-toggle" on:click=move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        due_dropdown_open.update(|v| *v = !*v);
                                    }>
                                        {move || if due_dropdown_open.get() { "\u{25B4}" } else { "\u{25BE}" }}
                                    </button>
                                    {move || due_dropdown_open.get().then(|| {
                                        let set_due_date = set_due_date;
                                        view! {
                                            <div class="due-date-dropdown-menu">
                                                {DueDatePreset::ALL.into_iter().map(|p| {
                                                    let set_due_date = set_due_date;
                                                    view! {
                                                        <button
                                                            class="save-dropdown-item"
                                                            class:active=move || due_preset.get() == p
                                                            on:click=move |_| {
                                                                due_preset.set(p);
                                                                set_due_date(Some(p.resolve()));
                                                                due_dropdown_open.set(false);
                                                            }
                                                        >
                                                            {p.label()}
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }
                                    })}
                                </div>
                                <input
                                    class="due-date-picker"
                                    type="date"
                                    prop:value=move || due_date_opt.get().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
                                    on:change={
                                        let set_due_date = set_due_date;
                                        move |ev| {
                                            let val = event_target_value(&ev);
                                            if let Ok(date) = chrono::NaiveDate::parse_from_str(&val, "%Y-%m-%d") {
                                                set_due_date(Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc()));
                                            }
                                        }
                                    }
                                />
                                {
                                    let set_due_date = set_due_date;
                                    move || due_date_opt.get().map(|_| {
                                        let set_due_date = set_due_date;
                                        view! {
                                            <button class="due-date-clear-btn" on:click=move |_| set_due_date(None)>"Clear"</button>
                                        }
                                    })
                                }
                            </div>
                        </div>
                        </div>

                        // ── Details section (metadata) ──
                        <div class="detail-section">
                            <div class="detail-meta">
                                <div class="meta-row">
                                    <span class="meta-label">"ID"</span>
                                    <span class="meta-value meta-id-value">
                                        <button class="meta-copy-btn" title="Copy to clipboard" on:click=move |_| {
                                            copy_to_clipboard(&id_str_copy);
                                            show_copy_toast(state, &copy_toast_msg);
                                        }>{"\u{29C9}"}</button>
                                        {id_str}
                                    </span>
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">"Priority"</span>
                                    <span class="meta-value">{format!("{priority_raw} ({priority_pct:.2}%)")}</span>
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">"Version"</span>
                                    <span class="meta-value">{count.to_string()}</span>
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">"Created"</span>
                                    <Timestamp datetime=created class="meta-value" />
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">"Modified"</span>
                                    <Timestamp datetime=modified class="meta-value" />
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">"Due Date"</span>
                                    {move || match due_date_opt.get() {
                                        Some(d) => view! {
                                            <Timestamp datetime=d class="meta-value" />
                                        }.into_any(),
                                        None => view! {
                                            <span class="meta-value due-not-set">"Not set"</span>
                                        }.into_any(),
                                    }}
                                </div>
                            </div>
                        </div>

                        // ── Linked Cards section ──
                        {linked_cards_section}

                        // ── History section ──
                        <div class="detail-section">
                            <VersionHistory card_id=card_id />
                        </div>
                    </div>
                }.into_any()
            };
            Some(result)
        }}}
    }
}

/// Inline component rendered inside `CardDetail` when `creating_new_tag` is true.
#[component]
fn NewTagForm(on_close: impl Fn(()) + Copy + 'static) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let title_input = RwSignal::new(String::new());
    let color_input = RwSignal::new(String::from("#808080"));
    let use_color = RwSignal::new(false);

    // Track dirty state — compare against initial empty form.
    Effect::new(move |_| {
        let dirty = !title_input.get().trim().is_empty() || use_color.get();
        state.has_unsaved_changes.set(dirty);
    });
    on_cleanup(move || {
        state.has_unsaved_changes.set(false);
    });

    let create_action = move || {
        let title = title_input.get_untracked();
        if title.trim().is_empty() {
            return;
        }

        let color = if use_color.get_untracked() {
            parse_hex_color(&color_input.get_untracked())
        } else {
            None
        };

        let state = state;
        leptos::task::spawn_local(async move {
            // Tag push isn't queued offline (the offline queue only
            // holds Cards), so surface a toast instead of silently
            // swallowing the click.
            let Some(client) = get_client() else {
                show_error_toast(state, "Can't create tags while offline", 3000);
                return;
            };
            let tag = Tag::first(Uuid::new_v4(), title, color, Utc::now());
            match client.push_tag(tag.clone()).await {
                Ok(_) => {}
                Err(ClientError::ConnectionLost) => {
                    show_error_toast(state, "Can't create tags while offline", 3000);
                    return;
                }
                Err(e) => {
                    tracing::error!(%e, "Failed to create tag");
                    show_error_toast(state, &format!("Failed to create tag: {e}"), 3000);
                    return;
                }
            }
            let tag_id = tag.id();
            state.tags.update(|tags| tags.push(tag));
            state.creating_new_tag.set(false);
            state.selected_card.set(Some(tag_id));
            sync_query_params(&state);
        });
    };

    let cancel_action = move || {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.creating_new_tag.set(false);
        sync_query_params(&state);
    };

    view! {
        <div class="detail-header">
            <div class="detail-header-left">
                <span class="detail-status tag-not-card">"New Tag"</span>
                {move || state.has_unsaved_changes.get().then(|| view! {
                    <span class="unsaved-indicator">"(unsaved)"</span>
                })}
            </div>
            <button class="detail-close" on:click=move |_| on_close(())>"x"</button>
        </div>

        <div class="tag-title-section">
            <form class="tag-rename-form" on:submit=move |ev| {
                ev.prevent_default();
                create_action();
            }>
                <input
                    class="tag-rename-input"
                    type="text"
                    placeholder="Tag title..."
                    prop:value=move || title_input.get()
                    on:input=move |ev| title_input.set(event_target_value(&ev))
                />
            </form>
        </div>

        <TagColorPicker color_input=color_input use_color=use_color />

        <div class="card-actions">
            <div class="action-row cmd-row">
                <button class="btn-save" on:click=move |_| create_action()>"Create"</button>
                <button class="btn-cancel" on:click=move |_| cancel_action()>"Cancel"</button>
            </div>
        </div>
    }
}

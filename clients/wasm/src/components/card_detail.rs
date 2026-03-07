use crate::components::card_editor::CardEditor;
use crate::components::hooks::use_click_outside_close;
use crate::components::tag_detail::TagDetail;
use crate::components::version_history::VersionHistory;
use crate::state::store::{
    AppState, DueDatePreset, confirm_discard_changes, format_due_date_badge,
    format_due_date_display, get_client, sync_query_params, tag_chip_style,
};
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, move_card,
};
use blazelist_protocol::{Card, CardFilter, Entity, PushItem, Utc};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setTimeout")]
    fn set_timeout_js(handler: &js_sys::Function, timeout: i32) -> i32;
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout_js(handle: i32);
}

fn render_markdown(content: &str, card_ids: &std::collections::HashSet<uuid::Uuid>) -> String {
    let html =
        comrak::markdown_to_html(content, &blazelist_client_lib::display::markdown_options());
    // comrak renders checkboxes with disabled="" — remove it so clicks fire
    let html = html.replace(" disabled=\"\"", "");
    blazelist_client_lib::display::linkify_card_uuids(&html, card_ids)
}

/// Flush pending debounced versions (fire-and-forget).
fn flush_pending_sig(pending: RwSignal<Vec<Card>>, handle: RwSignal<i32>) {
    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
        handle.set(0);
    }
    let versions = pending.get_untracked();
    pending.set(Vec::new());
    if !versions.is_empty() {
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                if let Err(e) = client.push_card_versions(versions).await {
                    log::error!("Failed to push pending card versions: {e}");
                }
            }
        });
    }
}

/// Drain pending debounced versions and return them.
fn drain_pending_sig(pending: RwSignal<Vec<Card>>, handle: RwSignal<i32>) -> Vec<Card> {
    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
        handle.set(0);
    }
    let versions = pending.get_untracked();
    pending.set(Vec::new());
    versions
}

/// Schedule a debounced push of a card version.
fn schedule_push_sig(
    updated: Card,
    state: AppState,
    pending: RwSignal<Vec<Card>>,
    pending_card_id: RwSignal<Option<uuid::Uuid>>,
    handle: RwSignal<i32>,
) {
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
                if let Some(client) = get_client() {
                    if let Err(e) = client.push_card_versions(old_versions).await {
                        log::error!("Failed to push pending card versions: {e}");
                    }
                }
            });
        }
    }
    pending_card_id.set(Some(card_id));
    pending.update(|v| v.push(updated));

    let old = handle.get_untracked();
    if old != 0 {
        clear_timeout_js(old);
    }

    let cb = Closure::once(move || {
        let versions = pending.get_untracked();
        pending.set(Vec::new());
        handle.set(0);
        pending_card_id.set(None);
        if !versions.is_empty() {
            leptos::task::spawn_local(async move {
                if let Some(client) = get_client() {
                    if let Err(e) = client.push_card_versions(versions).await {
                        log::error!("Failed to push card versions: {e}");
                    }
                }
            });
        }
    });
    let func = cb.into_js_value();
    let new_handle = set_timeout_js(func.unchecked_ref(), 300);
    handle.set(new_handle);
}

/// Apply a move placement result: update the moved card locally and push
/// shifted cards via batch if rebalancing occurred.
fn apply_move_placement(
    placement: Placement,
    current: &Card,
    all_cards: &[Card],
    state: AppState,
    pending_versions: RwSignal<Vec<Card>>,
    pending_card_id: RwSignal<Option<uuid::Uuid>>,
    timeout_handle: RwSignal<i32>,
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
            schedule_push_sig(
                updated,
                state,
                pending_versions,
                pending_card_id,
                timeout_handle,
            );
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

            // Push batch: shifted cards + moved card
            leptos::task::spawn_local(async move {
                if let Some(client) = get_client() {
                    let mut items: Vec<PushItem> = shifted_cards
                        .into_iter()
                        .map(|c| PushItem::Cards(vec![c]))
                        .collect();
                    items.push(PushItem::Cards(vec![updated]));
                    if let Err(e) = client.push_batch(items).await {
                        log::error!("Failed to push rebalanced move: {e}");
                    }
                }
            });
        }
    }
}

#[component]
pub fn CardDetail() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let confirm_delete = RwSignal::new(false);
    let move_to_input = RwSignal::new(String::new());
    let due_preset = RwSignal::new(DueDatePreset::Today);
    let due_dropdown_open = RwSignal::new(false);

    let due_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(due_dropdown_open, due_group_ref);

    // Debounce state (all signals — Copy + Send + Sync)
    let pending_versions: RwSignal<Vec<Card>> = RwSignal::new(Vec::new());
    let pending_card_id: RwSignal<Option<uuid::Uuid>> = RwSignal::new(None);
    let timeout_handle: RwSignal<i32> = RwSignal::new(0);

    on_cleanup(move || {
        let old = timeout_handle.get_untracked();
        if old != 0 {
            clear_timeout_js(old);
        }
        pending_versions.set(Vec::new());
    });

    let on_close = move |_| {
        if !confirm_discard_changes(&state) {
            return;
        }
        flush_pending_sig(pending_versions, timeout_handle);
        state.selected_card.set(None);
        state.creating_new.set(false);
        state.editing.set(false);
        confirm_delete.set(false);
        sync_query_params(&state);
    };

    view! {
        {move || {
            // --- Creating new card ---
            if state.creating_new.get() {
                return Some(view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <div class="detail-header-left">
                                <span class="detail-status active">"New Card"</span>
                                {move || state.has_unsaved_changes.get().then(|| view! {
                                    <span class="unsaved-indicator">"(unsaved)"</span>
                                })}
                            </div>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        <CardEditor
                            on_save=move || state.creating_new.set(false)
                            on_cancel=Callback::new(move |_: ()| state.creating_new.set(false))
                        />
                    </div>
                }.into_any());
            }

            // --- Selected card ---
            let selected_id = state.selected_card.get();
            if selected_id.is_none() {
                return None;
            }
            let selected_id = selected_id.unwrap();
            let card = state.cards.get().into_iter().find(|c| c.id() == selected_id);

            if card.is_none() {
                let is_tag = state.tags.get().iter().any(|t| t.id() == selected_id);
                let id_str = selected_id.to_string();
                if is_tag {
                    return Some(view! {
                        <div class="card-detail">
                            <TagDetail />
                        </div>
                    }.into_any());
                } else {
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
            }

            let card = card.unwrap();
            {
            let card_id = card.id();
            let is_blazed = card.blazed();
            let content_raw = card.content().to_string();
            let all_cards_snapshot = state.cards.get_untracked();

            // Build card ID set for linkifying UUIDs in the markdown view.
            let known_card_ids: std::collections::HashSet<uuid::Uuid> =
                all_cards_snapshot.iter().map(|c| c.id()).collect();
            let content_html = render_markdown(&content_raw, &known_card_ids);

            let task_progress = blazelist_client_lib::display::task_progress(&content_raw);
            let content_node_ref = NodeRef::<leptos::html::Div>::new();
            let priority_raw = card.priority();
            let priority_pct = priority_raw.percentage();
            let count = i64::from(card.count());
            let created = card.created_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
            let modified = card.modified_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
            let id_str = card_id.to_string();
            let id_str_copy = id_str.clone();

            let card_tag_ids = card.tags().to_vec();
            let all_tags = state.tags.get_untracked();
            let mut card_tags_with_ids: Vec<(uuid::Uuid, String, Option<rgb::RGB8>)> = card_tag_ids.iter().filter_map(|tid| {
                all_tags.iter().find(|t| t.id() == *tid).map(|t| {
                    (*tid, t.title().to_string(), t.color())
                })
            }).collect();
            card_tags_with_ids.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

            // Extract forward links (this card mentions other cards) and
            // back links (other cards mention this card) for bidirectional display.
            let forward_ids = blazelist_client_lib::display::extract_card_links(&content_raw, card_id);
            let back_ids = blazelist_client_lib::display::extract_back_links(card_id, &all_cards_snapshot);

            // Merge forward + back links, deduplicating (forward takes precedence).
            let mut all_linked_ids = forward_ids.clone();
            let forward_set: std::collections::HashSet<uuid::Uuid> = forward_ids.iter().copied().collect();
            for id in &back_ids {
                if !forward_set.contains(id) {
                    all_linked_ids.push(*id);
                }
            }
            let back_set: std::collections::HashSet<uuid::Uuid> = back_ids.iter().copied().collect();
            let linked_cards_with_preview = blazelist_client_lib::display::resolve_linked_cards(&all_linked_ids, &all_cards_snapshot, 500);

            // Compute summary counts for the linked cards header.
            let forward_only_count = forward_ids.iter().filter(|id| !back_set.contains(id)).count();
            let back_only_count = back_ids.iter().filter(|id| !forward_set.contains(id)).count();
            let mutual_count = forward_ids.iter().filter(|id| back_set.contains(id)).count();

            let reorder_disabled = !state.sort_order.get().is_default()
                || !state.search_query.get().is_empty();

            let filtered = state.filtered_cards().get();
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

            let due_date_opt = card.due_date();

            let blaze_text = if is_blazed { "Extinguish" } else { "Blaze" };
            let blaze_class = if is_blazed { "btn-extinguish" } else { "btn-blaze" };
            let status_text = if is_blazed { "Blazed" } else { "Active" };
            let status_class = if is_blazed { "detail-status blazed" } else { "detail-status active" };

            // Helper to set due date on a card (creates new version and pushes)
            let set_due_date = move |new_due: Option<chrono::DateTime<Utc>>| {
                let state = state.clone();
                leptos::task::spawn_local(async move {
                    if let Some(client) = get_client() {
                        let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                        if let Some(current) = current {
                            let next = current.next(
                                current.content().to_string(),
                                current.priority(),
                                current.tags().to_vec(),
                                current.blazed(),
                                Utc::now(),
                                new_due,
                            );
                            if let Err(e) = client.push_card(next.clone()).await {
                                log::error!("Failed to set due date: {e}");
                                return;
                            }
                            state.upsert_card(next);
                        }
                    }
                });
            };

            let on_blaze = move |_| {
                let pending = drain_pending_sig(pending_versions, timeout_handle);
                let state = state.clone();
                leptos::task::spawn_local(async move {
                    if let Some(client) = get_client() {
                        if !pending.is_empty() {
                            if let Err(e) = client.push_card_versions(pending).await {
                                log::error!("Failed to push pending versions: {e}");
                                return;
                            }
                        }
                        let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                        if let Some(current) = current {
                            let next = current.next(
                                current.content().to_string(),
                                current.priority(),
                                current.tags().to_vec(),
                                !current.blazed(),
                                Utc::now(),
                                current.due_date(),
                            );
                            if let Err(e) = client.push_card(next.clone()).await {
                                log::error!("Failed to toggle blaze: {e}");
                                return;
                            }
                            state.upsert_card(next);
                        }
                    }
                });
            };

            let on_delete_click = move |_| {
                confirm_delete.set(true);
            };

            let on_confirm_delete = move |_| {
                let pending = drain_pending_sig(pending_versions, timeout_handle);
                let state = state.clone();
                confirm_delete.set(false);
                leptos::task::spawn_local(async move {
                    if let Some(client) = get_client() {
                        if !pending.is_empty() {
                            if let Err(e) = client.push_card_versions(pending).await {
                                log::error!("Failed to push pending versions: {e}");
                                return;
                            }
                        }
                        if let Err(e) = client.delete_card(card_id).await {
                            log::error!("Failed to delete card: {e}");
                            return;
                        }
                        state.cards.update(|cards| cards.retain(|c| c.id() != card_id));
                        state.selected_card.set(None);
                        sync_query_params(&state);
                    }
                });
            };

            let on_cancel_delete = move |_| {
                confirm_delete.set(false);
            };

            let filtered_cards_memo = state.filtered_cards();

            let on_move_top = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let placement = move_card(&filtered, card_id, InsertPosition::Top);
                    apply_move_placement(placement, &current, &filtered, state, pending_versions, pending_card_id, timeout_handle);
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
                    apply_move_placement(placement, &current, &filtered, state, pending_versions, pending_card_id, timeout_handle);
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
                    apply_move_placement(placement, &current, &filtered, state, pending_versions, pending_card_id, timeout_handle);
                }
            };

            let on_move_bottom = move |_| {
                let current = state.cards.get_untracked().into_iter().find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let filtered = filtered_cards_memo.get_untracked();
                    let placement = move_card(&filtered, card_id, InsertPosition::Bottom);
                    apply_move_placement(placement, &current, &filtered, state, pending_versions, pending_card_id, timeout_handle);
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
                    apply_move_placement(placement, &current, &filtered, state, pending_versions, pending_card_id, timeout_handle);
                }
            };

            let on_edit = move |_| {
                flush_pending_sig(pending_versions, timeout_handle);
                state.editing.set(true);
            };

            let on_content_click = move |ev: web_sys::MouseEvent| {
                let target = match ev.target() {
                    Some(t) => t,
                    None => return,
                };

                // Check for card UUID link click.
                if let Ok(el) = target.clone().dyn_into::<web_sys::HtmlElement>() {
                    if el.class_list().contains("card-uuid-link") {
                        if let Some(card_id_str) = el.get_attribute("data-card-id") {
                            if let Ok(target_id) = card_id_str.parse::<uuid::Uuid>() {
                                if !confirm_discard_changes(&state) {
                                    return;
                                }
                                flush_pending_sig(pending_versions, timeout_handle);
                                state.selected_card.set(Some(target_id));
                                state.editing.set(false);
                                sync_query_params(&state);
                                return;
                            }
                        }
                    }
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
                    if let Some(node) = node_list.item(i) {
                        if node == *input_node {
                            cb_index = Some(i as usize);
                            break;
                        }
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
                schedule_push_sig(
                    updated,
                    state,
                    pending_versions,
                    pending_card_id,
                    timeout_handle,
                );
            };
            let card_for_editor = card.clone();

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
                            on_cancel=Callback::new(move |_: ()| state.editing.set(false))
                        />
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="card-detail">
                        <div class="detail-header">
                            <span class=status_class>{status_text}</span>
                            <button class="detail-close" on:click=on_close>"x"</button>
                        </div>
                        <div class="card-content" node_ref=content_node_ref inner_html=content_html on:click=on_content_click></div>
                        {task_progress.map(|(done, total)| view! {
                            <div class="detail-task-progress">
                                <span class="meta-label">"Tasks"</span>
                                <span class="meta-value">{format!("{done}/{total}")}</span>
                            </div>
                        })}
                        {(!card_tags_with_ids.is_empty()).then(|| {
                            let tags = card_tags_with_ids.clone();
                            view! {
                                <div class="detail-tags">
                                    <span class="meta-label">"Tags"</span>
                                    <div class="detail-tag-chips">
                                        {tags.into_iter().map(|(tag_id, name, color)| {
                                            let on_tag_click = move |_| {
                                                state.tag_filter.update(|tags| {
                                                    if !tags.contains(&tag_id) {
                                                        tags.push(tag_id);
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
                                </div>
                            }
                        })}
                        <div class="detail-tags">
                            <span class="meta-label">"Due date"</span>
                            <div class="due-date-controls">
                                {due_date_opt.map(|d| {
                                    let (_badge_text, badge_class) = format_due_date_badge(&d);
                                    let cls = format!("due-date-current {badge_class}");
                                    let display = format_due_date_display(&d);
                                    view! { <span class=cls>{display}</span> }
                                })}
                                {(!due_date_opt.is_some()).then(|| view! {
                                    <span class="due-date-current due-not-set">"Not set"</span>
                                })}
                                <div class="due-date-dropdown-group" node_ref=due_group_ref>
                                    <button class="due-date-quick-btn" on:click={
                                        let set_due_date = set_due_date.clone();
                                        move |_| set_due_date(Some(due_preset.get_untracked().resolve()))
                                    }>{move || due_preset.get().label()}</button>
                                    <button class="due-date-dropdown-toggle" on:click=move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        due_dropdown_open.update(|v| *v = !*v);
                                    }>
                                        {move || if due_dropdown_open.get() { "\u{25B4}" } else { "\u{25BE}" }}
                                    </button>
                                    {move || due_dropdown_open.get().then(|| {
                                        let set_due_date = set_due_date.clone();
                                        view! {
                                            <div class="due-date-dropdown-menu">
                                                {DueDatePreset::ALL.into_iter().map(|p| {
                                                    let set_due_date = set_due_date.clone();
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
                                    prop:value=move || due_date_opt.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
                                    on:change={
                                        let set_due_date = set_due_date.clone();
                                        move |ev| {
                                            let val = event_target_value(&ev);
                                            if let Ok(date) = chrono::NaiveDate::parse_from_str(&val, "%Y-%m-%d") {
                                                set_due_date(Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc()));
                                            }
                                        }
                                    }
                                />
                                {due_date_opt.map(|_| {
                                    let set_due_date = set_due_date.clone();
                                    view! {
                                        <button class="due-date-clear-btn" on:click=move |_| set_due_date(None)>"Clear"</button>
                                    }
                                })}
                            </div>
                        </div>
                        {(!linked_cards_with_preview.is_empty()).then(|| {
                            let links = linked_cards_with_preview.clone();
                            let all_linked_ids_for_filter = all_linked_ids.clone();
                            let back_set_clone = back_set.clone();
                            let forward_set_clone = forward_set.clone();
                            // Build colored summary spans like →3 ←2 ↔1
                            let summary_fwd = (forward_only_count > 0).then(|| {
                                let t = format!("\u{2192}{forward_only_count}");
                                view! { <span class="summary-forward">{t}</span> }
                            });
                            let summary_bck = (back_only_count > 0).then(|| {
                                let t = format!("\u{2190}{back_only_count}");
                                view! { <span class="summary-back">{t}</span> }
                            });
                            let summary_mut = (mutual_count > 0).then(|| {
                                let t = format!("\u{2194}{mutual_count}");
                                view! { <span class="summary-mutual">{t}</span> }
                            });
                            view! {
                                <div class="detail-linked-cards">
                                    <div class="linked-cards-header">
                                        <span class="meta-label">"Linked Cards"</span>
                                        <span class="linked-cards-summary">{summary_fwd}{summary_bck}{summary_mut}</span>
                                    </div>
                                    <ul class="linked-card-list">
                                        {links.into_iter().map(|(lid, preview)| {
                                            let short_id = format!("{}\u{2026}", &lid.to_string()[..8]);
                                            let is_forward = forward_set_clone.contains(&lid);
                                            let is_back = back_set_clone.contains(&lid);
                                            let (direction, dir_class) = match (is_forward, is_back) {
                                                (true, true) => ("\u{2194}", "linked-card-direction dir-mutual"),
                                                (true, false) => ("\u{2192}", "linked-card-direction dir-forward"),
                                                (false, true) => ("\u{2190}", "linked-card-direction dir-back"),
                                                _ => ("", "linked-card-direction"),
                                            };
                                            let full_id = lid.to_string();
                                            view! {
                                                <li class="linked-card-item" on:click=move |_| {
                                                    if !confirm_discard_changes(&state) {
                                                        return;
                                                    }
                                                    flush_pending_sig(pending_versions, timeout_handle);
                                                    state.selected_card.set(Some(lid));
                                                    state.editing.set(false);
                                                    sync_query_params(&state);
                                                } title=full_id>
                                                    <span class=dir_class>{direction}</span>
                                                    <span class="linked-card-id">{short_id}</span>
                                                    <span class="linked-card-preview">{preview}</span>
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                    <button class="btn-filter-linked" on:click=move |_| {
                                        let mut ids = all_linked_ids_for_filter.clone();
                                        ids.insert(0, card_id);
                                        state.linked_card_filter.set(ids);
                                        state.filter.set(CardFilter::All);
                                        sync_query_params(&state);
                                    }>"Filter Linked"</button>
                                </div>
                            }
                        })}
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
                                <button class=blaze_class on:click=on_blaze>{blaze_text}</button>
                                {move || if confirm_delete.get() {
                                    view! {
                                        <span class="confirm-delete">
                                            <span class="confirm-text">"Delete?"</span>
                                            <button class="btn-confirm-yes" on:click=on_confirm_delete>"Yes"</button>
                                            <button class="btn-confirm-no" on:click=on_cancel_delete>"No"</button>
                                        </span>
                                    }.into_any()
                                } else {
                                    view! {
                                        <button class="btn-delete" on:click=on_delete_click>"Delete"</button>
                                    }.into_any()
                                }}
                            </div>
                        </div>
                        <div class="detail-meta">
                            <div class="meta-row">
                                <span class="meta-label">"ID"</span>
                                <span class="meta-value meta-id-value">
                                    <button class="meta-copy-btn" title="Copy to clipboard" on:click=move |_| {
                                        if let Some(w) = web_sys::window() {
                                            let clipboard = w.navigator().clipboard();
                                            let _ = clipboard.write_text(&id_str_copy);
                                        }
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
                                <span class="meta-value">{created}</span>
                            </div>
                            <div class="meta-row">
                                <span class="meta-label">"Modified"</span>
                                <span class="meta-value">{modified}</span>
                            </div>
                            <div class="meta-row">
                                <span class="meta-label">"Due Date"</span>
                                {match due_date_opt {
                                    Some(d) => view! {
                                        <span class="meta-value">{d.format("%Y-%m-%d %H:%M:%S UTC").to_string()}</span>
                                    }.into_any(),
                                    None => view! {
                                        <span class="meta-value due-not-set">"Not set"</span>
                                    }.into_any(),
                                }}
                            </div>
                        </div>
                        <VersionHistory card_id=card_id />
                    </div>
                }.into_any()
            };
            Some(result)
        }}}
    }
}

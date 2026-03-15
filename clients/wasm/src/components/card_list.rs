use crate::components::card_item::CardItem;
use crate::components::hooks::use_click_outside_close;
use crate::state::store::{
    AppState, NewCardPosition, confirm_discard_changes, get_client, sync_query_params,
};
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, move_card,
};
use blazelist_protocol::{Card, Entity, PushItem, Utc};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Shared drag-and-drop state, provided via Leptos context.
#[derive(Clone, Copy)]
pub struct DragState {
    /// UUID string of the card currently being dragged, or empty.
    pub dragged_card_id: RwSignal<String>,
    /// Index in the filtered list where the drop indicator is shown.
    /// `None` means no indicator visible.
    pub drop_target_index: RwSignal<Option<usize>>,
}

#[component]
pub fn CardList() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let filtered = state.filtered_cards();

    // Memoize link counts — only recomputed when the full card set changes,
    // not on every filter/search keystroke.
    let link_counts = Memo::new(move |_| {
        let all = state.cards.get();
        blazelist_client_lib::display::compute_all_link_counts(&all)
    });

    let new_card_dropdown = RwSignal::new(false);
    let new_card_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(new_card_dropdown, new_card_group_ref);

    let start_creating = move |position: NewCardPosition| {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.selected_card.set(None);
        state.editing.set(false);
        state.new_card_position.set(position);
        state.creating_new.set(true);
        state.settings_open.set(false);
        new_card_dropdown.set(false);
        sync_query_params(&state);
    };

    let on_new_card = move |_| {
        start_creating(NewCardPosition::Bottom);
    };

    // Drag-and-drop state
    let drag_state = DragState {
        dragged_card_id: RwSignal::new(String::new()),
        drop_target_index: RwSignal::new(None),
    };
    provide_context(drag_state);

    let on_dragover_cards = move |ev: web_sys::DragEvent| {
        if !state.drag_drop_reorder.get_untracked() {
            return;
        }
        // Allow dropping
        ev.prevent_default();
    };

    let on_drop_cards = move |ev: web_sys::DragEvent| {
        if !state.drag_drop_reorder.get_untracked() {
            return;
        }
        ev.prevent_default();

        let dragged_id_str = drag_state.dragged_card_id.get_untracked();
        let target_idx = drag_state.drop_target_index.get_untracked();

        // Clean up drag state
        drag_state.dragged_card_id.set(String::new());
        drag_state.drop_target_index.set(None);

        if dragged_id_str.is_empty() {
            return;
        }
        let target_idx = match target_idx {
            Some(i) => i,
            None => return,
        };

        let dragged_id: uuid::Uuid = match dragged_id_str.parse() {
            Ok(id) => id,
            Err(_) => return,
        };

        let cards = filtered.get_untracked();
        let current = match state
            .cards
            .get_untracked()
            .into_iter()
            .find(|c| c.id() == dragged_id)
        {
            Some(c) => c,
            None => return,
        };

        let cur_idx = match cards.iter().position(|c| c.id() == dragged_id) {
            Some(i) => i,
            None => return,
        };

        if target_idx == cur_idx || target_idx == cur_idx + 1 {
            // Dropped in the same position — no-op
            return;
        }

        // Determine the InsertPosition in the filtered list (after removing the dragged card).
        let insert_pos = if target_idx == 0 {
            InsertPosition::Top
        } else if target_idx >= cards.len() {
            InsertPosition::Bottom
        } else {
            // target_idx is the slot *before* the card at that index.
            // After removing the dragged card, indices shift.
            let adjusted = if target_idx > cur_idx {
                target_idx - 1
            } else {
                target_idx
            };
            if adjusted == 0 {
                InsertPosition::Top
            } else {
                InsertPosition::At(adjusted)
            }
        };

        let placement = move_card(&cards, dragged_id, insert_pos);
        apply_drag_placement(placement, &current, &cards, state);
    };

    let on_dragleave_cards = move |ev: web_sys::DragEvent| {
        // Only clear if leaving the .cards container itself
        let target = ev.current_target();
        let related = ev.related_target();
        if let (Some(container), related) = (target, related) {
            let container: web_sys::Element = container.unchecked_into();
            let inside = related
                .and_then(|r| r.dyn_into::<web_sys::Node>().ok())
                .map(|node| container.contains(Some(&node)))
                .unwrap_or(false);
            if !inside {
                drag_state.drop_target_index.set(None);
            }
        }
    };

    view! {
        <div class="card-list">
            <div class="btn-new-card-group" node_ref=new_card_group_ref>
                <button class="btn-new-card" on:click=on_new_card>
                    "+ New Card"
                </button>
                <button class="btn-new-card-dropdown" on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    new_card_dropdown.update(|v| *v = !*v);
                }>
                    {move || if new_card_dropdown.get() { "\u{25B4}" } else { "\u{25BE}" }}
                </button>
                {move || new_card_dropdown.get().then(|| {
                    let has_selected = state.selected_card.get_untracked().is_some();
                    view! {
                        <div class="new-card-dropdown-menu">
                            <button class="save-dropdown-item" on:click=move |_| {
                                start_creating(NewCardPosition::Bottom);
                            }>"Add to bottom"</button>
                            <button class="save-dropdown-item" on:click=move |_| {
                                start_creating(NewCardPosition::Top);
                            }>"Add to top"</button>
                            <button
                                class="save-dropdown-item"
                                disabled=!has_selected
                                on:click=move |_| {
                                    if let Some(id) = state.selected_card.get_untracked() {
                                        start_creating(NewCardPosition::Above(id));
                                    }
                                }
                            >"Add above selected"</button>
                            <button
                                class="save-dropdown-item"
                                disabled=!has_selected
                                on:click=move |_| {
                                    if let Some(id) = state.selected_card.get_untracked() {
                                        start_creating(NewCardPosition::Below(id));
                                    }
                                }
                            >"Add below selected"</button>
                        </div>
                    }
                })}
            </div>
            <div
                class="cards"
                on:dragover=on_dragover_cards
                on:drop=on_drop_cards
                on:dragleave=on_dragleave_cards
            >
                {move || {
                    let cards = filtered.get();
                    let counts = link_counts.get();
                    let total = cards.len();
                    cards.into_iter().enumerate().map(|(i, card)| {
                        let c = counts.get(&card.id());
                        let fwd = c.map(|c| c.forward).unwrap_or(0);
                        let back = c.map(|c| c.back).unwrap_or(0);
                        view! { <CardItem card=card index=i+1 total=total link_forward=fwd link_back=back /> }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}

/// Apply a move placement from drag-and-drop (mirrors card_detail's apply_move_placement).
fn apply_drag_placement(
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
            state.upsert_card(updated.clone());
            leptos::task::spawn_local(async move {
                if let Some(client) = get_client() {
                    if let Err(e) = client.push_card_versions(vec![updated]).await {
                        log::error!("Failed to push drag-move: {e}");
                    }
                }
            });
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

            state.upsert_card(updated.clone());
            for sc in &shifted_cards {
                state.upsert_card(sc.clone());
            }

            leptos::task::spawn_local(async move {
                if let Some(client) = get_client() {
                    let mut items: Vec<PushItem> = shifted_cards
                        .into_iter()
                        .map(|c| PushItem::Cards(vec![c]))
                        .collect();
                    items.push(PushItem::Cards(vec![updated]));
                    if let Err(e) = client.push_batch(items).await {
                        log::error!("Failed to push rebalanced drag-move: {e}");
                    }
                }
            });
        }
    }
}

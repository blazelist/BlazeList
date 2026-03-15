use crate::components::card_item::CardItem;
use crate::components::hooks::use_click_outside_close;
use crate::state::store::{
    AppState, NewCardPosition, confirm_discard_changes, sync_query_params,
};
use blazelist_protocol::Entity;
use leptos::prelude::*;

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
            <div class="cards">
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


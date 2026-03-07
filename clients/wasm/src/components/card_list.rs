use crate::components::card_item::CardItem;
use crate::state::store::{AppState, confirm_discard_changes, sync_query_params};
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

    let on_new_card = move |_| {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.selected_card.set(None);
        state.editing.set(false);
        state.creating_new.set(true);
        sync_query_params(&state);
    };

    view! {
        <div class="card-list">
            <button class="btn-new-card" on:click=on_new_card>
                "+ New Card"
            </button>
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

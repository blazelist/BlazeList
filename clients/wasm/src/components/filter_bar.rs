use crate::state::store::{
    AppState, DueDateFilter, SortOrder, TagFilterMode, sync_query_params, tag_chip_style,
};
use blazelist_protocol::CardFilter;
use leptos::prelude::*;

#[component]
pub fn FilterBar() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let set_filter = move |f: CardFilter| {
        state.filter.set(f);
        sync_query_params(&state);
    };

    let set_due_filter = move |f: DueDateFilter| {
        // Toggle: clicking the active filter resets to All
        let current = state.due_date_filter.get_untracked();
        if current == f {
            state.due_date_filter.set(DueDateFilter::All);
        } else {
            state.due_date_filter.set(f);
        }
        sync_query_params(&state);
    };

    let due_class = move |f: DueDateFilter| {
        if state.due_date_filter.get() == f {
            "filter-btn active"
        } else {
            "filter-btn"
        }
    };

    let toggle_mode = move |_| {
        state.tag_filter_mode.update(|m| *m = m.toggle());
        if state.tag_filter_mode.get_untracked() == TagFilterMode::And {
            state.no_tags_filter.set(false);
        }
        sync_query_params(&state);
    };

    let active_class = move |f: CardFilter| {
        if state.filter.get() == f {
            "filter-btn active"
        } else {
            "filter-btn"
        }
    };

    let mode_text = move || state.tag_filter_mode.get().label().to_string();

    let tag_chips = move || {
        let tags = state.tag_filter.get();
        let all_tags = state.tags.get();
        tags.into_iter()
            .filter_map(|id| {
                let tag = all_tags.iter().find(|t| {
                    use blazelist_protocol::Entity;
                    t.id() == id
                })?;
                Some((id, tag.title().to_string(), tag.color()))
            })
            .collect::<Vec<_>>()
    };

    let has_tags = move || !state.tag_filter.get().is_empty() || state.no_tags_filter.get();

    let clear_all_tags = move |_| {
        state.tag_filter.set(Vec::new());
        state.no_tags_filter.set(false);
        sync_query_params(&state);
    };

    let has_search = move || !state.search_query.get().is_empty();

    let clear_search = move |_| {
        state.search_query.set(String::new());
    };

    let on_sort_change = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        let order = SortOrder::from_url_value(&val);
        state.sort_order.set(order);
        sync_query_params(&state);
    };

    let reset_sort = move |_| {
        state.sort_order.set(SortOrder::default());
        sync_query_params(&state);
    };

    let sort_is_non_default = move || !state.sort_order.get().is_default();

    view! {
        <div class="filter-bar">
            <div class="blaze-filter">
                <button class=move || active_class(CardFilter::Extinguished) on:click=move |_| set_filter(CardFilter::Extinguished)>"Active"</button>
                <button class=move || active_class(CardFilter::All) on:click=move |_| set_filter(CardFilter::All)>"All"</button>
                <button class=move || active_class(CardFilter::Blazed) on:click=move |_| set_filter(CardFilter::Blazed)>"Blazed"</button>
            </div>
            <div class="due-filter">
                <button class=move || due_class(DueDateFilter::Overdue) on:click=move |_| set_due_filter(DueDateFilter::Overdue)>"Overdue"</button>
                <button class=move || due_class(DueDateFilter::Today) on:click=move |_| set_due_filter(DueDateFilter::Today)>"Today"</button>
                <button class=move || due_class(DueDateFilter::Upcoming) on:click=move |_| set_due_filter(DueDateFilter::Upcoming)>"Upcoming"</button>
            </div>
            <div class="sort-controls">
                <select
                    class="sort-select"
                    on:change=on_sort_change
                    prop:value=move || state.sort_order.get().url_value().unwrap_or("priority").to_string()
                >
                    {SortOrder::ALL.iter().map(|&order| {
                        let val = order.url_value().unwrap_or("priority").to_string();
                        view! { <option value=val.clone()>{order.label()}</option> }
                    }).collect::<Vec<_>>()}
                </select>
                {move || sort_is_non_default().then(|| view! {
                    <button class="btn-clear-x" on:click=reset_sort title="Reset to default sort">"x"</button>
                })}
            </div>
            <div class="search-controls">
                <input
                    class="search-input"
                    type="text"
                    placeholder="Search..."
                    prop:value=move || state.search_query.get()
                    on:input=move |ev| {
                        state.search_query.set(event_target_value(&ev));
                    }
                />
                {move || has_search().then(|| view! {
                    <button class="btn-clear-x" on:click=clear_search title="Clear search">"x"</button>
                })}
            </div>
            {move || has_tags().then(|| view! {
                <div class="tag-filter-controls">
                    <button class="mode-btn" on:click=toggle_mode>{mode_text}</button>
                    <div class="tag-chips">
                        {move || state.no_tags_filter.get().then(|| {
                            let remove = move |_| {
                                state.no_tags_filter.set(false);
                                sync_query_params(&state);
                            };
                            view! {
                                <span class="tag-chip no-tags-chip">
                                    "No tags"
                                    <button class="chip-remove" on:click=remove>"x"</button>
                                </span>
                            }
                        })}
                        {move || tag_chips().into_iter().map(|(id, title, color)| {
                            let remove = move |_| {
                                state.tag_filter.update(|tags| tags.retain(|t| *t != id));
                                sync_query_params(&state);
                            };
                            let style = tag_chip_style(&color);
                            view! {
                                <span class="tag-chip" style=style>
                                    {title}
                                    <button class="chip-remove" on:click=remove>"x"</button>
                                </span>
                            }
                        }).collect::<Vec<_>>()}
                        <button class="btn-clear-x" on:click=clear_all_tags title="Clear all tags">"x"</button>
                    </div>
                </div>
            })}
            {move || {
                let link_ids = state.linked_card_filter.get();
                if link_ids.is_empty() {
                    return None;
                }
                let cards = state.cards.get();
                let items = blazelist_client_lib::display::resolve_linked_cards(&link_ids, &cards, 500);
                let clear_links = move |_| {
                    state.linked_card_filter.set(Vec::new());
                    sync_query_params(&state);
                };
                Some(view! {
                    <div class="linked-filter-list">
                        {items.into_iter().map(|(id, preview)| {
                            let short = format!("{}\u{2026}", &id.to_string()[..8]);
                            let remove = move |_| {
                                state.linked_card_filter.update(|ids| ids.retain(|i| *i != id));
                                sync_query_params(&state);
                            };
                            view! {
                                <div class="linked-filter-row">
                                    <span class="linked-filter-id">{short}</span>
                                    <span class="linked-filter-preview">{preview}</span>
                                    <button class="chip-remove" on:click=remove title="Remove">"x"</button>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                        <button class="btn-clear-x linked-filter-clear" on:click=clear_links title="Clear all">"x"</button>
                    </div>
                })
            }}
        </div>
    }
}

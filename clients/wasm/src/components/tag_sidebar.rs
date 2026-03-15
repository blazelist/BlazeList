use crate::components::sequence_history::SequenceHistory;
use crate::state::store::{
    AppState, confirm_discard_changes, sync_query_params,
};
use blazelist_protocol::Entity;
use leptos::prelude::*;

#[component]
pub fn TagSidebar() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let search = RwSignal::new(String::new());

    let on_new_tag = move |_| {
        state.selected_card.set(None);
        state.creating_new.set(false);
        state.editing.set(false);
        state.creating_new_tag.set(true);
        state.settings_open.set(false);
        sync_query_params(&state);
    };

    view! {
        <>
            <div class="tag-sidebar-header">
                <h3>"Tags"</h3>
                <button class="btn-new-tag" on:click=on_new_tag>"+ New Tag"</button>
                <input
                    class="tag-search-input"
                    type="text"
                    placeholder="Search tags\u{2026}"
                    prop:value=move || search.get()
                    on:input=move |ev| search.set(event_target_value(&ev))
                />
            </div>
            <ul class="tag-list">
                {move || {
                    let q = search.get().to_lowercase();
                    let show_no_tags = q.is_empty() || "no tags".contains(&q);
                    show_no_tags.then(|| view! {
                        <li
                            class=move || if state.no_tags_filter.get() { "tag-item active" } else { "tag-item" }
                            style="border-left: 3px solid transparent;"
                            on:click=move |_| {
                                let enabling = !state.no_tags_filter.get_untracked();
                                state.no_tags_filter.set(enabling);
                                if enabling && state.tag_filter_mode.get_untracked() == blazelist_client_lib::filter::TagFilterMode::And {
                                    state.tag_filter.set(Vec::new());
                                    state.tag_filter_mode.set(blazelist_client_lib::filter::TagFilterMode::Or);
                                }
                                sync_query_params(&state);
                            }
                        >
                            <span class="tag-title no-tags-label">"No tags"</span>
                        </li>
                    })
                }}
                {move || {
                let q = search.get().to_lowercase();
                let mut tags = state.tags.get();
                tags.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));
                if !q.is_empty() {
                    tags.retain(|t| t.title().to_lowercase().contains(&q));
                }
                tags.into_iter().map(|tag| {
                    let tag_id = tag.id();
                    let title = tag.title().to_string();
                    let color = tag.color().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                    let is_active = move || state.tag_filter.get().contains(&tag_id);

                    let toggle_filter = move |_| {
                        if state.tag_filter_mode.get_untracked() == blazelist_client_lib::filter::TagFilterMode::And {
                            state.no_tags_filter.set(false);
                        }
                        state.tag_filter.update(|tags| {
                            if tags.contains(&tag_id) {
                                tags.retain(|t| *t != tag_id);
                            } else {
                                tags.push(tag_id);
                            }
                        });
                        sync_query_params(&state);
                    };

                    let on_manage = move |ev: web_sys::MouseEvent| {
                        ev.stop_propagation();
                        if !confirm_discard_changes(&state) {
                            return;
                        }
                        state.selected_card.set(Some(tag_id));
                        state.creating_new_tag.set(false);
                        state.editing.set(false);
                        state.creating_new.set(false);
                        state.settings_open.set(false);
                        sync_query_params(&state);
                    };

                    let item_class = move || if is_active() { "tag-item active" } else { "tag-item" };

                    let border_style = color
                        .map(|c| format!("border-left: 3px solid {c};"))
                        .unwrap_or_else(|| "border-left: 3px solid transparent;".to_string());

                    view! {
                        <li class=item_class style=border_style on:click=toggle_filter>
                            <span class="tag-title">{title}</span>
                            <button class="tag-manage" on:click=on_manage>"\u{2026}"</button>
                        </li>
                    }
                }).collect::<Vec<_>>()
                }}
            </ul>
            <div class="sidebar-stats">
                {move || {
                    // Read tick to re-evaluate periodically
                    let _ = state.tick.get();
                    let root_hash = state.root.get()
                        .map(|r| r.hash.to_hex().to_string())
                        .unwrap_or_else(|| "---".to_string());

                    let sequence = state.root.get()
                        .map(|r| format!("{}", r.sequence))
                        .unwrap_or_else(|| "---".to_string());
                    let total_cards = state.cards.get().len();
                    let blazed_cards = state.cards.get().iter().filter(|c| c.blazed()).count();
                    let active_cards = total_cards - blazed_cards;
                    let total_tags = state.tags.get().len();
                    let deleted = state.deleted_count.get();
                    let total_entities = total_cards + total_tags + deleted;
                    let synced = state.last_synced.get()
                        .map(|ts| {
                            let secs = blazelist_protocol::Utc::now()
                                .signed_duration_since(ts)
                                .num_seconds()
                                .max(0);
                            format!("{secs}s ago")
                        })
                        .unwrap_or_else(|| "never".to_string());
                    let sync_duration = state.last_sync_duration_ms.get()
                        .map(|ms| format!("{ms}ms"))
                        .unwrap_or_else(|| "---".to_string());
                    let sync_ops = state.last_sync_ops.get();
                    let sync_ops_str = if sync_ops > 0 {
                        sync_ops.to_string()
                    } else {
                        "---".to_string()
                    };
                    view! {
                        <div class="meta-row">
                            <span class="meta-label">"Last Sync"</span>
                            <span class="meta-value">{synced}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Last Sync Operations"</span>
                            <span class="meta-value">{sync_ops_str}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Last Sync Duration"</span>
                            <span class="meta-value">{sync_duration}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Total Cards"</span>
                            <span class="meta-value">{total_cards.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Active Cards"</span>
                            <span class="meta-value">{active_cards.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Blazed Cards"</span>
                            <span class="meta-value">{blazed_cards.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Tags"</span>
                            <span class="meta-value">{total_tags.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Deleted Entities"</span>
                            <span class="meta-value">{deleted.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Total Entities"</span>
                            <span class="meta-value">{total_entities.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Sequence"</span>
                            <span class="meta-value">{sequence}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Root Hash"</span>
                            <span class="meta-value">{root_hash}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"WASM"</span>
                            <span class="meta-value">{concat!("v", env!("CARGO_PKG_VERSION"))}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Client Lib"</span>
                            <span class="meta-value">{format!("v{}", blazelist_client_lib::VERSION)}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Protocol"</span>
                            <span class="meta-value">{format!("v{}", blazelist_protocol::PROTOCOL_VERSION)}</span>
                        </div>
                    }
                }}
            </div>
            <SequenceHistory />
        </>
    }
}

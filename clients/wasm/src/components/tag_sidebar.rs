use crate::components::sequence_history::SequenceHistory;
use crate::state::store::{AppState, format_relative_time, get_client, sync_query_params};
use blazelist_client_lib::client::Client as _;
use blazelist_protocol::{Entity, Tag, Utc};
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn TagSidebar() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let new_tag_title = RwSignal::new(String::new());

    let create_tag = move || {
        let state = state.clone();
        let title = new_tag_title.get_untracked();
        if title.trim().is_empty() {
            return;
        }
        new_tag_title.set(String::new());
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                let tag = Tag::first(Uuid::new_v4(), title, None, Utc::now());
                if let Err(e) = client.push_tag(tag.clone()).await {
                    log::error!("Failed to create tag: {e}");
                    return;
                }
                state.tags.update(|tags| tags.push(tag));
            }
        });
    };

    view! {
        <>
            <div class="tag-sidebar-header">
                <h3>"Tags"</h3>
                <form class="tag-input" on:submit=move |ev| {
                    ev.prevent_default();
                    create_tag();
                }>
                    <input
                        type="text"
                        placeholder="New tag..."
                        prop:value=move || new_tag_title.get()
                        on:input=move |ev| new_tag_title.set(event_target_value(&ev))
                    />
                    <button type="submit">"+"</button>
                </form>
            </div>
            <ul class="tag-list">
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
                {move || {
                let mut tags = state.tags.get();
                tags.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));
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
                        state.selected_card.set(Some(tag_id));
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
                    let synced = state.last_synced.get()
                        .map(|ts| format_relative_time(&ts))
                        .unwrap_or_else(|| "never".to_string());
                    view! {
                        <div class="meta-row">
                            <span class="meta-label">"Root Hash"</span>
                            <span class="meta-value">{root_hash}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Sequence"</span>
                            <span class="meta-value">{sequence}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Cards"</span>
                            <span class="meta-value">{total_cards.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Active"</span>
                            <span class="meta-value">{active_cards.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Blazed"</span>
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
                            <span class="meta-label">"Last Sync"</span>
                            <span class="meta-value">{synced}</span>
                        </div>
                    }
                }}
            </div>
            <SequenceHistory />
        </>
    }
}

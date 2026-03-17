use crate::components::hooks::{handle_code_copy_click, toggle_expanded};
use crate::state::store::{
    AppState, format_relative_time, get_client, sync_query_params, tag_chip_style,
};
use crate::storage;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::display::card_preview;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, place_card,
};
use blazelist_protocol::{Card, Entity, PushItem, Utc};
use leptos::prelude::*;
use uuid::Uuid;

fn render_markdown(content: &str) -> String {
    let html = comrak::markdown_to_html(content, &blazelist_client_lib::display::markdown_options());
    blazelist_client_lib::display::wrap_code_blocks_with_copy_button(&html)
}

/// Inline version history section for a card. Renders a "History" label
/// followed by an expandable version list with "New from this" buttons.
#[component]
pub fn VersionHistory(card_id: Uuid) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let versions: RwSignal<Vec<Card>> = RwSignal::new(Vec::new());
    let loading = RwSignal::new(true);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let expanded: RwSignal<Option<i64>> = RwSignal::new(None);
    let prev_card: RwSignal<Option<Uuid>> = RwSignal::new(None);

    // Fetch history on mount — show cached data first, then refresh from server.
    // Also re-trigger when connection status changes so that history is fetched
    // after the client connects (the card detail no longer re-creates this
    // component on sync, so the Effect must retry on its own).
    Effect::new(move |_| {
        let selected = state.selected_card.get(); // re-trigger on card change
        let _ = state.connection_status.get(); // re-trigger on connect

        // Only reset UI state when the selected card changes,
        // not on every connection status transition.
        if prev_card.get_untracked() != selected {
            error_msg.set(None);
            expanded.set(None);
            prev_card.set(selected);
        }

        // Load from cache immediately
        let cached = storage::get_cached_card_history(card_id);
        if !cached.is_empty() {
            versions.set(cached);
            loading.set(false);
        } else {
            loading.set(true);
        }

        // Fetch fresh data from server in background
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                match client.get_card_history(card_id).await {
                    Ok(mut history) => {
                        history.sort_by(|a, b| b.count().cmp(&a.count()));
                        versions.set(history.clone());
                        storage::update_cached_card_history(card_id, history);
                        storage::save_history_cache().await;
                    }
                    Err(e) => {
                        if versions.get_untracked().is_empty() {
                            error_msg.set(Some(format!("Failed to load history: {e}")));
                        }
                    }
                }
            }
            loading.set(false);
        });
    });

    let on_toggle_expand = move |count: i64| {
        toggle_expanded(expanded, count);
    };

    let on_restore = move |version: Card| {
        let state = state;
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                let current = state
                    .cards
                    .get_untracked()
                    .into_iter()
                    .find(|c| c.id() == card_id);
                if let Some(current) = current {
                    let restored = current.next(
                        version.content().to_string(),
                        current.priority(),
                        version.tags().to_vec(),
                        version.blazed(),
                        Utc::now(),
                        version.due_date(),
                    );
                    if let Err(e) = client.push_card(restored.clone()).await {
                        tracing::error!(%e, "Failed to restore card version");
                        return;
                    }
                    state.upsert_card(restored);
                }
            }
        });
    };

    let on_fork = move |version: Card| {
        let state = state;
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                let mut cards = state.cards.get_untracked();
                cards.sort_by(|a, b| b.priority().cmp(&a.priority()));
                let placement = place_card(&cards, InsertPosition::Bottom);
                match placement {
                    Placement::Simple(priority) => {
                        let card = Card::first(
                            Uuid::new_v4(),
                            version.content().to_string(),
                            priority,
                            version.tags().to_vec(),
                            version.blazed(),
                            Utc::now(),
                            version.due_date(),
                        );
                        let new_id = card.id();
                        if let Err(e) = client.push_card(card.clone()).await {
                            tracing::error!(%e, "Failed to create card from version");
                            return;
                        }
                        state.upsert_card(card);
                        state.selected_card.set(Some(new_id));
                        sync_query_params(&state);
                    }
                    Placement::Rebalanced { priority, shifted } => {
                        let card = Card::first(
                            Uuid::new_v4(),
                            version.content().to_string(),
                            priority,
                            version.tags().to_vec(),
                            version.blazed(),
                            Utc::now(),
                            version.due_date(),
                        );
                        let shifted_cards = build_shifted_versions(&shifted, &cards);
                        let mut items: Vec<PushItem> = shifted_cards
                            .into_iter()
                            .map(|c| PushItem::Cards(vec![c]))
                            .collect();
                        items.push(PushItem::Cards(vec![card.clone()]));
                        let new_id = card.id();
                        if let Err(e) = client.push_batch(items).await {
                            tracing::error!(%e, "Failed to create card from version (rebalanced)");
                            return;
                        }
                        state.upsert_card(card);
                        state.selected_card.set(Some(new_id));
                        sync_query_params(&state);
                    }
                }
            }
        });
    };

    view! {
        <div class="tag-history-section">
            <span class="tag-history-label">"History"</span>
            {move || {
                if loading.get() {
                    return view! {
                        <div class="version-list">
                            <p class="version-loading">"Loading history\u{2026}"</p>
                        </div>
                    }.into_any();
                }
                if let Some(err) = error_msg.get() {
                    return view! {
                        <div class="version-list">
                            <p class="error">{err}</p>
                        </div>
                    }.into_any();
                }
                let items = versions.get();
                if items.is_empty() {
                    return view! {
                        <div class="version-list">
                            <p class="version-loading">"No history available."</p>
                        </div>
                    }.into_any();
                }
                let expanded_count = expanded.get();
                let total = items.len();
                let num_width = total.max(1).ilog10() as usize + 1;
                let max_count = items.first().map(|v| i64::from(v.count()));
                view! {
                    <div class="version-list">
                        {items.into_iter().map(|v| {
                            let count = i64::from(v.count());
                            let number = format!("{:0>width$}", count, width = num_width);
                            let time_str = format_relative_time(&v.modified_at());
                            let preview_text = card_preview(v.content(), 60)
                                .unwrap_or_else(|| "(empty)".to_string());
                            let due_date = v.due_date();
                            let is_current = max_count == Some(count);
                            let is_expanded = expanded_count == Some(count);
                            let item_class = if is_expanded {
                                "version-item expanded"
                            } else {
                                "version-item"
                            };
                            let expanded_view = if is_expanded {
                                let v_for_fork = v.clone();
                                let v_for_restore = v.clone();
                                let on_fork = on_fork.clone();
                                let on_restore = on_restore.clone();
                                let content_html = render_markdown(v.content());
                                let created = v.created_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                let modified = v.modified_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                let is_blazed = v.blazed();
                                let priority = v.priority();
                                let priority_pct = blazelist_client_lib::priority::priority_percentage(priority);

                                let all_tags = state.tags.get_untracked();
                                let mut tag_entries: Vec<(String, Option<rgb::RGB8>)> = v.tags().iter().filter_map(|tid| {
                                    all_tags.iter().find(|t| t.id() == *tid).map(|t| {
                                        (t.title().to_string(), t.color())
                                    })
                                }).collect();
                                tag_entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

                                Some(view! {
                                    <div class="version-expanded">
                                        <div class="version-preview" inner_html=content_html on:click=move |ev: web_sys::MouseEvent| {
                                            handle_code_copy_click(&ev);
                                        }></div>
                                        <div class="version-detail-meta">
                                            {(!tag_entries.is_empty()).then(|| {
                                                let tags = tag_entries.clone();
                                                view! {
                                                    <div class="meta-row">
                                                        <span class="meta-label">"Tags"</span>
                                                        <div class="detail-tag-chips">
                                                            {tags.into_iter().map(|(name, color)| {
                                                                let style = tag_chip_style(&color);
                                                                view! {
                                                                    <span class="tag-chip" style=style>{name}</span>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    </div>
                                                }
                                            })}
                                            <div class="meta-row">
                                                <span class="meta-label">"Status"</span>
                                                {if is_blazed {
                                                    view! { <span class="meta-value detail-status blazed">"Blazed"</span> }.into_any()
                                                } else {
                                                    view! { <span class="meta-value detail-status active">"Active"</span> }.into_any()
                                                }}
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Priority"</span>
                                                <span class="meta-value">{format!("{priority} ({priority_pct:.2}%)")}</span>
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Modified"</span>
                                                <span class="meta-value">{modified}</span>
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Created"</span>
                                                <span class="meta-value">{created}</span>
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Due"</span>
                                                {match due_date.map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string()) {
                                                    Some(d) => view! {
                                                        <span class="meta-value">{d}</span>
                                                    }.into_any(),
                                                    None => view! {
                                                        <span class="meta-value due-not-set">"Not set"</span>
                                                    }.into_any(),
                                                }}
                                            </div>
                                        </div>
                                        <div class="version-expanded-actions">
                                            <button class="btn-fork" on:click=move |ev| {
                                                ev.stop_propagation();
                                                let v = v_for_fork.clone();
                                                on_fork(v);
                                            }>"New from this"</button>
                                            {(!is_current).then(|| {
                                                let on_restore = on_restore.clone();
                                                view! {
                                                    <button class="btn-restore" on:click=move |ev| {
                                                        ev.stop_propagation();
                                                        let v = v_for_restore.clone();
                                                        on_restore(v);
                                                    }>"Restore"</button>
                                                }
                                            })}
                                        </div>
                                    </div>
                                })
                            } else {
                                None
                            };
                            view! {
                                <div class=item_class>
                                    <div class="version-row" on:click=move |_| on_toggle_expand(count)>
                                        <span class="version-number">{number.clone()}</span>
                                        <span class="version-preview-text">{preview_text.clone()}</span>
                                        {is_current.then(|| view! {
                                            <span class="version-current-badge">"current"</span>
                                        })}
                                        <span class="version-time">{time_str.clone()}</span>
                                    </div>
                                    {expanded_view}
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

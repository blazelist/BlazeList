use crate::components::hooks::toggle_expanded;
use crate::state::store::{
    AppState, confirm_discard_changes, format_relative_time, get_client, sync_query_params,
};
use crate::storage;
use blazelist_client_lib::client::Client as _;
use blazelist_protocol::{Card, Entity, PushItem, Tag, Utc};
use leptos::prelude::*;
use rgb::RGB8;

#[component]
pub fn TagDetail() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let versions: RwSignal<Vec<Tag>> = RwSignal::new(Vec::new());
    let loading = RwSignal::new(true);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let expanded: RwSignal<Option<i64>> = RwSignal::new(None);

    // Editing state
    let editing_title = RwSignal::new(false);
    let title_input = RwSignal::new(String::new());
    let color_input = RwSignal::new(String::from("#808080"));
    let confirm_delete = RwSignal::new(0u8);

    // Fetch tag history on mount — show cached data first, then refresh from server.
    Effect::new(move |_| {
        let tag_id = match state.selected_card.get() {
            Some(id) => id,
            None => return,
        };
        // Only fetch if this UUID is actually a tag
        if !state.tags.get_untracked().iter().any(|t| t.id() == tag_id) {
            return;
        }
        error_msg.set(None);
        expanded.set(None);
        editing_title.set(false);
        confirm_delete.set(0);
        // Initialize color picker with the current tag's color
        if let Some(tag) = state.tags.get_untracked().iter().find(|t| t.id() == tag_id) {
            color_input.set(
                tag.color()
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                    .unwrap_or_else(|| "#808080".to_string()),
            );
        }

        // Load from cache immediately
        let cached = storage::get_cached_tag_history(tag_id);
        if !cached.is_empty() {
            versions.set(cached);
            loading.set(false);
        } else {
            loading.set(true);
        }

        // Fetch fresh data from server in background
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                match client.get_tag_history(tag_id).await {
                    Ok(mut history) => {
                        history.sort_by(|a, b| b.count().cmp(&a.count()));
                        versions.set(history.clone());
                        storage::update_cached_tag_history(tag_id, history);
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

    let on_close = move |_| {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.selected_card.set(None);
        state.editing.set(false);
        sync_query_params(&state);
    };

    let on_start_edit = move |_| {
        if !confirm_discard_changes(&state) {
            return;
        }
        let tag_id = state.selected_card.get_untracked().unwrap();
        if let Some(tag) = state
            .tags
            .get_untracked()
            .into_iter()
            .find(|t| t.id() == tag_id)
        {
            title_input.set(tag.title().to_string());
            editing_title.set(true);
        }
    };

    let on_cancel_edit = move |_| {
        editing_title.set(false);
    };

    let on_save_title = move |_| {
        let tag_id = state.selected_card.get_untracked().unwrap();
        let new_title = title_input.get_untracked();
        if new_title.trim().is_empty() {
            return;
        }
        let tag = state
            .tags
            .get_untracked()
            .into_iter()
            .find(|t| t.id() == tag_id);
        let tag = match tag {
            Some(t) => t,
            None => return,
        };
        let updated = tag.next(new_title, tag.color(), Utc::now());
        editing_title.set(false);
        let state = state;
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                if let Err(e) = client.push_tag(updated.clone()).await {
                    log::error!("Failed to rename tag: {e}");
                    return;
                }
                state.tags.update(|tags| {
                    if let Some(t) = tags.iter_mut().find(|t| t.id() == tag_id) {
                        *t = updated.clone();
                    }
                });
                // Refresh history
                versions.update(|v| v.insert(0, updated));
            }
        });
    };

    let deleting = RwSignal::new(false);

    let on_delete = move |_| {
        let tag_id = match state.selected_card.get_untracked() {
            Some(id) => id,
            None => return,
        };
        deleting.set(true);
        error_msg.set(None);
        let state = state;
        leptos::task::spawn_local(async move {
            let client = match get_client() {
                Some(c) => c,
                None => {
                    error_msg.set(Some("Not connected to server".to_string()));
                    deleting.set(false);
                    return;
                }
            };

            // Collect all cards referencing this tag and build cleanup versions
            let cards = state.cards.get_untracked();
            let affected: Vec<Card> = cards
                .iter()
                .filter(|c| c.tags().contains(&tag_id))
                .map(|c| {
                    let new_tags: Vec<uuid::Uuid> =
                        c.tags().iter().copied().filter(|t| *t != tag_id).collect();
                    c.next(
                        c.content().to_string(),
                        c.priority(),
                        new_tags,
                        c.blazed(),
                        Utc::now(),
                        c.due_date(),
                    )
                })
                .collect();

            // Build batch: card updates first, then delete tag
            let mut items: Vec<PushItem> = affected
                .iter()
                .map(|c| PushItem::Cards(vec![c.clone()]))
                .collect();
            items.push(PushItem::DeleteTag { id: tag_id });

            if let Err(e) = client.push_batch(items).await {
                log::error!("Failed to delete tag: {e}");
                error_msg.set(Some(format!("Delete failed: {e}")));
                confirm_delete.set(0);
                deleting.set(false);
                return;
            }

            // Update local state for affected cards
            for card in &affected {
                state.upsert_card(card.clone());
            }
            state.tags.update(|tags| tags.retain(|t| t.id() != tag_id));
            state
                .tag_filter
                .update(|tags| tags.retain(|t| *t != tag_id));
            state.selected_card.set(None);
            deleting.set(false);
            sync_query_params(&state);
        });
    };

    let on_toggle_expand = move |count: i64| {
        toggle_expanded(expanded, count);
    };

    view! {
        <div class="detail-header">
            <span class="detail-status tag-not-card">"Tag"</span>
            <button class="detail-close" on:click=on_close>"x"</button>
        </div>

        // Title section
        {move || {
            let tag_id = state.selected_card.get()?;
            let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
            let title = tag.title().to_string();

            if editing_title.get() {
                Some(view! {
                    <div class="tag-title-section">
                        <form class="tag-rename-form" on:submit=move |ev| {
                            ev.prevent_default();
                            on_save_title(());
                        }>
                            <input
                                class="tag-rename-input"
                                type="text"
                                prop:value=move || title_input.get()
                                on:input=move |ev| title_input.set(event_target_value(&ev))
                            />
                            <button type="submit" class="btn-save">"Save"</button>
                            <button type="button" class="btn-cancel" on:click=on_cancel_edit>"Cancel"</button>
                        </form>
                    </div>
                }.into_any())
            } else {
                Some(view! {
                    <div class="tag-title-section tag-title-editable" on:click=on_start_edit title="Click to rename">
                        <span class="tag-detail-title">{title}</span>
                        <span class="tag-rename-icon">{"\u{270E}"}</span>
                    </div>
                }.into_any())
            }
        }}

        // Error message
        {move || error_msg.get().map(|msg| view! {
            <div class="error">{msg}</div>
        })}

        // Actions
        <div class="card-actions">
            <div class="action-row cmd-row">
                {move || {
                    if deleting.get() {
                        return view! {
                            <span class="confirm-text">"Deleting\u{2026}"</span>
                        }.into_any();
                    }
                    let step = confirm_delete.get();
                    if step == 2 {
                        let tag_id = state.selected_card.get();
                        let tag_title = tag_id
                            .and_then(|id| state.tags.get().into_iter().find(|t| t.id() == id))
                            .map(|t| t.title().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        view! {
                            <div class="confirm-delete-permanent">
                                <span class="confirm-text-permanent">"This action is permanent and cannot be undone."</span>
                                <span class="confirm-entity-info">{format!("Tag: {tag_title}")}</span>
                                <div class="confirm-permanent-buttons">
                                    <button class="btn-confirm-permanent" on:click=on_delete>"Delete permanently"</button>
                                    <button class="btn-confirm-no" on:click=move |_| confirm_delete.set(0)>"Cancel"</button>
                                </div>
                            </div>
                        }.into_any()
                    } else if step == 1 {
                        view! {
                            <div class="confirm-delete">
                                <span class="confirm-text">"Delete tag?"</span>
                                <button class="btn-confirm-yes" on:click=move |_| confirm_delete.set(2)>"Yes"</button>
                                <button class="btn-confirm-no" on:click=move |_| confirm_delete.set(0)>"No"</button>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <button class="btn-delete" on:click=move |_| confirm_delete.set(1)>"Delete"</button>
                        }.into_any()
                    }
                }}
            </div>
        </div>

        // Color section
        {move || {
            let tag_id = state.selected_card.get()?;
            let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
            let current_color = tag.color().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
            let has_color = tag.color().is_some();

            let on_set_color = move |_| {
                let tag_id = state.selected_card.get_untracked().unwrap();
                let hex = color_input.get_untracked();
                let hex = hex.trim_start_matches('#');
                if hex.len() != 6 {
                    return;
                }
                let rgb = match (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    (Ok(r), Ok(g), Ok(b)) => RGB8::new(r, g, b),
                    _ => return,
                };
                let tag = state.tags.get_untracked().into_iter().find(|t| t.id() == tag_id);
                let tag = match tag {
                    Some(t) => t,
                    None => return,
                };
                let updated = tag.next(tag.title().to_string(), Some(rgb), Utc::now());
                let state = state;
                leptos::task::spawn_local(async move {
                    if let Some(client) = get_client() {
                        if let Err(e) = client.push_tag(updated.clone()).await {
                            log::error!("Failed to set tag color: {e}");
                            return;
                        }
                        state.tags.update(|tags| {
                            if let Some(t) = tags.iter_mut().find(|t| t.id() == tag_id) {
                                *t = updated.clone();
                            }
                        });
                        versions.update(|v| v.insert(0, updated));
                    }
                });
            };

            let on_clear_color = move |_| {
                let tag_id = state.selected_card.get_untracked().unwrap();
                let tag = state.tags.get_untracked().into_iter().find(|t| t.id() == tag_id);
                let tag = match tag {
                    Some(t) => t,
                    None => return,
                };
                let updated = tag.next(tag.title().to_string(), None, Utc::now());
                let state = state;
                leptos::task::spawn_local(async move {
                    if let Some(client) = get_client() {
                        if let Err(e) = client.push_tag(updated.clone()).await {
                            log::error!("Failed to clear tag color: {e}");
                            return;
                        }
                        state.tags.update(|tags| {
                            if let Some(t) = tags.iter_mut().find(|t| t.id() == tag_id) {
                                *t = updated.clone();
                            }
                        });
                        versions.update(|v| v.insert(0, updated));
                    }
                });
            };

            Some(view! {
                <div class="tag-color-section">
                    <span class="tag-color-label">"Color"</span>
                </div>
                <div class="tag-color-row">
                    <input
                        class="tag-color-input"
                        type="color"
                        prop:value=move || color_input.get()
                        on:input=move |ev| color_input.set(event_target_value(&ev))
                    />
                    {current_color.as_ref().map(|c| {
                        let style = format!("background: {c};");
                        let hex = c.clone();
                        view! {
                            <span class="tag-color-preview" style=style></span>
                            <span class="tag-color-hex">{hex}</span>
                        }
                    })}
                    <button class="btn-save tag-color-btn" on:click=on_set_color>"Set"</button>
                    {has_color.then(|| view! {
                        <button class="btn-cancel tag-color-btn" on:click=on_clear_color>"Clear"</button>
                    })}
                </div>
            })
        }}

        // Metadata (nerd stats)
        {move || {
            let tag_id = state.selected_card.get()?;
            let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
            let id_str = tag_id.to_string();
            let created = tag.created_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
            let modified = tag.modified_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
            let count = tag.count().to_string();
            Some(view! {
                <div class="detail-meta">
                    <div class="meta-row">
                        <span class="meta-label">"ID"</span>
                        <span class="meta-value">{id_str}</span>
                    </div>
                    <div class="meta-row">
                        <span class="meta-label">"Version"</span>
                        <span class="meta-value">{count}</span>
                    </div>
                    <div class="meta-row">
                        <span class="meta-label">"Created"</span>
                        <span class="meta-value">{created}</span>
                    </div>
                    <div class="meta-row">
                        <span class="meta-label">"Modified"</span>
                        <span class="meta-value">{modified}</span>
                    </div>
                </div>
            })
        }}

        // Version history (read-only)
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
                            let title = v.title().to_string();
                            let is_current = max_count == Some(count);
                            let is_expanded = expanded_count == Some(count);
                            let item_class = if is_expanded {
                                "version-item expanded"
                            } else {
                                "version-item"
                            };
                            let expanded_view = if is_expanded {
                                let created = v.created_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                let modified = v.modified_at().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                let version_color = v.color().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                                Some(view! {
                                    <div class="version-expanded">
                                        <div class="version-detail-meta">
                                            <div class="meta-row">
                                                <span class="meta-label">"Title"</span>
                                                <span class="meta-value">{v.title().to_string()}</span>
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Color"</span>
                                                {match version_color {
                                                    Some(c) => {
                                                        let style = format!("background: {c};");
                                                        let hex = c.clone();
                                                        view! {
                                                            <span class="tag-color-preview" style=style></span>
                                                            <span class="meta-value">{hex}</span>
                                                        }.into_any()
                                                    }
                                                    None => view! {
                                                        <span class="meta-value due-not-set">"None"</span>
                                                    }.into_any(),
                                                }}
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Modified"</span>
                                                <span class="meta-value">{modified}</span>
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Created"</span>
                                                <span class="meta-value">{created}</span>
                                            </div>
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
                                        <span class="version-preview-text">{title}</span>
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

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

    // Unified editing state for title + color
    let editing = RwSignal::new(false);
    let title_input = RwSignal::new(String::new());
    let color_input = RwSignal::new(String::from("#808080"));
    let use_color = RwSignal::new(false);
    let confirm_delete = RwSignal::new(0u8);

    // Originals captured when editing starts, for dirty comparison.
    let orig_title = RwSignal::new(String::new());
    let orig_use_color = RwSignal::new(false);
    let orig_color = RwSignal::new(String::from("#808080"));

    // Track dirty state — compare current inputs against originals.
    Effect::new(move |_| {
        if !editing.get() {
            return;
        }
        let dirty = title_input.get() != orig_title.get()
            || use_color.get() != orig_use_color.get()
            || (use_color.get() && color_input.get() != orig_color.get());
        state.has_unsaved_changes.set(dirty);
    });

    // Populate editing inputs from the current tag state.
    let init_inputs = move |tag: &Tag| {
        title_input.set(tag.title().to_string());
        if let Some(c) = tag.color() {
            let hex = format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
            color_input.set(hex.clone());
            use_color.set(true);
            orig_color.set(hex);
            orig_use_color.set(true);
        } else {
            color_input.set(String::from("#808080"));
            use_color.set(false);
            orig_color.set(String::from("#808080"));
            orig_use_color.set(false);
        }
        orig_title.set(tag.title().to_string());
    };

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
        editing.set(false);
        state.has_unsaved_changes.set(false);
        confirm_delete.set(0);
        // Initialize inputs with current tag data
        if let Some(tag) = state.tags.get_untracked().iter().find(|t| t.id() == tag_id) {
            init_inputs(tag);
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
        editing.set(false);
        state.has_unsaved_changes.set(false);
        state.selected_card.set(None);
        state.editing.set(false);
        sync_query_params(&state);
    };

    let start_editing = move || {
        if let Some(tag_id) = state.selected_card.get_untracked() {
            if let Some(tag) = state.tags.get_untracked().iter().find(|t| t.id() == tag_id) {
                init_inputs(tag);
                confirm_delete.set(0);
                editing.set(true);
            }
        }
    };

    let cancel_editing = move || {
        if !confirm_discard_changes(&state) {
            return;
        }
        editing.set(false);
        state.has_unsaved_changes.set(false);
    };

    let save_changes = move || {
        let tag_id = match state.selected_card.get_untracked() {
            Some(id) => id,
            None => return,
        };
        let new_title = title_input.get_untracked();
        if new_title.trim().is_empty() {
            return;
        }
        let new_color = if use_color.get_untracked() {
            let hex = color_input.get_untracked();
            let hex = hex.trim_start_matches('#');
            if hex.len() == 6 {
                match (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    (Ok(r), Ok(g), Ok(b)) => Some(RGB8::new(r, g, b)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        let tag = state
            .tags
            .get_untracked()
            .into_iter()
            .find(|t| t.id() == tag_id);
        let tag = match tag {
            Some(t) => t,
            None => return,
        };
        let updated = tag.next(new_title, new_color, Utc::now());
        editing.set(false);
        state.has_unsaved_changes.set(false);
        let state = state;
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
                if let Err(e) = client.push_tag(updated.clone()).await {
                    tracing::error!(%e, "Failed to save tag");
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
                tracing::error!(%e, "Failed to delete tag");
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
            <div class="detail-header-left">
                {move || if editing.get() {
                    view! { <span class="detail-status editing">"Editing"</span> }.into_any()
                } else {
                    view! { <span class="detail-status tag-not-card">"Tag"</span> }.into_any()
                }}
                {move || (editing.get() && state.has_unsaved_changes.get()).then(|| view! {
                    <span class="unsaved-indicator">"(unsaved)"</span>
                })}
            </div>
            <button class="detail-close" on:click=on_close>"x"</button>
        </div>

        // Title section
        {move || {
            let tag_id = state.selected_card.get()?;
            let editing_now = editing.get();
            // When editing, use get_untracked to avoid auto-sync
            // destroying the input and losing the in-progress text.
            let tag = if editing_now {
                state.tags.get_untracked()
            } else {
                state.tags.get()
            }.into_iter().find(|t| t.id() == tag_id)?;

            if editing_now {
                Some(view! {
                    <div class="tag-title-section">
                        <form class="tag-rename-form" on:submit=move |ev| {
                            ev.prevent_default();
                            save_changes();
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
                }.into_any())
            } else {
                let title = tag.title().to_string();
                Some(view! {
                    <div class="tag-title-section tag-title-editable" on:click=move |_| start_editing() title="Click to edit">
                        <span class="tag-detail-title">{title}</span>
                        <span class="tag-rename-icon">{"\u{270E}"}</span>
                    </div>
                }.into_any())
            }
        }}

        // Color section
        {move || {
            let tag_id = state.selected_card.get()?;
            let editing_now = editing.get();

            if editing_now {
                Some(view! {
                    <div class="tag-color-section">
                        <span class="tag-color-label">"Color"</span>
                    </div>
                    <div class="tag-color-row">
                        <input
                            class="tag-color-input"
                            type="color"
                            prop:value=move || color_input.get()
                            on:input=move |ev| {
                                color_input.set(event_target_value(&ev));
                                use_color.set(true);
                            }
                        />
                        <span
                            class=move || if use_color.get() { "tag-color-preview" } else { "tag-color-preview tag-color-placeholder" }
                            style=move || format!("background: {};", color_input.get())
                        ></span>
                        {move || use_color.get().then(|| {
                            let hex = color_input.get();
                            view! {
                                <span class="tag-color-hex">{hex}</span>
                            }
                        })}
                        {move || use_color.get().then(|| view! {
                            <button class="btn-cancel tag-color-btn" on:click=move |_| {
                                use_color.set(false);
                                color_input.set(String::from("#808080"));
                            }>"Clear"</button>
                        })}
                    </div>
                }.into_any())
            } else {
                let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
                let current_color = tag.color().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));

                Some(view! {
                    <div class="tag-color-section">
                        <span class="tag-color-label">"Color"</span>
                    </div>
                    <div class="tag-color-row">
                        {current_color.as_ref().map(|c| {
                            let style = format!("background: {c};");
                            let hex = c.clone();
                            view! {
                                <span class="tag-color-preview" style=style></span>
                                <span class="tag-color-hex">{hex}</span>
                            }
                        })}
                        {current_color.is_none().then(|| view! {
                            <span class="tag-color-hex due-not-set">"None"</span>
                        })}
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
                    if editing.get() {
                        return view! {
                            <button class="btn-save" on:click=move |_| save_changes()>"Save"</button>
                            <button class="btn-cancel" on:click=move |_| cancel_editing()>"Cancel"</button>
                        }.into_any();
                    }
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
                            <button class="btn-save" on:click=move |_| start_editing()>"Edit"</button>
                            <button class="btn-delete" on:click=move |_| confirm_delete.set(1)>"Delete"</button>
                        }.into_any()
                    }
                }}
            </div>
        </div>

        // Metadata
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

        // Version history
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

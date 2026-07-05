use crate::components::hooks::{ConfirmDeletePrompt, TagColorPicker, parse_hex_color};
use crate::components::timestamp::Timestamp;
use crate::components::toast::show_error_toast;
use crate::components::version_history::TagVersionHistory;
use crate::state::store::{
    AppState, confirm_discard_changes, get_client, set_selection, tag_chip_style,
};
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::error::ClientError;
use blazelist_client_lib::tag_graph::{TagGraph, affected_cards_for_change};
use blazelist_protocol::{Card, Entity, PushItem, Tag, Utc};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;

#[component]
pub fn TagDetail() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let prev_tag: RwSignal<Option<Uuid>> = RwSignal::new(None);

    // Unified editing state for title + color + implies.
    let editing = RwSignal::new(false);
    let title_input = RwSignal::new(String::new());
    let color_input = RwSignal::new(String::from("#808080"));
    let use_color = RwSignal::new(false);
    let implies_input: RwSignal<Vec<Uuid>> = RwSignal::new(Vec::new());
    let confirm_delete = RwSignal::new(0u8);

    // Originals captured when editing starts, for dirty comparison.
    let orig_title = RwSignal::new(String::new());
    let orig_use_color = RwSignal::new(false);
    let orig_color = RwSignal::new(String::from("#808080"));
    let orig_implies: RwSignal<Vec<Uuid>> = RwSignal::new(Vec::new());

    // Track dirty state — compare current inputs against originals.
    Effect::new(move |_| {
        if !editing.get() {
            return;
        }
        let mut cur_implies = implies_input.get();
        cur_implies.sort();
        let mut orig = orig_implies.get();
        orig.sort();
        let dirty = title_input.get() != orig_title.get()
            || use_color.get() != orig_use_color.get()
            || (use_color.get() && color_input.get() != orig_color.get())
            || cur_implies != orig;
        state.has_unsaved_changes.set(dirty);
    });

    // Populate editing inputs from the current tag state.
    let init_inputs = move |tag: &Tag| {
        title_input.set(tag.title().to_string());
        if let Some(c) = tag.color() {
            let hex = blazelist_client_lib::color::format_tag_hex(&c);
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
        let implies = tag.implies().to_vec();
        implies_input.set(implies.clone());
        orig_implies.set(implies);
    };

    // Reset editing state when the selected tag changes.
    Effect::new(move |_| {
        let tag_id = match state.selected_card().get() {
            Some(id) => id,
            None => return,
        };
        // Only run if this UUID is actually a tag
        if !state.tags.get_untracked().iter().any(|t| t.id() == tag_id) {
            return;
        }

        // Only reset UI state when the selected tag changes,
        // not on every re-render with the same tag.
        if prev_tag.get_untracked() != Some(tag_id) {
            editing.set(false);
            state.has_unsaved_changes.set(false);
            confirm_delete.set(0);
            // Initialize inputs with current tag data
            if let Some(tag) = state.tags.get_untracked().iter().find(|t| t.id() == tag_id) {
                init_inputs(tag);
            }
            prev_tag.set(Some(tag_id));
        }
    });

    let on_close = move |_| {
        if !set_selection(&state, None) {
            return;
        }
        // `set_selection` clears its own cluster; the local `editing`
        // (tag-edit inline form) and global `has_unsaved_changes` live
        // outside it.
        editing.set(false);
        state.has_unsaved_changes.set(false);
    };

    let start_editing = move || {
        if let Some(tag_id) = state.selected_card().get_untracked()
            && let Some(tag) = state.tags.get_untracked().iter().find(|t| t.id() == tag_id)
        {
            init_inputs(tag);
            confirm_delete.set(0);
            editing.set(true);
        }
    };

    let cancel_editing = move || {
        if !confirm_discard_changes(&state) {
            return;
        }
        editing.set(false);
        state.has_unsaved_changes.set(false);
    };

    // Apply a new tag version + any required card updates via `push_batch`,
    // then refresh local state on success. Shared by the "no affected cards"
    // fast path and the "user confirmed the update" path.
    //
    // Rebuilds the tag + affected card versions from the freshest local
    // state inside the async task so stale-base-version races with
    // auto-sync are avoided.
    let commit_tag_update = move |title: String,
                                  color: Option<rgb::RGB8>,
                                  implies: Vec<Uuid>,
                                  affected: Vec<(Uuid, Vec<Uuid>)>| {
        let state = state;
        leptos::task::spawn_local(async move {
            let Some(client) = get_client() else {
                show_error_toast(state, "Can't edit tags while offline", 3000);
                editing.set(true);
                return;
            };

            let tag_id = match state.selected_card().get_untracked() {
                Some(id) => id,
                None => return,
            };

            // Re-fetch the tag from the freshest local state to build on
            // the latest version and avoid hash-chain breaks.
            let tag = match state
                .tags
                .get_untracked()
                .into_iter()
                .find(|t| t.id() == tag_id)
            {
                Some(t) => t,
                None => return,
            };
            let new_tag = tag.next_with_implies(title, color, implies, Utc::now());

            // Build the affected card updates from the freshest snapshot.
            let current_cards = state.cards.get_untracked();
            let mut updated_cards: Vec<Card> = Vec::with_capacity(affected.len());
            for (card_id, missing) in &affected {
                let Some(card) = current_cards.iter().find(|c| c.id() == *card_id) else {
                    continue;
                };
                let mut new_tags = card.tags().to_vec();
                for m in missing {
                    if !new_tags.contains(m) {
                        new_tags.push(*m);
                    }
                }
                let next = card.next(
                    card.content().to_string(),
                    card.priority(),
                    new_tags,
                    card.blazed(),
                    Utc::now(),
                    card.due_date(),
                );
                updated_cards.push(next);
            }

            let mut items: Vec<PushItem> = updated_cards
                .iter()
                .cloned()
                .map(|c| PushItem::Cards(vec![c]))
                .collect();
            items.push(PushItem::Tags(vec![new_tag.clone()]));

            match client.push_batch(items).await {
                Ok(_) => {}
                Err(ClientError::ConnectionLost) => {
                    show_error_toast(state, "Can't edit tags while offline", 3000);
                    editing.set(true);
                    return;
                }
                Err(e) => {
                    tracing::error!(%e, "Failed to save tag");
                    show_error_toast(state, &format!("Failed to save tag: {e}"), 3000);
                    editing.set(true);
                    return;
                }
            }

            // Success: update local state and exit editing mode.
            editing.set(false);
            state.has_unsaved_changes.set(false);
            for card in updated_cards {
                state.upsert_card(card);
            }
            state.tags.update(|tags| {
                if let Some(t) = tags.iter_mut().find(|t| t.id() == tag_id) {
                    *t = new_tag.clone();
                }
            });
        });
    };

    // Reactive preview of cards that would need updating under the
    // current implies edits — recomputes whenever implies_input changes.
    let affected_preview: Memo<Vec<(Uuid, Vec<Uuid>)>> = Memo::new(move |_| {
        if !editing.get() {
            return Vec::new();
        }
        let tag_id = match state.selected_card().get() {
            Some(id) => id,
            None => return Vec::new(),
        };
        let current_tags = state.tags.get();
        let mut new_implies = implies_input.get();
        new_implies.sort();
        new_implies.dedup();

        let mut next_graph = TagGraph::from_tags(&current_tags);
        next_graph.upsert(tag_id, new_implies);

        if next_graph.detect_cycle().is_some() {
            return Vec::new();
        }

        let current_cards = state.cards.get();
        affected_cards_for_change(&next_graph, &current_cards)
    });

    let save_changes = move || {
        let tag_id = match state.selected_card().get_untracked() {
            Some(id) => id,
            None => return,
        };
        let new_title = title_input.get_untracked();
        if new_title.trim().is_empty() {
            return;
        }
        let new_color = if use_color.get_untracked() {
            parse_hex_color(&color_input.get_untracked())
        } else {
            None
        };
        let mut new_implies = implies_input.get_untracked();
        new_implies.sort();
        new_implies.dedup();

        // Local cycle check before sending anything to the server — this
        // gives immediate feedback without a round-trip.
        let current_tags = state.tags.get_untracked();
        let mut next_graph = TagGraph::from_tags(&current_tags);
        next_graph.upsert(tag_id, new_implies.clone());
        if next_graph.detect_cycle().is_some() {
            show_error_toast(
                state,
                "Tag implication cycle detected — please fix before saving",
                3500,
            );
            return;
        }

        // Compute which cards need new versions under the new graph.
        let current_cards = state.cards.get_untracked();
        let affected = affected_cards_for_change(&next_graph, &current_cards);

        commit_tag_update(new_title, new_color, new_implies, affected);
    };

    let deleting = RwSignal::new(false);

    let do_delete = move || {
        let tag_id = match state.selected_card().get_untracked() {
            Some(id) => id,
            None => return,
        };
        deleting.set(true);
        let state = state;
        leptos::task::spawn_local(async move {
            let client = match get_client() {
                Some(c) => c,
                None => {
                    show_error_toast(state, "Can't delete tags while offline", 3000);
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

            match client.push_batch(items).await {
                Ok(_) => {}
                Err(ClientError::ConnectionLost) => {
                    show_error_toast(state, "Can't delete tags while offline", 3000);
                    confirm_delete.set(0);
                    deleting.set(false);
                    return;
                }
                Err(e) => {
                    tracing::error!(%e, "Failed to delete tag");
                    show_error_toast(state, &format!("Failed to delete tag: {e}"), 3000);
                    confirm_delete.set(0);
                    deleting.set(false);
                    return;
                }
            }

            // Update local state for affected cards
            for card in &affected {
                state.upsert_card(card.clone());
            }
            state.tags.update(|tags| tags.retain(|t| t.id() != tag_id));
            state
                .tag_filter
                .update(|tags| tags.retain(|t| *t != tag_id));
            set_selection(&state, None);
            deleting.set(false);
        });
    };

    // Render a labelled chip list of related tags (clickable to navigate,
    // but inert while editing). Shared by the read-only "Transitively
    // implies" and "Implied by" sections, which differ only by label and
    // source list.
    let chip_section = move |label: &'static str,
                             items: Vec<(Uuid, String, Option<rgb::RGB8>)>|
          -> AnyView {
        let editing_now = editing.get();
        let chips: Vec<_> = items
            .into_iter()
            .map(|(id, title, color)| {
                let style = tag_chip_style(&color);
                let on_click = move |_| {
                    if editing.get_untracked() {
                        return;
                    }
                    set_selection(&state, Some(id));
                };
                view! { <span class="tag-chip detail-tag-chip-link" style=style on:click=on_click>{title}</span> }
            })
            .collect();
        let container_class = if editing_now {
            "tag-implies-row"
        } else {
            "tag-implies-row detail-tag-chips"
        };
        view! {
            <div class="tag-color-section">
                <span class="tag-color-label">{label}</span>
            </div>
            <div class=container_class>
                {if chips.is_empty() {
                    view! { <span class="tag-color-hex due-not-set">"None"</span> }.into_any()
                } else {
                    chips.into_any()
                }}
            </div>
        }
        .into_any()
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
            let tag_id = state.selected_card().get()?;
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
        <div class="detail-section">
        {move || {
            let tag_id = state.selected_card().get()?;
            let editing_now = editing.get();

            if editing_now {
                Some(view! {
                    <TagColorPicker color_input=color_input use_color=use_color />
                }.into_any())
            } else {
                let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
                let current_color = tag.color().map(|c| blazelist_client_lib::color::format_tag_hex(&c));

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
        </div>

        // Implies section — direct implies only (editable in edit mode).
        <div class="detail-section">
        {move || {
            let tag_id = state.selected_card().get()?;
            let editing_now = editing.get();
            let all_tags = state.tags.get();

            if editing_now {
                let current = implies_input.get();
                let prev_graph = TagGraph::from_tags(&all_tags);
                let mut candidates: Vec<(Uuid, String)> = all_tags
                    .iter()
                    .filter(|t| t.id() != tag_id && !current.contains(&t.id()))
                    .filter(|t| {
                        let reachable_from_candidate = prev_graph.closure_of(&[t.id()]);
                        !reachable_from_candidate.contains(&tag_id)
                    })
                    .map(|t| (t.id(), t.title().to_string()))
                    .collect();
                candidates.sort_by_key(|a| a.1.to_lowercase());

                let on_add = move |ev: web_sys::Event| {
                    let val = event_target_value(&ev);
                    if val.is_empty() {
                        return;
                    }
                    if let Ok(new_id) = val.parse::<Uuid>() {
                        implies_input.update(|v| {
                            if !v.contains(&new_id) {
                                v.push(new_id);
                            }
                        });
                    }
                    // Reset dropdown back to placeholder.
                    if let Some(select) = ev.target().and_then(|t| t.dyn_ref::<web_sys::HtmlSelectElement>().cloned()) {
                        select.set_selected_index(0);
                    }
                };

                Some(view! {
                    <div class="tag-color-section">
                        <span class="tag-color-label">"\u{2192} Implies"</span>
                    </div>
                    <div class="tag-implies-row">
                        {move || {
                            let all = all_tags.clone();
                            let mut items: Vec<_> = implies_input.get().into_iter().map(|imp_id| {
                                let title = all.iter()
                                    .find(|t| t.id() == imp_id)
                                    .map(|t| t.title().to_string())
                                    .unwrap_or_else(|| "unknown".to_string());
                                (imp_id, title)
                            }).collect();
                            items.sort_by_key(|a| a.1.to_lowercase());
                            items.into_iter().map(|(imp_id, title)| {
                                let color = all.iter()
                                    .find(|t| t.id() == imp_id)
                                    .and_then(|t| t.color());
                                let style = tag_chip_style(&color);
                                let remove = move |_| {
                                    implies_input.update(|v| v.retain(|x| *x != imp_id));
                                };
                                view! {
                                    <span class="tag-chip" style=style>
                                        {title}
                                        <button class="chip-remove" on:click=remove>"x"</button>
                                    </span>
                                }
                            }).collect::<Vec<_>>()
                        }}
                    </div>
                    <div class="tag-implies-add">
                        <select class="settings-select" on:change=on_add>
                            <option value="">"+ Add implies\u{2026}"</option>
                            {candidates.into_iter().map(|(id, title)| {
                                let val = id.to_string();
                                view! { <option value=val>{title}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                    </div>
                }.into_any())
            } else {
                // Read mode: direct implies only. Renders via the shared
                // `chip_section` like the transitive / implied-by sections
                // below (this arm only exists while `editing` is false, so
                // the helper's editing-aware bits are inert here).
                let tag = all_tags.iter().find(|t| t.id() == tag_id)?;
                let mut direct: Vec<_> = tag.implies().iter().filter_map(|id| {
                    let t = all_tags.iter().find(|t| t.id() == *id)?;
                    Some((*id, t.title().to_string(), t.color()))
                }).collect();
                direct.sort_by_key(|a| a.1.to_lowercase());
                Some(chip_section("\u{2192} Implies", direct))
            }
        }}
        </div>

        // Also-implies section — transitive (inherited) implies,
        // read-only, visible in both read and edit mode.
        <div class="detail-section">
        {move || {
            let tag_id = state.selected_card().get()?;
            let all_tags = state.tags.get();
            let tag = all_tags.iter().find(|t| t.id() == tag_id)?;
            let direct_ids: std::collections::HashSet<Uuid> =
                tag.implies().iter().copied().collect();
            let graph = TagGraph::from_tags(&all_tags);
            let full_closure = graph.closure_of(&[tag_id]);
            let mut transitive: Vec<_> = full_closure
                .iter()
                .filter(|id| **id != tag_id && !direct_ids.contains(id))
                .filter_map(|id| {
                    let t = all_tags.iter().find(|t| t.id() == *id)?;
                    Some((*id, t.title().to_string(), t.color()))
                })
                .collect();
            transitive.sort_by_key(|a| a.1.to_lowercase());
            Some(chip_section("\u{2192} Transitively implies", transitive))
        }}
        </div>

        // Implied-by section — read-only, visible in both modes.
        <div class="detail-section">
        {move || {
            let tag_id = state.selected_card().get()?;
            let all_tags = state.tags.get();
            let graph = TagGraph::from_tags(&all_tags);
            let mut parents: Vec<_> = all_tags
                .iter()
                .filter(|t| t.id() != tag_id && graph.closure_of(&[t.id()]).contains(&tag_id))
                .map(|t| (t.id(), t.title().to_string(), t.color()))
                .collect();
            parents.sort_by_key(|a| a.1.to_lowercase());
            Some(chip_section("\u{2190} Implied by", parents))
        }}
        </div>

        // Inline affected-cards preview (shown while editing implies).
        {move || {
            if !editing.get() {
                return None;
            }
            let affected = affected_preview.get();
            if affected.is_empty() {
                return None;
            }
            let cards = state.cards.get();
            let all_tags = state.tags.get();
            let count = affected.len();
            let items: Vec<_> = affected.into_iter().filter_map(|(card_id, missing)| {
                let card = cards.iter().find(|c| c.id() == card_id)?;
                let preview = blazelist_client_lib::display::card_preview(card.content(), 40)
                    .unwrap_or_else(|| "Untitled".to_string());
                let full_title = blazelist_client_lib::display::card_preview(card.content(), 200)
                    .unwrap_or_else(|| "Untitled".to_string());
                let tag_chips: Vec<_> = missing.iter().filter_map(|tid| {
                    let tag = all_tags.iter().find(|t| t.id() == *tid)?;
                    Some((tag.title().to_string(), tag.color()))
                }).collect();
                Some((preview, full_title, tag_chips))
            }).collect();

            Some(view! {
                <div class="detail-section">
                    <div class="tag-color-section">
                        <span class="tag-color-label affected-label">
                            {format!("Saving will update {} card{}", count, if count == 1 { "" } else { "s" })}
                        </span>
                    </div>
                    <div class="affected-cards-preview">
                        {items.into_iter().map(|(preview, full_title, tags)| {
                            view! {
                                <div class="affected-card-row">
                                    <span class="affected-card-title" title=full_title>{preview}</span>
                                    <span class="affected-card-tags">
                                        {tags.into_iter().map(|(name, color)| {
                                            let style = tag_chip_style(&color);
                                            view! { <span class="tag-chip affected-tag-chip" style=style>{"+"}{name}</span> }
                                        }).collect::<Vec<_>>()}
                                    </span>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            })
        }}

        // Actions
        <div class="detail-section">
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
                    if confirm_delete.get() > 0 {
                        let tag_title = move || {
                            let tag_id = state.selected_card().get_untracked();
                            let title = tag_id
                                .and_then(|id| state.tags.get_untracked().into_iter().find(|t| t.id() == id))
                                .map(|t| t.title().to_string())
                                .unwrap_or_else(|| "Unknown".to_string());
                            format!("Tag: {title}")
                        };
                        return view! {
                            <ConfirmDeletePrompt
                                step=confirm_delete
                                first_prompt=|| "Delete tag?".to_string()
                                entity_label=tag_title
                                on_confirm=do_delete
                                on_cancel=move || confirm_delete.set(0)
                            />
                        }.into_any();
                    }
                    view! {
                        <button class="btn-save" on:click=move |_| start_editing()>"Edit"</button>
                        <button class="btn-delete" on:click=move |_| confirm_delete.set(1)>"Delete"</button>
                    }.into_any()
                }}
            </div>
        </div>
        </div>

        // Metadata
        <div class="detail-section">
        {move || {
            let tag_id = state.selected_card().get()?;
            let tag = state.tags.get().into_iter().find(|t| t.id() == tag_id)?;
            let id_str = tag_id.to_string();
            let created = tag.created_at();
            let modified = tag.modified_at();
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
                        <Timestamp datetime=created class="meta-value" />
                    </div>
                    <div class="meta-row">
                        <span class="meta-label">"Modified"</span>
                        <Timestamp datetime=modified class="meta-value" />
                    </div>
                </div>
            })
        }}
        </div>

        // Version history
        {move || {
            let tag_id = state.selected_card().get()?;
            Some(view! { <TagVersionHistory tag_id=tag_id /> })
        }}
    }
}

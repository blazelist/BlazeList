use crate::components::hooks::{handle_code_copy_click, use_click_outside_close};
use crate::state::store::{
    AppState, DueDatePreset, DueDateStatus, NewCardPosition, due_date_status,
    format_due_date_badge, get_client, open_editor, set_selection, tag_chip_style,
};
use crate::state::sync::push_card_or_queue;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, place_card,
};
use blazelist_client_lib::tag_graph::TagGraph;
use blazelist_protocol::{Card, Entity, PushItem, Utc};
use chrono::DateTime;
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;

/// Remove `tag_id` and any tag that transitively requires it, then re-add
/// anything still implied by the remainder. `graph` must be built from the
/// global `state.tags` (not from the selected set) so the closure reflects
/// the full implication structure.
fn remove_editor_tag_cascade(graph: &TagGraph, selected_tags: RwSignal<Vec<Uuid>>, tag_id: Uuid) {
    selected_tags.update(|tags| {
        tags.retain(|t| *t != tag_id);
        tags.retain(|t| !graph.closure_of(&[*t]).contains(&tag_id));
        let to_add = graph.closure_of(tags);
        for id in to_add {
            if !tags.contains(&id) {
                tags.push(id);
            }
        }
    });
}

/// Toggle `tag_id` in the editor's selected-tag set, cascading implications
/// in both directions: deselecting a tag also drops anything that requires
/// it (then re-closes the remainder), while selecting adds the tag's full
/// transitive closure. Rebuilds the implication graph from `state.tags`
/// (NOT `selected_tags`) on every call, then optionally clears the tag
/// search box per the `clear_tag_search` setting.
fn toggle_editor_tag(
    state: &AppState,
    selected_tags: RwSignal<Vec<Uuid>>,
    tag_search: RwSignal<String>,
    tag_id: Uuid,
) {
    // Build a graph from the current tag set so we can run closure_of for
    // cascade-add / block-remove.
    let graph = TagGraph::from_tags(&state.tags.get_untracked());
    if selected_tags.get_untracked().contains(&tag_id) {
        remove_editor_tag_cascade(&graph, selected_tags, tag_id);
    } else {
        // Cascade-add: add the clicked tag plus its entire transitive closure.
        let to_add = graph.closure_of(&[tag_id]);
        selected_tags.update(|tags| {
            for new_t in to_add {
                if !tags.contains(&new_t) {
                    tags.push(new_t);
                }
            }
        });
    }
    if state.clear_tag_search.get_untracked() {
        tag_search.set(String::new());
    }
}

#[component]
pub fn CardEditor(
    #[prop(into)] on_save: Callback<()>,
    #[prop(optional)] editing_card: Option<Card>,
    #[prop(optional, into)] on_cancel: Option<Callback<()>>,
) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let is_editing = editing_card.is_some();
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();

    Effect::new(move |_| {
        if let Some(el) = textarea_ref.get() {
            let _ = el.focus();
        }
    });

    // For a new card, consume any stored prefill (e.g. from "New from this").
    let prefill = (!is_editing)
        .then(|| state.new_card_prefill.get_untracked())
        .flatten();
    let is_prefill = prefill.is_some();

    let initial_content = editing_card
        .as_ref()
        .map(|c| c.content().to_string())
        .or_else(|| prefill.as_ref().map(|p| p.content.clone()))
        .unwrap_or_default();
    let initial_tags = editing_card
        .as_ref()
        .map(|c| c.tags().to_vec())
        .or_else(|| prefill.as_ref().map(|p| p.tags.clone()))
        .unwrap_or_default();
    let initial_due_date = editing_card
        .as_ref()
        .and_then(|c| c.due_date())
        .or_else(|| prefill.as_ref().and_then(|p| p.due_date));

    let content = RwSignal::new(initial_content.clone());
    let selected_tags = RwSignal::new(initial_tags.clone());
    let due_date: RwSignal<Option<DateTime<Utc>>> = RwSignal::new(initial_due_date);
    let due_preset = RwSignal::new(DueDatePreset::Today);
    let due_dropdown_open = RwSignal::new(false);
    let show_preview = RwSignal::new(state.show_preview.get_untracked());
    let tag_search = RwSignal::new(String::new());

    // Track the baseline snapshot for dirty detection.
    //
    // When the editor mounts with a prefill (e.g. "New from this"), the
    // baseline is an EMPTY card so the prefilled content/tags/due-date
    // immediately flag the editor as unsaved — the user sees the
    // "(unsaved)" indicator from the moment they land in the editor,
    // and can't accidentally dismiss it thinking everything was saved.
    // A prefilled new card is conceptually "blank card + inserted
    // content", and the dirty tracking now reflects that.
    let orig_content = RwSignal::new(if is_prefill {
        String::new()
    } else {
        initial_content
    });
    let orig_tags = RwSignal::new(if is_prefill {
        Vec::new()
    } else {
        let mut t = initial_tags;
        t.sort();
        t
    });
    let orig_due_date = RwSignal::new(if is_prefill { None } else { initial_due_date });

    // Track dirty state
    Effect::new(move |_| {
        let cur = content.get();
        let mut cur_tags = selected_tags.get();
        cur_tags.sort();
        let cur_due = due_date.get();
        let dirty = cur != orig_content.get()
            || cur_tags != orig_tags.get()
            || cur_due != orig_due_date.get();
        state.has_unsaved_changes.set(dirty);
    });
    on_cleanup(move || {
        state.has_unsaved_changes.set(false);
        // Prefill is single-shot — clear it when the editor unmounts so
        // subsequent "new card" sessions start blank.
        state.new_card_prefill.set(None);
    });

    let stored_editing = StoredValue::new(editing_card.clone());

    // --- Save handler (manual save / new card) ---
    // Works offline: creates the card locally and queues the push for when
    // connectivity is restored. `stay == true` keeps the editor open with
    // the saved card as the new baseline; new-card flows additionally
    // transition into the editing flow for the just-created card.
    let do_save = move |stay: bool| {
        let state = state;
        let on_save = on_save;
        let text = content.get_untracked();
        let selected_due = due_date.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        // Ensure tag set is closed under implications before saving.
        let graph = TagGraph::from_tags(&state.tags.get_untracked());
        let mut sorted_tags: Vec<Uuid> = graph
            .closure_of(&selected_tags.get_untracked())
            .into_iter()
            .collect();
        sorted_tags.sort();

        let editing = stored_editing.get_value();
        // Skip saving when nothing actually changed — otherwise we'd
        // create a new version with only `modified_at` differing.
        if let Some(existing) = &editing
            && text == existing.content()
            && sorted_tags == existing.tags()
            && selected_due == existing.due_date()
        {
            if !stay {
                on_save.run(());
            }
            return;
        }
        leptos::task::spawn_local(async move {
            let card = if let Some(existing) = editing {
                // Skip the auto-extinguish path when only content/tags changed.
                let new_blazed = if selected_due != existing.due_date() {
                    state.blazed_after_due_change(existing.blazed(), selected_due)
                } else {
                    existing.blazed()
                };
                existing.next(
                    text,
                    existing.priority(),
                    sorted_tags,
                    new_blazed,
                    Utc::now(),
                    selected_due,
                )
            } else {
                let mut cards = state.cards.get_untracked();
                blazelist_client_lib::filter::sort_by_priority(&mut cards);
                let position = state.new_card_position.get_untracked();
                let insert_pos = match position {
                    NewCardPosition::Top => InsertPosition::Top,
                    NewCardPosition::Bottom => InsertPosition::Bottom,
                    NewCardPosition::Above(ref_id) => {
                        match cards.iter().position(|c| c.id() == ref_id) {
                            Some(idx) => InsertPosition::At(idx),
                            None => InsertPosition::Bottom,
                        }
                    }
                    NewCardPosition::Below(ref_id) => {
                        match cards.iter().position(|c| c.id() == ref_id) {
                            Some(idx) => InsertPosition::At(idx + 1),
                            None => InsertPosition::Bottom,
                        }
                    }
                };
                let placement = place_card(&cards, insert_pos);
                match placement {
                    Placement::Simple(priority) => Card::first(
                        Uuid::new_v4(),
                        text.clone(),
                        priority,
                        sorted_tags,
                        false,
                        Utc::now(),
                        selected_due,
                    ),
                    Placement::Rebalanced { priority, shifted } => {
                        let card = Card::first(
                            Uuid::new_v4(),
                            text.clone(),
                            priority,
                            sorted_tags,
                            false,
                            Utc::now(),
                            selected_due,
                        );
                        // Update local state optimistically.
                        let shifted_cards = build_shifted_versions(&shifted, &cards);
                        for sc in &shifted_cards {
                            state.upsert_card(sc.clone());
                        }
                        let new_id = card.id();
                        state.upsert_card(card.clone());
                        // Refresh the dirty-detection snapshot and clear
                        // `has_unsaved_changes` synchronously — Leptos
                        // Effects are async-scheduled, so without the
                        // explicit clear the next `set_selection` /
                        // `close_editor` would prompt to discard.
                        stored_editing.set_value(Some(card.clone()));
                        orig_content.set(card.content().to_string());
                        orig_tags.set(card.tags().to_vec());
                        orig_due_date.set(card.due_date());
                        state.has_unsaved_changes.set(false);
                        set_selection(&state, Some(new_id));
                        // Push (or queue) all shifted + new card.
                        if let Some(client) = get_client() {
                            let mut items: Vec<PushItem> = shifted_cards
                                .into_iter()
                                .map(|c| PushItem::Cards(vec![c]))
                                .collect();
                            items.push(PushItem::Cards(vec![card.clone()]));
                            if let Err(e) = client.push_batch(items).await {
                                tracing::warn!(%e, "Batch push failed, queuing");
                                push_card_or_queue(&state, card.clone()).await;
                            }
                        } else {
                            push_card_or_queue(&state, card.clone()).await;
                        }
                        if stay {
                            // Baseline already refreshed above; for new-card
                            // flows flip back into editing for the just-created
                            // card (set_selection cleared `editing`).
                            if !is_editing {
                                open_editor(&state);
                            }
                        } else {
                            on_save.run(());
                        }
                        return;
                    }
                }
            };
            let new_id = card.id();
            state.upsert_card(card.clone());
            // Post-save dirty-flag dance: clear `has_unsaved_changes`
            // synchronously because Leptos Effects are async-scheduled
            // (see the `Rebalanced` match arm above for full detail).
            stored_editing.set_value(Some(card.clone()));
            orig_content.set(card.content().to_string());
            orig_tags.set(card.tags().to_vec());
            orig_due_date.set(card.due_date());
            state.has_unsaved_changes.set(false);
            if !is_editing {
                set_selection(&state, Some(new_id));
            }
            if stay {
                if !is_editing {
                    open_editor(&state);
                }
            } else {
                on_save.run(());
            }
            push_card_or_queue(&state, card).await;
        });
    };

    let on_submit = move |_| do_save(false);
    let on_submit_stay = move |_| do_save(true);

    // Ctrl+S, surfaced by the global keyboard handler.
    Effect::new(move |_| {
        if state.save_requested.get() {
            state.save_requested.set(false);
            do_save(true);
        }
    });

    let due_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(due_dropdown_open, due_group_ref);

    // Filtered and sorted tag list, recomputed reactively whenever the tag
    // search query or the global tag list changes.  Both the rendered list and
    // the keyboard shortcut handler use this memo so the filtering logic is
    // defined in exactly one place.
    let filtered_editor_tags = Memo::new(move |_| {
        let q = tag_search.get().to_lowercase();
        let mut tags = state.tags.get();
        tags.sort_by_key(|a| a.title().to_lowercase());
        if !q.is_empty() {
            tags.retain(|t| t.title().to_lowercase().contains(&q));
        }
        tags
    });

    let preview_html = move || {
        let text = content.get();
        let html =
            comrak::markdown_to_html(&text, &blazelist_client_lib::display::markdown_options());
        blazelist_client_lib::display::wrap_code_blocks_with_copy_button(&html)
    };

    let editor_body_class = move || {
        if show_preview.get() {
            "editor-body"
        } else {
            "editor-body no-preview"
        }
    };

    let main_row_class = move || {
        if show_preview.get() {
            "editor-main-row preview-active"
        } else {
            "editor-main-row"
        }
    };

    let card_editor_class = move || {
        if show_preview.get() {
            "card-editor"
        } else {
            "card-editor side-by-side"
        }
    };

    let on_cancel_clone = on_cancel;

    view! {
        <div class=card_editor_class>
            <div class=main_row_class>
                <div class="editor-left">
                    <div class="editor-toolbar">
                        <label class="preview-toggle">
                            "Preview"
                            <input
                                type="checkbox"
                                class="toggle-checkbox"
                                prop:checked=move || show_preview.get()
                                on:change=move |_| show_preview.update(|v| *v = !*v)
                            />
                        </label>
                    </div>
                    <div class="editor-body-row">
                        <div class=editor_body_class>
                            <textarea
                                class="editor-input"
                                placeholder="Write markdown content..."
                                prop:value=move || content.get()
                                on:input=move |ev| content.set(event_target_value(&ev))
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                                        ev.prevent_default();
                                        on_submit(ev.unchecked_into());
                                    }
                                }
                                node_ref=textarea_ref
                            />
                            {move || show_preview.get().then(|| view! {
                                <div class="editor-preview" inner_html=preview_html on:click=move |ev: web_sys::MouseEvent| {
                                    handle_code_copy_click(&ev);
                                }></div>
                            })}
                        </div>
                        <div class="editor-tags">
                            <span class="meta-label">"Tags"</span>
                            <input
                                class="tag-search-input"
                                type="text"
                                placeholder="Search tags\u{2026}"
                                prop:value=move || tag_search.get()
                                on:input=move |ev| tag_search.set(event_target_value(&ev))
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                                        // Ctrl/Cmd+Enter saves the card
                                        ev.prevent_default();
                                        on_submit(ev.unchecked_into());
                                        return;
                                    }
                                    // Enter (without Ctrl/Cmd) toggles the first visible tag
                                    // so users can add tags without leaving the keyboard.
                                    if ev.key() == "Enter" {
                                        ev.prevent_default();
                                        let tags = filtered_editor_tags.get_untracked();
                                        if let Some(first_tag) = tags.first() {
                                            let tag_id = first_tag.id();
                                            toggle_editor_tag(&state, selected_tags, tag_search, tag_id);
                                        }
                                    }
                                }
                            />
                            <ul class="editor-tag-list">
                            {move || {
                                filtered_editor_tags.get().into_iter().map(|tag| {
                                    let tag_id = tag.id();
                                    let title = tag.title().to_string();
                                    let color = tag.color().map(|c| blazelist_client_lib::color::format_tag_hex(&c));
                                    let is_selected = move || selected_tags.get().contains(&tag_id);
                                    let toggle = move |_| {
                                        toggle_editor_tag(&state, selected_tags, tag_search, tag_id);
                                    };
                                    let item_class = move || if is_selected() { "editor-tag-item active" } else { "editor-tag-item" };
                                    let border_style = color
                                        .map(|c| format!("border-left: 3px solid {c};"))
                                        .unwrap_or_else(|| "border-left: 3px solid transparent;".to_string());
                                    view! {
                                        <li class=item_class style=border_style on:click=toggle>
                                            <span class="editor-tag-name">{title}</span>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()
                            }}
                            </ul>
                        </div>
                    </div>
                    // Selected tags shown below editor
                    {move || {
                        let sel = selected_tags.get();
                        let all_tags = state.tags.get();
                        if sel.is_empty() {
                            return None;
                        }
                        let chips: Vec<_> = sel.iter().filter_map(|id| {
                            let tag = all_tags.iter().find(|t| t.id() == *id)?;
                            let tag_id = *id;
                            let title = tag.title().to_string();
                            let color = tag.color();
                            let style = tag_chip_style(&color);
                            let remove = move |ev: web_sys::MouseEvent| {
                                ev.stop_propagation();
                                let graph = TagGraph::from_tags(&state.tags.get_untracked());
                                remove_editor_tag_cascade(&graph, selected_tags, tag_id);
                            };
                            Some(view! {
                                <span class="tag-chip" style=style>
                                    {title}
                                    <button class="chip-remove" on:click=remove>"x"</button>
                                </span>
                            })
                        }).collect();
                        Some(view! {
                            <div class="editor-selected-tags">{chips}</div>
                        })
                    }}
                    <div class="detail-tags">
                        <span class="meta-label">"Due date"</span>
                        <div class="due-date-controls">
                            {move || {
                                due_date.get().map(|d| {
                                    let (badge_text, badge_class) = format_due_date_badge(&d);
                                    let cls = format!("due-date-current {badge_class}");
                                    let date_str = d.format("%Y-%m-%d").to_string();
                                    view! {
                                        <span class=cls>{format!("{date_str} ({badge_text})")}</span>
                                    }
                                })
                            }}
                            <div class="due-date-dropdown-group" node_ref=due_group_ref>
                                <button class="due-date-quick-btn" on:click=move |_| {
                                    let smart = match due_date.get_untracked() {
                                        Some(d) if matches!(due_date_status(&d), DueDateStatus::Today) => DueDatePreset::Tomorrow,
                                        _ => DueDatePreset::Today,
                                    };
                                    due_preset.set(smart);
                                    due_date.set(Some(smart.resolve()));
                                }>{move || {
                                    match due_date.get() {
                                        Some(d) if matches!(due_date_status(&d), DueDateStatus::Today) => DueDatePreset::Tomorrow.label(),
                                        _ => DueDatePreset::Today.label(),
                                    }
                                }}</button>
                                <button class="due-date-dropdown-toggle" on:click=move |ev: web_sys::MouseEvent| {
                                    ev.stop_propagation();
                                    due_dropdown_open.update(|v| *v = !*v);
                                }>
                                    {move || if due_dropdown_open.get() { "\u{25B4}" } else { "\u{25BE}" }}
                                </button>
                                {move || due_dropdown_open.get().then(|| view! {
                                    <div class="due-date-dropdown-menu">
                                        {DueDatePreset::ALL.into_iter().map(|p| {
                                            view! {
                                                <button
                                                    class="save-dropdown-item"
                                                    class:active=move || due_preset.get() == p
                                                    on:click=move |_| {
                                                        due_preset.set(p);
                                                        due_date.set(Some(p.resolve()));
                                                        due_dropdown_open.set(false);
                                                    }
                                                >
                                                    {p.label()}
                                                </button>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                })}
                            </div>
                            <input
                                class="due-date-picker"
                                type="date"
                                prop:value=move || due_date.get().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    if val.is_empty() {
                                        due_date.set(None);
                                    } else if let Ok(date) = chrono::NaiveDate::parse_from_str(&val, "%Y-%m-%d") {
                                        due_date.set(Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc()));
                                    }
                                }
                            />
                            {move || due_date.get().map(|_| view! {
                                <button class="due-date-clear-btn" on:click=move |_| due_date.set(None)>"Clear"</button>
                            })}
                        </div>
                    </div>
                </div>
            </div>
            <div class="editor-actions">
                {on_cancel_clone.map(|cb| {
                    view! {
                        <button class="btn-cancel" on:click=move |_| cb.run(())>"Cancel"</button>
                    }
                })}
                <button class="btn-save-stay" on:click=on_submit_stay title="Save and keep editing (Ctrl+S)">"Save"</button>
                <button class="btn-save" on:click=on_submit>"Save & Close"</button>
            </div>
        </div>
    }
}

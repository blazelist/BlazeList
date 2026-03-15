use crate::components::hooks::use_click_outside_close;
use crate::state::store::{
    AppState, AutoSaveStatus, DueDatePreset, NewCardPosition, format_due_date_badge, get_client,
    sync_query_params, tag_chip_style,
};
use crate::state::sync::push_card_or_queue;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, place_card,
};
use blazelist_protocol::{Card, Entity, PushItem, Utc};
use chrono::DateTime;
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setInterval")]
    fn set_interval_js(handler: &js_sys::Function, timeout: i32) -> i32;
    #[wasm_bindgen(js_name = "clearInterval")]
    fn clear_interval_js(handle: i32);
    #[wasm_bindgen(js_name = "setTimeout")]
    fn set_timeout_js(handler: &js_sys::Function, timeout: i32) -> i32;
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout_js(handle: i32);
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

    if !is_editing {
        Effect::new(move |_| {
            if let Some(el) = textarea_ref.get() {
                let _ = el.focus();
            }
        });
    }

    let initial_content = editing_card
        .as_ref()
        .map(|c| c.content().to_string())
        .unwrap_or_default();
    let initial_tags = editing_card
        .as_ref()
        .map(|c| c.tags().to_vec())
        .unwrap_or_default();
    let initial_due_date = editing_card.as_ref().and_then(|c| c.due_date());

    let content = RwSignal::new(initial_content.clone());
    let selected_tags = RwSignal::new(initial_tags.clone());
    let due_date: RwSignal<Option<DateTime<Utc>>> = RwSignal::new(initial_due_date);
    let due_preset = RwSignal::new(DueDatePreset::Today);
    let due_dropdown_open = RwSignal::new(false);
    let show_preview = RwSignal::new(state.show_preview.get_untracked());
    let tag_search = RwSignal::new(String::new());

    // Track the "last saved" snapshot so dirty detection resets after auto-save.
    let orig_content = RwSignal::new(initial_content);
    let orig_tags = RwSignal::new({
        let mut t = initial_tags;
        t.sort();
        t
    });
    let orig_due_date = RwSignal::new(initial_due_date);

    // Track dirty state
    Effect::new(move |_| {
        let cur = content.get();
        let mut cur_tags = selected_tags.get();
        cur_tags.sort();
        let cur_due = due_date.get();
        let dirty =
            cur != orig_content.get() || cur_tags != orig_tags.get() || cur_due != orig_due_date.get();
        state.has_unsaved_changes.set(dirty);
    });
    on_cleanup(move || {
        state.has_unsaved_changes.set(false);
    });

    let stored_editing = StoredValue::new(editing_card.clone());
    // Tracks whether a server-side card exists (true for edits, becomes
    // true after auto-save creates a new card).
    let card_created = RwSignal::new(is_editing);

    // --- Auto-save machinery ---
    let auto_save_status = state.auto_save_status;
    let interval_handle = RwSignal::new(0i32);
    let saved_timeout_handle = RwSignal::new(0i32);

    // Clear all auto-save timers.
    let clear_auto_save_timers = move || {
        let ih = interval_handle.get_untracked();
        if ih != 0 {
            clear_interval_js(ih);
            interval_handle.set(0);
        }
        let sh = saved_timeout_handle.get_untracked();
        if sh != 0 {
            clear_timeout_js(sh);
            saved_timeout_handle.set(0);
        }
    };

    // Auto-save: watch for dirty changes and manage countdown.
    {
        Effect::new(move |_| {
            let dirty = state.has_unsaved_changes.get();
            let auto_save = state.auto_save_enabled.get();
            let delay = state.auto_save_delay_secs.get();

            if !dirty || !auto_save {
                clear_auto_save_timers();
                if !dirty {
                    // Don't overwrite Saved status
                    if auto_save_status.get_untracked() != AutoSaveStatus::Saved {
                        auto_save_status.set(AutoSaveStatus::Idle);
                    }
                }
                return;
            }

            // Dirty and auto-save enabled: start countdown if not already running.
            // This avoids restarting the timer on every keystroke (debounce),
            // so the save fires promptly after the initial change.
            let current_status = auto_save_status.get_untracked();
            if matches!(current_status, AutoSaveStatus::Countdown(_) | AutoSaveStatus::Saving) {
                return;
            }
            clear_auto_save_timers();
            auto_save_status.set(AutoSaveStatus::Countdown(delay));

            let cb = Closure::<dyn Fn()>::new(move || {
                let current = auto_save_status.get_untracked();
                match current {
                    AutoSaveStatus::Countdown(1) => {
                        // Time's up — trigger auto-save.
                        let ih = interval_handle.get_untracked();
                        if ih != 0 {
                            clear_interval_js(ih);
                            interval_handle.set(0);
                        }
                        auto_save_status.set(AutoSaveStatus::Saving);

                        let text = content.get_untracked();
                        let tags = selected_tags.get_untracked();
                        let selected_due = due_date.get_untracked();
                        let editing = stored_editing.get_value();

                        if text.trim().is_empty() {
                            auto_save_status.set(AutoSaveStatus::Idle);
                            return;
                        }

                        if let Some(existing) = editing {
                            let card = existing.next(
                                text.clone(),
                                existing.priority(),
                                tags.clone(),
                                existing.blazed(),
                                Utc::now(),
                                selected_due,
                            );

                            // Optimistic local update.
                            state.upsert_card(card.clone());
                            leptos::task::spawn_local(async move {
                                push_card_or_queue(&state, card.clone()).await;
                                // Update base snapshot so dirty resets.
                                stored_editing.set_value(Some(card));
                                orig_content.set(text);
                                let mut sorted_tags = tags;
                                sorted_tags.sort();
                                orig_tags.set(sorted_tags);
                                orig_due_date.set(selected_due);

                                auto_save_status.set(AutoSaveStatus::Saved);
                                // Clear "Saved" after 2s.
                                let reset_cb = Closure::once(move || {
                                    if auto_save_status.get_untracked() == AutoSaveStatus::Saved
                                    {
                                        auto_save_status.set(AutoSaveStatus::Idle);
                                    }
                                    saved_timeout_handle.set(0);
                                });
                                let func = reset_cb.into_js_value();
                                let h = set_timeout_js(func.unchecked_ref(), 2000);
                                saved_timeout_handle.set(h);
                            });
                        } else {
                            // New card auto-save: create the card and push.
                            let new_id = Uuid::new_v4();
                            let mut cards_for_placement = state.cards.get_untracked();
                            blazelist_client_lib::filter::sort_by_priority(&mut cards_for_placement);
                            let position = state.new_card_position.get_untracked();
                            let insert_pos = match position {
                                NewCardPosition::Top => InsertPosition::Top,
                                NewCardPosition::Bottom => InsertPosition::Bottom,
                                NewCardPosition::Above(ref_id) => {
                                    match cards_for_placement.iter().position(|c| c.id() == ref_id) {
                                        Some(idx) => InsertPosition::At(idx),
                                        None => InsertPosition::Bottom,
                                    }
                                }
                                NewCardPosition::Below(ref_id) => {
                                    match cards_for_placement.iter().position(|c| c.id() == ref_id) {
                                        Some(idx) => InsertPosition::At(idx + 1),
                                        None => InsertPosition::Bottom,
                                    }
                                }
                            };
                            let placement = place_card(&cards_for_placement, insert_pos);
                            let (priority, shifted) = match placement {
                                Placement::Simple(p) => (p, Vec::new()),
                                Placement::Rebalanced { priority, shifted } => (priority, shifted),
                            };
                            let card = Card::first(
                                new_id, text.clone(), priority, tags.clone(),
                                false, Utc::now(), selected_due,
                            );
                            let shifted_cards = if shifted.is_empty() {
                                Vec::new()
                            } else {
                                build_shifted_versions(&shifted, &cards_for_placement)
                            };
                            leptos::task::spawn_local(async move {
                                if let Some(client) = get_client() {
                                    if shifted_cards.is_empty() {
                                        if let Err(e) = client.push_card(card.clone()).await {
                                            log::error!("Auto-save new card failed: {e}");
                                            auto_save_status.set(AutoSaveStatus::Idle);
                                            return;
                                        }
                                    } else {
                                        let mut items: Vec<PushItem> = shifted_cards
                                            .into_iter()
                                            .map(|c| PushItem::Cards(vec![c]))
                                            .collect();
                                        items.push(PushItem::Cards(vec![card.clone()]));
                                        if let Err(e) = client.push_batch(items).await {
                                            log::error!("Auto-save new card failed: {e}");
                                            auto_save_status.set(AutoSaveStatus::Idle);
                                            return;
                                        }
                                    }
                                    state.upsert_card(card.clone());
                                    // Keep the editor alive — update internal state so
                                    // subsequent auto-saves use the existing-card path.
                                    stored_editing.set_value(Some(card));
                                    orig_content.set(text);
                                    let mut sorted_tags = tags;
                                    sorted_tags.sort();
                                    orig_tags.set(sorted_tags);
                                    orig_due_date.set(selected_due);
                                    card_created.set(true);
                                    auto_save_status.set(AutoSaveStatus::Saved);
                                    state.selected_card.set(Some(new_id));
                                    sync_query_params(&state);
                                    let reset_cb = Closure::once(move || {
                                        if auto_save_status.get_untracked() == AutoSaveStatus::Saved
                                        {
                                            auto_save_status.set(AutoSaveStatus::Idle);
                                        }
                                        saved_timeout_handle.set(0);
                                    });
                                    let func = reset_cb.into_js_value();
                                    let h = set_timeout_js(func.unchecked_ref(), 2000);
                                    saved_timeout_handle.set(h);
                                }
                            });
                        }
                    }
                    AutoSaveStatus::Countdown(n) if n > 1 => {
                        auto_save_status.set(AutoSaveStatus::Countdown(n - 1));
                    }
                    _ => {}
                }
            });
            let func = cb.into_js_value();
            let h = set_interval_js(func.unchecked_ref(), 1000);
            interval_handle.set(h);
        });
    }

    on_cleanup(move || {
        clear_auto_save_timers();
    });

    // --- Save handler (manual save / new card) ---
    // Works offline: creates the card locally and queues the push for when
    // connectivity is restored.
    let on_submit = move |_| {
        let state = state.clone();
        let on_save = on_save.clone();
        let text = content.get_untracked();
        let tags = selected_tags.get_untracked();
        let selected_due = due_date.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        // Stop any in-flight auto-save countdown.
        clear_auto_save_timers();
        auto_save_status.set(AutoSaveStatus::Idle);

        let editing = stored_editing.get_value();
        leptos::task::spawn_local(async move {
            let card = if let Some(existing) = editing {
                existing.next(
                    text,
                    existing.priority(),
                    tags,
                    existing.blazed(),
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
                        tags,
                        false,
                        Utc::now(),
                        selected_due,
                    ),
                    Placement::Rebalanced { priority, shifted } => {
                        let card = Card::first(
                            Uuid::new_v4(),
                            text.clone(),
                            priority,
                            tags,
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
                        state.selected_card.set(Some(new_id));
                        sync_query_params(&state);
                        // Push (or queue) all shifted + new card.
                        if let Some(client) = get_client() {
                            let mut items: Vec<PushItem> = shifted_cards
                                .into_iter()
                                .map(|c| PushItem::Cards(vec![c]))
                                .collect();
                            items.push(PushItem::Cards(vec![card.clone()]));
                            if let Err(e) = client.push_batch(items).await {
                                log::warn!("Batch push failed, queuing: {e}");
                                push_card_or_queue(&state, card).await;
                            }
                        } else {
                            push_card_or_queue(&state, card).await;
                        }
                        on_save.run(());
                        return;
                    }
                }
            };
            let new_id = card.id();
            state.upsert_card(card.clone());
            if !is_editing {
                state.selected_card.set(Some(new_id));
                sync_query_params(&state);
            }
            on_save.run(());
            push_card_or_queue(&state, card).await;
        });
    };

    let due_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(due_dropdown_open, due_group_ref);

    let preview_html = move || {
        let text = content.get();
        comrak::markdown_to_html(&text, &blazelist_client_lib::display::markdown_options())
    };

    let save_label = move || if card_created.get() { "Update" } else { "Save" };

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

    let auto_save_indicator = move || {
        if !state.auto_save_enabled.get() {
            return String::new();
        }
        match auto_save_status.get() {
            AutoSaveStatus::Idle => String::new(),
            AutoSaveStatus::Countdown(n) => format!("Auto-saving in {n}s\u{2026}"),
            AutoSaveStatus::Saving => "Saving\u{2026}".to_string(),
            AutoSaveStatus::Saved => "Saved".to_string(),
        }
    };

    let auto_save_class = move || match auto_save_status.get() {
        AutoSaveStatus::Saved => "auto-save-status auto-save-saved",
        AutoSaveStatus::Saving => "auto-save-status auto-save-saving",
        AutoSaveStatus::Countdown(_) => "auto-save-status auto-save-countdown",
        AutoSaveStatus::Idle => "auto-save-status",
    };

    let on_cancel_clone = on_cancel.clone();

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
                        <span class=auto_save_class>{auto_save_indicator}</span>
                    </div>
                    <div class="editor-body-row">
                        <div class=editor_body_class>
                            <textarea
                                class="editor-input"
                                placeholder="Write markdown content..."
                                prop:value=move || content.get()
                                on:input=move |ev| content.set(event_target_value(&ev))
                                node_ref=textarea_ref
                            />
                            {move || show_preview.get().then(|| view! {
                                <div class="editor-preview" inner_html=preview_html></div>
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
                            />
                            <ul class="editor-tag-list">
                            {move || {
                                let q = tag_search.get().to_lowercase();
                                let mut tags = state.tags.get();
                                tags.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));
                                if !q.is_empty() {
                                    tags.retain(|t| t.title().to_lowercase().contains(&q));
                                }
                                tags.into_iter().map(|tag| {
                                    let tag_id = tag.id();
                                    let title = tag.title().to_string();
                                    let color = tag.color().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                                    let is_selected = move || selected_tags.get().contains(&tag_id);
                                    let toggle = move |_| {
                                        selected_tags.update(|tags| {
                                            if tags.contains(&tag_id) {
                                                tags.retain(|t| *t != tag_id);
                                            } else {
                                                tags.push(tag_id);
                                            }
                                        });
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
                                selected_tags.update(|tags| tags.retain(|t| *t != tag_id));
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
                                    due_date.set(Some(due_preset.get_untracked().resolve()));
                                }>{move || due_preset.get().label()}</button>
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
                <button class="btn-save" on:click=on_submit>{save_label}</button>
            </div>
        </div>
    }
}

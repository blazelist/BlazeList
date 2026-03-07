use crate::components::hooks::use_click_outside_close;
use crate::state::store::{
    AppState, DueDatePreset, format_due_date_badge, get_client, tag_chip_style,
};
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::priority::{
    InsertPosition, Placement, build_shifted_versions, place_card,
};
use blazelist_protocol::{Card, Entity, PushItem, Utc};
use chrono::DateTime;
use leptos::prelude::*;
use uuid::Uuid;

/// Where a newly created card should be placed in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavePosition {
    Bottom,
    Top,
}

#[component]
pub fn CardEditor(
    #[prop(into)] on_save: Callback<()>,
    #[prop(optional)] editing_card: Option<Card>,
    #[prop(optional, into)] on_cancel: Option<Callback<()>>,
) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let is_editing = editing_card.is_some();

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
    let show_preview = RwSignal::new(true);
    let save_position = RwSignal::new(SavePosition::Bottom);
    let dropdown_open = RwSignal::new(false);

    // Track dirty state — compare current content/tags/due_date to initial values
    let orig_content = initial_content;
    let orig_tags = {
        let mut t = initial_tags;
        t.sort();
        t
    };
    let orig_due_date = initial_due_date;
    Effect::new(move |_| {
        let cur = content.get();
        let mut cur_tags = selected_tags.get();
        cur_tags.sort();
        let cur_due = due_date.get();
        let dirty = cur != orig_content || cur_tags != orig_tags || cur_due != orig_due_date;
        state.has_unsaved_changes.set(dirty);
    });
    on_cleanup(move || {
        state.has_unsaved_changes.set(false);
    });

    let stored_editing = StoredValue::new(editing_card.clone());
    let on_submit = move |_| {
        let state = state.clone();
        let on_save = on_save.clone();
        let text = content.get_untracked();
        let tags = selected_tags.get_untracked();
        let position = save_position.get_untracked();
        let selected_due = due_date.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        let editing = stored_editing.get_value();
        leptos::task::spawn_local(async move {
            if let Some(client) = get_client() {
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
                    let insert_pos = match position {
                        SavePosition::Top => InsertPosition::Top,
                        SavePosition::Bottom => InsertPosition::Bottom,
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
                            let shifted_cards = build_shifted_versions(&shifted, &cards);
                            let mut items: Vec<PushItem> = shifted_cards
                                .into_iter()
                                .map(|c| PushItem::Cards(vec![c]))
                                .collect();
                            items.push(PushItem::Cards(vec![card.clone()]));
                            if let Err(e) = client.push_batch(items).await {
                                log::error!("Failed to push rebalanced card: {e}");
                                return;
                            }
                            state.upsert_card(card);
                            on_save.run(());
                            return;
                        }
                    }
                };
                if let Err(e) = client.push_card(card.clone()).await {
                    log::error!("Failed to save card: {e}");
                    return;
                }
                state.upsert_card(card);
                on_save.run(());
            }
        });
    };

    let on_toggle_dropdown = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        dropdown_open.update(|v| *v = !*v);
    };

    let group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(dropdown_open, group_ref);

    let due_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(due_dropdown_open, due_group_ref);

    let preview_html = move || {
        let text = content.get();
        comrak::markdown_to_html(&text, &blazelist_client_lib::display::markdown_options())
    };

    let save_label = move || match save_position.get() {
        SavePosition::Bottom => {
            if is_editing {
                "Update"
            } else {
                "Save"
            }
        }
        SavePosition::Top => {
            if is_editing {
                "Update"
            } else {
                "Save (top)"
            }
        }
    };

    let editor_body_class = move || {
        if show_preview.get() {
            "editor-body"
        } else {
            "editor-body no-preview"
        }
    };

    let on_cancel_clone = on_cancel.clone();

    view! {
        <div class="card-editor">
            <div class="editor-toolbar">
                <label class="preview-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || show_preview.get()
                        on:change=move |_| show_preview.update(|v| *v = !*v)
                    />
                    "Preview"
                </label>
            </div>
            <div class=editor_body_class>
                <textarea
                    class="editor-input"
                    placeholder="Write markdown content..."
                    prop:value=move || content.get()
                    on:input=move |ev| content.set(event_target_value(&ev))
                />
                {move || show_preview.get().then(|| view! {
                    <div class="editor-preview" inner_html=preview_html></div>
                })}
            </div>
            <div class="editor-tags">
                {move || {
                    let mut tags = state.tags.get();
                    tags.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));
                    tags.into_iter().map(|tag| {
                        let tag_id = tag.id();
                        let title = tag.title().to_string();
                        let color = tag.color();
                        let selected = move || selected_tags.get().contains(&tag_id);
                        let toggle = move |_| {
                            selected_tags.update(|tags| {
                                if tags.contains(&tag_id) {
                                    tags.retain(|t| *t != tag_id);
                                } else {
                                    tags.push(tag_id);
                                }
                            });
                        };
                        let style = tag_chip_style(&color);
                        view! {
                            <span
                                class="tag-chip editor-tag-chip"
                                class:editor-tag-selected=selected
                                style=style.clone()
                                on:click=toggle
                            >
                                {title}
                            </span>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
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
            <div class="editor-actions">
                {on_cancel_clone.map(|cb| {
                    view! {
                        <button class="btn-cancel" on:click=move |_| cb.run(())>"Cancel"</button>
                    }
                })}
                {if is_editing {
                    view! {
                        <button class="btn-save" on:click=on_submit>"Update"</button>
                    }.into_any()
                } else {
                    view! {
                        <div class="btn-save-group" node_ref=group_ref>
                            <button class="btn-save" on:click=on_submit>{save_label}</button>
                            <button class="btn-save-dropdown" on:click=on_toggle_dropdown>
                                {move || if dropdown_open.get() { "\u{25B4}" } else { "\u{25BE}" }}
                            </button>
                            {move || dropdown_open.get().then(|| view! {
                                <div class="save-dropdown-menu">
                                    <button
                                        class="save-dropdown-item"
                                        class:active=move || save_position.get() == SavePosition::Top
                                        on:click=move |_| {
                                            save_position.set(SavePosition::Top);
                                            dropdown_open.set(false);
                                        }
                                    >
                                        "Add to top"
                                    </button>
                                    <button
                                        class="save-dropdown-item"
                                        class:active=move || save_position.get() == SavePosition::Bottom
                                        on:click=move |_| {
                                            save_position.set(SavePosition::Bottom);
                                            dropdown_open.set(false);
                                        }
                                    >
                                        "Add to bottom"
                                    </button>
                                </div>
                            })}
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

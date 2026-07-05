use crate::components::hooks::{handle_code_copy_click, toggle_expanded, use_click_outside_close};
use crate::components::timestamp::Timestamp;
use crate::state::pending_priority;
use crate::state::store::{
    AppState, NewCardPosition, NewCardPrefill, format_relative_time, get_client,
    set_selection_without_flush, start_new_card, tag_chip_style,
};
use crate::state::sync::push_card_or_queue;
use crate::storage;
use blazelist_client_lib::client::Client as _;
use blazelist_client_lib::display::card_preview;
use blazelist_protocol::{Card, Entity, Tag, Utc};
use leptos::prelude::*;
use uuid::Uuid;

fn render_markdown(content: &str) -> String {
    let html =
        comrak::markdown_to_html(content, &blazelist_client_lib::display::markdown_options());
    blazelist_client_lib::display::wrap_code_blocks_with_copy_button(&html)
}

/// Kinds of changes between two consecutive card versions. A single
/// `next()` call may produce more than one of these — we surface them
/// all so the history view can label and filter precisely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardChangeKind {
    Created,
    Content,
    Priority,
    Tags,
    Blazed,
    Extinguished,
    DueDate,
    /// New version with no tracked semantic change — only `modified_at` differs.
    Touched,
}

impl CardChangeKind {
    fn label(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Content => "Content",
            Self::Priority => "Priority",
            Self::Tags => "Tags",
            Self::Blazed => "Blazed",
            Self::Extinguished => "Extinguished",
            Self::DueDate => "Due",
            Self::Touched => "Touched",
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Self::Created => "history-kind-created",
            Self::Content => "history-kind-content",
            Self::Priority => "history-kind-priority",
            Self::Tags => "history-kind-tags",
            Self::Blazed => "history-kind-blazed",
            Self::Extinguished => "history-kind-extinguished",
            Self::DueDate => "history-kind-due",
            Self::Touched => "history-kind-touched",
        }
    }

    /// Stable order for filter chips and badge sequences.
    fn all() -> &'static [Self] {
        &[
            Self::Created,
            Self::Content,
            Self::Priority,
            Self::Tags,
            Self::Blazed,
            Self::Extinguished,
            Self::DueDate,
            Self::Touched,
        ]
    }
}

/// Compare two consecutive card versions and return the set of changes.
/// `prev` is `None` for the first version (which is treated as a creation).
fn card_changes(prev: Option<&Card>, curr: &Card) -> Vec<CardChangeKind> {
    let Some(prev) = prev else {
        return vec![CardChangeKind::Created];
    };
    let mut changes = Vec::new();
    if prev.content() != curr.content() {
        changes.push(CardChangeKind::Content);
    }
    if prev.priority() != curr.priority() {
        changes.push(CardChangeKind::Priority);
    }
    if prev.tags() != curr.tags() {
        changes.push(CardChangeKind::Tags);
    }
    if prev.blazed() != curr.blazed() {
        if curr.blazed() {
            changes.push(CardChangeKind::Blazed);
        } else {
            changes.push(CardChangeKind::Extinguished);
        }
    }
    if prev.due_date() != curr.due_date() {
        changes.push(CardChangeKind::DueDate);
    }
    if changes.is_empty() {
        changes.push(CardChangeKind::Touched);
    }
    changes
}

/// Kinds of changes between two consecutive tag versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagChangeKind {
    Created,
    Title,
    Color,
    Implies,
    /// Mirrors `CardChangeKind::Touched`.
    Touched,
}

impl TagChangeKind {
    fn label(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Title => "Title",
            Self::Color => "Color",
            Self::Implies => "Implies",
            Self::Touched => "Touched",
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Self::Created => "history-kind-created",
            Self::Title => "history-kind-title",
            Self::Color => "history-kind-color",
            Self::Implies => "history-kind-implies",
            Self::Touched => "history-kind-touched",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Created,
            Self::Title,
            Self::Color,
            Self::Implies,
            Self::Touched,
        ]
    }
}

fn tag_changes(prev: Option<&Tag>, curr: &Tag) -> Vec<TagChangeKind> {
    let Some(prev) = prev else {
        return vec![TagChangeKind::Created];
    };
    let mut changes = Vec::new();
    if prev.title() != curr.title() {
        changes.push(TagChangeKind::Title);
    }
    if prev.color() != curr.color() {
        changes.push(TagChangeKind::Color);
    }
    if prev.implies() != curr.implies() {
        changes.push(TagChangeKind::Implies);
    }
    if changes.is_empty() {
        changes.push(TagChangeKind::Touched);
    }
    changes
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
    // Filter chip selection. `None` = show every version.
    let filter: RwSignal<Option<CardChangeKind>> = RwSignal::new(None);

    // Dropdown state for the "New from this" group — matches the
    // main `+ New Card` button's dropdown. Position defaults to Top
    // when the user clicks the main button; the dropdown offers
    // top, bottom, above this card, and below this card.
    let fork_dropdown = RwSignal::new(false);
    let fork_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(fork_dropdown, fork_group_ref);

    // Fetch history on mount — show cached data first, then refresh from server.
    // Also re-trigger when connection status changes so that history is fetched
    // after the client connects (the card detail no longer re-creates this
    // component on sync, so the Effect must retry on its own).
    Effect::new(move |_| {
        let selected = state.selected_card().get(); // re-trigger on card change
        let _ = state.connection_status.get(); // re-trigger on connect

        // Only reset UI state when the selected card changes,
        // not on every connection status transition.
        if prev_card.get_untracked() != selected {
            error_msg.set(None);
            expanded.set(None);
            filter.set(None);
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
                        if history.is_empty() {
                            tracing::warn!(%card_id, "Server returned empty card history");
                        }
                        history.sort_by_key(|c| std::cmp::Reverse(c.count()));
                        versions.set(history.clone());
                        storage::update_cached_card_history(card_id, history);
                        storage::save_history_cache().await;
                    }
                    Err(e) => {
                        tracing::error!(%card_id, %e, "Failed to fetch card history");
                        if versions.get_untracked().is_empty() {
                            error_msg.set(Some(format!("Failed to load history: {e}")));
                        }
                    }
                }
                loading.set(false);
            }
            // If get_client() was None, keep loading=true so the
            // Effect retries when connection_status changes.
        });
    });

    let on_toggle_expand = move |count: i64| {
        toggle_expanded(expanded, count);
    };

    let on_restore = move |version: Card| {
        let state = state;
        leptos::task::spawn_local(async move {
            let Some(current) = state
                .cards
                .get_untracked()
                .into_iter()
                .find(|c| c.id() == card_id)
            else {
                return;
            };
            let restored = current.next(
                version.content().to_string(),
                current.priority(),
                version.tags().to_vec(),
                version.blazed(),
                Utc::now(),
                version.due_date(),
            );
            // Restore is just another card edit — queue it if offline
            // so the user's action survives restart and is pushed on
            // reconnect.
            state.upsert_card(restored.clone());
            // `CardDetail`'s outer closure reads `state.cards` via
            // `get_untracked()` for editor stability, so an in-place
            // `upsert_card` does not re-render the card view. Toggle
            // `selected_card` to force the detail closure to re-run
            // and pick up the new content — the side effect of
            // collapsing the version history is acceptable since the
            // user just performed an explicit action.
            //
            // `set_selection_without_flush` instead of `set_selection`:
            // the UUID is unchanged before/after so any burst still
            // belongs to the same card; going through the full helper
            // would also reset `editing` / `creating_new` mid-restore.
            if let Some(sel) = state.selected_card().get_untracked() {
                set_selection_without_flush(&state, None);
                set_selection_without_flush(&state, Some(sel));
            }
            pending_priority::flush_now(&state).await;
            push_card_or_queue(&state, restored).await;
        });
    };

    // "New from this" — stash the version's content/tags/due-date as a
    // prefill, set the requested placement, and open the new-card
    // editor. The user then tweaks the copy and saves it normally
    // (going through the usual placement / push path in `CardEditor`),
    // instead of silently creating a clone behind the scenes.
    //
    // Behaves like the main `+ New Card` button: the direct click
    // defaults to `NewCardPosition::Top`, the dropdown lets the user
    // pick top / bottom / above this card / below this card. The
    // editor flags the prefilled content as unsaved from the moment
    // it mounts. CardEditor's save path uses `push_card_or_queue`
    // so offline / priority-rebalance cases are handled by the
    // same code as a regular new-card save.
    let on_fork = move |version: Card, position: NewCardPosition| {
        // Stash the prefill before `start_new_card` so CardEditor's
        // mount-time read of `new_card_prefill` picks it up.
        state.new_card_prefill.set(Some(NewCardPrefill {
            content: version.content().to_string(),
            tags: version.tags().to_vec(),
            due_date: version.due_date(),
        }));
        if start_new_card(&state, position) {
            fork_dropdown.set(false);
        }
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

                // items[i+1] is the previous version (lower count) since the
                // list is sorted newest-first. The last item has no prev,
                // which `card_changes` treats as a Created entry.
                let tagged: Vec<(Card, Vec<CardChangeKind>)> = items.iter().enumerate().map(|(i, v)| {
                    let prev = items.get(i + 1);
                    (v.clone(), card_changes(prev, v))
                }).collect();

                // Only show kinds that are actually present in the history,
                // so the user is never offered a chip that filters everything out.
                let mut present: Vec<CardChangeKind> = Vec::new();
                for (_, changes) in &tagged {
                    for c in changes {
                        if !present.contains(c) {
                            present.push(*c);
                        }
                    }
                }
                let mut ordered_present: Vec<CardChangeKind> = CardChangeKind::all()
                    .iter()
                    .copied()
                    .filter(|k| present.contains(k))
                    .collect();
                // If the current filter selects a kind no longer present
                // (e.g. history shrank), drop it.
                if let Some(active) = filter.get_untracked()
                    && !ordered_present.contains(&active)
                {
                    filter.set(None);
                }

                let active_filter = filter.get();
                let filtered: Vec<(Card, Vec<CardChangeKind>)> = tagged.into_iter()
                    .filter(|(_, changes)| match active_filter {
                        None => true,
                        Some(k) => changes.contains(&k),
                    })
                    .collect();

                // Filter chip view — shown only when there's something to filter.
                let show_filter_bar = ordered_present.len() > 1;
                let filter_bar = show_filter_bar.then(|| {
                    let all_active = active_filter.is_none();
                    let chips = ordered_present.drain(..).map(|kind| {
                        let is_active = active_filter == Some(kind);
                        let mut class = String::from("history-filter-chip ");
                        class.push_str(kind.css_class());
                        if is_active {
                            class.push_str(" active");
                        }
                        view! {
                            <button class=class on:click=move |_| {
                                filter.update(|f| {
                                    if *f == Some(kind) {
                                        *f = None;
                                    } else {
                                        *f = Some(kind);
                                    }
                                });
                            }>{kind.label()}</button>
                        }
                    }).collect::<Vec<_>>();
                    view! {
                        <div class="history-filter-bar">
                            <button
                                class=if all_active { "history-filter-chip history-filter-all active" } else { "history-filter-chip history-filter-all" }
                                on:click=move |_| filter.set(None)
                            >"All"</button>
                            {chips}
                        </div>
                    }
                });

                let empty_after_filter = filtered.is_empty();

                view! {
                    <div class="version-list">
                        {filter_bar}
                        {empty_after_filter.then(|| view! {
                            <p class="version-loading">"No versions match this filter."</p>
                        })}
                        {filtered.into_iter().map(|(v, changes)| {
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
                            let badges = changes.clone();
                            let expanded_view = if is_expanded {
                                let v_for_fork_main = v.clone();
                                let v_for_fork_top = v.clone();
                                let v_for_fork_bot = v.clone();
                                let v_for_fork_above = v.clone();
                                let v_for_fork_below = v.clone();
                                let v_for_restore = v.clone();
                                let on_fork = on_fork;
                                let on_restore = on_restore;
                                let content_html = render_markdown(v.content());
                                let created = v.created_at();
                                let modified = v.modified_at();
                                let is_blazed = v.blazed();
                                let priority = v.priority();
                                let priority_pct = blazelist_client_lib::priority::priority_percentage(priority);

                                let all_tags = state.tags.get_untracked();
                                let mut tag_entries: Vec<(String, Option<rgb::RGB8>)> = v.tags().iter().filter_map(|tid| {
                                    all_tags.iter().find(|t| t.id() == *tid).map(|t| {
                                        (t.title().to_string(), t.color())
                                    })
                                }).collect();
                                tag_entries.sort_by_key(|a| a.0.to_lowercase());

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
                                                <Timestamp datetime=modified class="meta-value" />
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Created"</span>
                                                <Timestamp datetime=created class="meta-value" />
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Due"</span>
                                                {match due_date {
                                                    Some(d) => view! {
                                                        <Timestamp datetime=d class="meta-value" />
                                                    }.into_any(),
                                                    None => view! {
                                                        <span class="meta-value due-not-set">"Not set"</span>
                                                    }.into_any(),
                                                }}
                                            </div>
                                        </div>
                                        <div class="version-expanded-actions">
                                            <div class="btn-new-card-group" node_ref=fork_group_ref>
                                                <button class="btn-new-card" on:click=move |ev: web_sys::MouseEvent| {
                                                    ev.stop_propagation();
                                                    let v = v_for_fork_main.clone();
                                                    on_fork(v, NewCardPosition::Top);
                                                }>"New from this"</button>
                                                <button class="btn-new-card-dropdown" on:click=move |ev: web_sys::MouseEvent| {
                                                    ev.stop_propagation();
                                                    fork_dropdown.update(|v| *v = !*v);
                                                }>
                                                    // Menu opens upward, so the glyphs are flipped
                                                    // relative to the main `+ New Card` button:
                                                    // closed → ▴ (will expand upward),
                                                    // open → ▾ (click to collapse back down).
                                                    {move || if fork_dropdown.get() { "\u{25BE}" } else { "\u{25B4}" }}
                                                </button>
                                                {move || fork_dropdown.get().then(|| {
                                                    let v_top = v_for_fork_top.clone();
                                                    let v_bot = v_for_fork_bot.clone();
                                                    let v_above = v_for_fork_above.clone();
                                                    let v_below = v_for_fork_below.clone();
                                                    view! {
                                                        <div class="new-card-dropdown-menu">
                                                            <button class="save-dropdown-item" on:click=move |_| {
                                                                let v = v_bot.clone();
                                                                on_fork(v, NewCardPosition::Bottom);
                                                            }>"Add to bottom"</button>
                                                            <button class="save-dropdown-item" on:click=move |_| {
                                                                let v = v_top.clone();
                                                                on_fork(v, NewCardPosition::Top);
                                                            }>"Add to top"</button>
                                                            <button class="save-dropdown-item" on:click=move |_| {
                                                                let v = v_above.clone();
                                                                on_fork(v, NewCardPosition::Above(card_id));
                                                            }>"Add above selected"</button>
                                                            <button class="save-dropdown-item" on:click=move |_| {
                                                                let v = v_below.clone();
                                                                on_fork(v, NewCardPosition::Below(card_id));
                                                            }>"Add below selected"</button>
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                            {(!is_current).then(|| {
                                                let on_restore = on_restore;
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
                                        <div class="version-changes">
                                            {badges.into_iter().map(|kind| {
                                                let class = format!("history-kind {}", kind.css_class());
                                                view! { <span class=class>{kind.label()}</span> }
                                            }).collect::<Vec<_>>()}
                                        </div>
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

/// Inline version history section for a tag. Renders a "History" label
/// followed by an expandable version list.
///
/// Mirrors [`VersionHistory`] for cards — the two are kept parallel so that
/// structural improvements made to one are easy to apply to the other.
#[component]
pub fn TagVersionHistory(tag_id: Uuid) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let versions: RwSignal<Vec<Tag>> = RwSignal::new(Vec::new());
    let loading = RwSignal::new(true);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let expanded: RwSignal<Option<i64>> = RwSignal::new(None);
    let prev_tag: RwSignal<Option<Uuid>> = RwSignal::new(None);
    let filter: RwSignal<Option<TagChangeKind>> = RwSignal::new(None);

    // Fetch history on mount — show cached data first, then refresh from server.
    // Also re-trigger when connection status changes so that history is fetched
    // after the client connects (the tag detail no longer re-creates this
    // component on sync, so the Effect must retry on its own).
    Effect::new(move |_| {
        // `selected_card` is the shared signal used for both cards and tags;
        // subscribing here re-triggers the effect whenever the selection changes.
        let selected = state.selected_card().get();
        let _ = state.connection_status.get(); // re-trigger on connect

        // Only reset UI state when the selected tag changes,
        // not on every connection status transition.
        if prev_tag.get_untracked() != selected {
            error_msg.set(None);
            expanded.set(None);
            filter.set(None);
            prev_tag.set(selected);
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
                        history.sort_by_key(|t| std::cmp::Reverse(t.count()));
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
                loading.set(false);
            }
        });
    });

    let on_toggle_expand = move |count: i64| {
        toggle_expanded(expanded, count);
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

                let tagged: Vec<(Tag, Vec<TagChangeKind>)> = items.iter().enumerate().map(|(i, v)| {
                    let prev = items.get(i + 1);
                    (v.clone(), tag_changes(prev, v))
                }).collect();

                let mut present: Vec<TagChangeKind> = Vec::new();
                for (_, changes) in &tagged {
                    for c in changes {
                        if !present.contains(c) {
                            present.push(*c);
                        }
                    }
                }
                let mut ordered_present: Vec<TagChangeKind> = TagChangeKind::all()
                    .iter()
                    .copied()
                    .filter(|k| present.contains(k))
                    .collect();
                if let Some(active) = filter.get_untracked()
                    && !ordered_present.contains(&active)
                {
                    filter.set(None);
                }

                let active_filter = filter.get();
                let filtered: Vec<(Tag, Vec<TagChangeKind>)> = tagged.into_iter()
                    .filter(|(_, changes)| match active_filter {
                        None => true,
                        Some(k) => changes.contains(&k),
                    })
                    .collect();

                let show_filter_bar = ordered_present.len() > 1;
                let filter_bar = show_filter_bar.then(|| {
                    let all_active = active_filter.is_none();
                    let chips = ordered_present.drain(..).map(|kind| {
                        let is_active = active_filter == Some(kind);
                        let mut class = String::from("history-filter-chip ");
                        class.push_str(kind.css_class());
                        if is_active {
                            class.push_str(" active");
                        }
                        view! {
                            <button class=class on:click=move |_| {
                                filter.update(|f| {
                                    if *f == Some(kind) {
                                        *f = None;
                                    } else {
                                        *f = Some(kind);
                                    }
                                });
                            }>{kind.label()}</button>
                        }
                    }).collect::<Vec<_>>();
                    view! {
                        <div class="history-filter-bar">
                            <button
                                class=if all_active { "history-filter-chip history-filter-all active" } else { "history-filter-chip history-filter-all" }
                                on:click=move |_| filter.set(None)
                            >"All"</button>
                            {chips}
                        </div>
                    }
                });

                let empty_after_filter = filtered.is_empty();

                view! {
                    <div class="version-list">
                        {filter_bar}
                        {empty_after_filter.then(|| view! {
                            <p class="version-loading">"No versions match this filter."</p>
                        })}
                        {filtered.into_iter().map(|(v, changes)| {
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
                            let badges = changes.clone();
                            let expanded_view = if is_expanded {
                                let created = v.created_at();
                                let modified = v.modified_at();
                                let version_color = v.color().map(|c| blazelist_client_lib::color::format_tag_hex(&c));
                                // Resolve implies to (title, color) pairs.
                                let all_tags = state.tags.get_untracked();
                                let mut implies_entries: Vec<(String, Option<rgb::RGB8>)> = v.implies().iter().filter_map(|tid| {
                                    all_tags.iter().find(|t| t.id() == *tid).map(|t| {
                                        (t.title().to_string(), t.color())
                                    })
                                }).collect();
                                implies_entries.sort_by_key(|a| a.0.to_lowercase());
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
                                                <span class="meta-label">"Implies"</span>
                                                {if implies_entries.is_empty() {
                                                    view! { <span class="meta-value due-not-set">"None"</span> }.into_any()
                                                } else {
                                                    let implies = implies_entries;
                                                    view! {
                                                        <div class="detail-tag-chips">
                                                            {implies.into_iter().map(|(name, color)| {
                                                                let style = tag_chip_style(&color);
                                                                view! {
                                                                    <span class="tag-chip" style=style>{name}</span>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    }.into_any()
                                                }}
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Modified"</span>
                                                <Timestamp datetime=modified class="meta-value" />
                                            </div>
                                            <div class="meta-row">
                                                <span class="meta-label">"Created"</span>
                                                <Timestamp datetime=created class="meta-value" />
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
                                        <div class="version-changes">
                                            {badges.into_iter().map(|kind| {
                                                let class = format!("history-kind {}", kind.css_class());
                                                view! { <span class=class>{kind.label()}</span> }
                                            }).collect::<Vec<_>>()}
                                        </div>
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

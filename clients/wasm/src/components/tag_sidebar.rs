use crate::components::sequence_history::SequenceHistory;
use crate::state::store::{AppState, set_selection, start_new_tag, sync_query_params};
use blazelist_client_lib::tag_graph::TagGraph;
use blazelist_protocol::Entity;
use leptos::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

#[component]
pub fn TagSidebar() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let search = RwSignal::new(String::new());

    // Toggle a tag in/out of the active tag filter. Shared by the search
    // box's Enter shortcut and each tag row's click handler: clears the
    // no-tags filter when the current mode disallows it, flips membership
    // of `tag_id`, optionally clears the search box, and syncs the URL.
    let toggle_tag_filter = move |tag_id: Uuid| {
        if !state.tag_filter_mode.get_untracked().allows_no_tags() {
            state.no_tags_filter.set(false);
        }
        state.tag_filter.update(|tags| {
            if tags.contains(&tag_id) {
                tags.retain(|t| *t != tag_id);
            } else {
                tags.push(tag_id);
            }
        });
        if state.clear_tag_search.get_untracked() {
            search.set(String::new());
        }
        sync_query_params(&state);
    };

    // Transitive implication stats: for each tag, count how many tags
    // it transitively implies (fwd) and how many tags transitively
    // imply it (rev). Uses TagGraph::closure_of for accurate counts.
    let implies_stats: Memo<HashMap<Uuid, (usize, usize)>> = Memo::new(move |_| {
        let tags = state.tags.get();
        let graph = TagGraph::from_tags(&tags);
        let mut stats: HashMap<Uuid, (usize, usize)> = HashMap::new();
        for tag in &tags {
            // Forward: size of closure minus self.
            let closure = graph.closure_of(&[tag.id()]);
            let fwd = closure.len().saturating_sub(1);
            stats.entry(tag.id()).or_insert((0, 0)).0 = fwd;
            // Reverse: every other tag in this tag's closure is
            // transitively implied-by this tag (bump their rev count).
            for implied_id in &closure {
                if *implied_id != tag.id() {
                    stats.entry(*implied_id).or_insert((0, 0)).1 += 1;
                }
            }
        }
        stats
    });

    let on_new_tag = move |_| {
        start_new_tag(&state);
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
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            ev.prevent_default();
                            let q = search.get_untracked().to_lowercase();
                            let mut tags = state.tags.get_untracked();
                            tags.sort_by_key(|a| a.title().to_lowercase());
                            if !q.is_empty() {
                                tags.retain(|t| t.title().to_lowercase().contains(&q));
                            }
                            if let Some(first) = tags.first() {
                                toggle_tag_filter(first.id());
                            }
                        }
                    }
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
                                if state.clear_tag_search.get_untracked() {
                                    search.set(String::new());
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
                tags.sort_by_key(|a| a.title().to_lowercase());
                if !q.is_empty() {
                    tags.retain(|t| t.title().to_lowercase().contains(&q));
                }
                tags.into_iter().map(|tag| {
                    let tag_id = tag.id();
                    let title = tag.title().to_string();
                    let color = tag.color().map(|c| blazelist_client_lib::color::format_tag_hex(&c));
                    let is_active = move || state.tag_filter.get().contains(&tag_id);

                    let toggle_filter = move |_| toggle_tag_filter(tag_id);

                    let on_manage = move |ev: web_sys::MouseEvent| {
                        ev.stop_propagation();
                        set_selection(&state, Some(tag_id));
                    };

                    let item_class = move || if is_active() { "tag-item active" } else { "tag-item" };

                    let border_style = color
                        .map(|c| format!("border-left: 3px solid {c};"))
                        .unwrap_or_else(|| "border-left: 3px solid transparent;".to_string());

                    // Implication indicators (→N / ←M) next to the tag title.
                    let tag_implies_indicator = move || {
                        let stats = implies_stats.get();
                        let (fwd, rev) = stats.get(&tag_id).copied().unwrap_or((0, 0));
                        let has_any = fwd > 0 || rev > 0;
                        has_any.then(|| {
                            let fwd_view = (fwd > 0).then(|| {
                                let text = format!("\u{2192}{fwd}");
                                let tip = format!("implies {fwd} tag{}", if fwd == 1 { "" } else { "s" });
                                view! { <span class="tag-implies-fwd" title=tip>{text}</span> }
                            });
                            let rev_view = (rev > 0).then(|| {
                                let text = format!("\u{2190}{rev}");
                                let tip = format!("implied by {rev} tag{}", if rev == 1 { "" } else { "s" });
                                view! { <span class="tag-implies-rev" title=tip>{text}</span> }
                            });
                            view! {
                                <span class="tag-implies-indicators">{fwd_view}{rev_view}</span>
                            }
                        })
                    };

                    view! {
                        <li class=item_class style=border_style on:click=toggle_filter>
                            <span class="tag-title">{title}</span>
                            {tag_implies_indicator}
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
                        // ── Sync ──
                        <div class="meta-section-label">"Sync"</div>
                        <div class="meta-row">
                            <span class="meta-label">"Last Sync"</span>
                            <span class="meta-value">{synced}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Operations"</span>
                            <span class="meta-value">{sync_ops_str}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Duration"</span>
                            <span class="meta-value">{sync_duration}</span>
                        </div>

                        // ── Data ──
                        <div class="meta-section-label">"Data"</div>
                        <div class="meta-row">
                            <span class="meta-label">"Cards"</span>
                            <span class="meta-value">{format!("{total_cards} ({active_cards} active, {blazed_cards} blazed)")}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Tags"</span>
                            <span class="meta-value">{total_tags.to_string()}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Entities"</span>
                            <span class="meta-value">{format!("{total_entities} ({deleted} deleted)")}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Sequence"</span>
                            <span class="meta-value">{sequence}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Root Hash"</span>
                            <span class="meta-value">{root_hash}</span>
                        </div>

                        // ── Local Cache ──
                        <div class="meta-section-label">"Local Cache"</div>
                        <div class="meta-row">
                            <span class="meta-label">"Card Histories"</span>
                            <span class="meta-value">{move || {
                                let count = crate::storage::cached_card_history_count();
                                if count > 0 { count.to_string() } else { "---".to_string() }
                            }}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Tag Histories"</span>
                            <span class="meta-value">{move || {
                                let count = crate::storage::cached_tag_history_count();
                                if count > 0 { count.to_string() } else { "---".to_string() }
                            }}</span>
                        </div>
                        <div class="meta-row">
                            <span class="meta-label">"Link Graph"</span>
                            <span class="meta-value">{move || {
                                let cached = state.link_graph_cache.get().len();
                                let (grand_total, remaining) = state.link_cache_progress.get();
                                if remaining > 0 && grand_total > 0 {
                                    let pct = cached * 100 / grand_total;
                                    format!("{cached}/{grand_total} \u{2022} {pct}%")
                                } else if cached > 0 {
                                    format!("{cached}/{cached}")
                                } else {
                                    "---".to_string()
                                }
                            }}</span>
                        </div>
                        {move || {
                            let cached = state.link_graph_cache.get().len();
                            let (grand_total, remaining) = state.link_cache_progress.get();
                            (remaining > 0 && grand_total > 0).then(|| {
                                let pct = cached * 100 / grand_total;
                                let pct_clamped = pct.min(100);
                                view! {
                                    <div class="cache-progress-bar">
                                        <div class="cache-progress-fill" style=format!("width:{pct_clamped}%")></div>
                                    </div>
                                }
                            })
                        }}
                        <div class="meta-row">
                            <span class="meta-label">"Offline Queue"</span>
                            <span class="meta-value">{move || {
                                let queue = state.offline_queue.get().len();
                                if queue > 0 { format!("{queue} pending") } else { "empty".to_string() }
                            }}</span>
                        </div>

                        // ── Versions ──
                        <div class="meta-section-label">"Versions"</div>
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
                            <span class="meta-value">{format!("v{}", blazelist_protocol::PROTOCOL_VERSION_STR)}</span>
                        </div>
                    }
                }}
            </div>
            <SequenceHistory />
        </>
    }
}

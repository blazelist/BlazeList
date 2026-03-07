use crate::components::hooks::toggle_expanded;
use crate::state::store::{AppState, get_client, sync_query_params};
use blazelist_client_lib::client::Client as _;
use blazelist_protocol::{Entity, SequenceHistoryEntry, SequenceOperationKind};
use leptos::prelude::*;

/// Fetch sequence history from the server and update signals.
fn fetch_history(
    entries: RwSignal<Vec<SequenceHistoryEntry>>,
    loading: RwSignal<bool>,
    error_msg: RwSignal<Option<String>>,
) {
    loading.set(true);
    error_msg.set(None);
    leptos::task::spawn_local(async move {
        if let Some(client) = get_client() {
            match client.get_sequence_history().await {
                Ok(history) => {
                    entries.set(history);
                }
                Err(e) => {
                    error_msg.set(Some(format!("Failed to load history: {e}")));
                }
            }
        }
        loading.set(false);
    });
}

#[component]
pub fn SequenceHistory() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let expanded = RwSignal::new(false);
    let entries: RwSignal<Vec<SequenceHistoryEntry>> = RwSignal::new(Vec::new());
    let loading = RwSignal::new(false);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let expanded_seq: RwSignal<Option<i64>> = RwSignal::new(None);
    let fetched = RwSignal::new(false);

    // Re-fetch when root state changes (new mutations arrived via subscription).
    Effect::new(move |_| {
        let _ = state.root.get(); // reactive dependency on root state
        if fetched.get() {
            fetch_history(entries, loading, error_msg);
        }
    });

    let on_toggle = move |_| {
        let is_expanded = expanded.get();
        expanded.set(!is_expanded);

        // Lazy-load on first expand
        if !is_expanded && !fetched.get() {
            fetched.set(true);
            fetch_history(entries, loading, error_msg);
        }
    };

    let on_toggle_seq = move |seq: i64| {
        toggle_expanded(expanded_seq, seq);
    };

    view! {
        <div class="sequence-history-section">
            <div class="sequence-history-header" on:click=on_toggle>
                <span class="sequence-history-toggle">{move || if expanded.get() { "\u{25be}" } else { "\u{25b8}" }}</span>
                <span class="sequence-history-label">"Sequence History"</span>
            </div>
            {move || {
                if !expanded.get() {
                    return view! { <div></div> }.into_any();
                }
                if loading.get() {
                    return view! {
                        <div class="sequence-history-list">
                            <p class="version-loading">"Loading history\u{2026}"</p>
                        </div>
                    }.into_any();
                }
                if let Some(err) = error_msg.get() {
                    return view! {
                        <div class="sequence-history-list">
                            <p class="error">{err}</p>
                        </div>
                    }.into_any();
                }
                let items = entries.get();
                if items.is_empty() {
                    return view! {
                        <div class="sequence-history-list">
                            <p class="version-loading">"No history available."</p>
                        </div>
                    }.into_any();
                }
                let current_expanded = expanded_seq.get();
                // Compute zero-padding width from the max sequence number.
                let max_seq = items.iter()
                    .map(|e| i64::from(e.sequence))
                    .max()
                    .unwrap_or(0);
                let num_width = max_seq.to_string().len();

                view! {
                    <div class="sequence-history-list">
                        {items.into_iter().map(|entry| {
                            let seq = i64::from(entry.sequence);
                            let full_hash = entry.hash.to_hex().to_string();
                            let op_count = entry.operations.len();
                            let created_at = entry.created_at;
                            let is_expanded = current_expanded == Some(seq);
                            let entry_class = if is_expanded { "seq-entry expanded" } else { "seq-entry" };
                            let hash_class = if is_expanded { "seq-hash seq-hash-full" } else { "seq-hash" };
                            let num_class = if is_expanded { "seq-number seq-number-expanded" } else { "seq-number" };

                            // Zero-padded, no # in collapsed, #<num> when expanded.
                            let seq_display = if is_expanded {
                                format!("#{:0>width$}", seq, width = num_width)
                            } else {
                                format!("{:0>width$}", seq, width = num_width)
                            };

                            let ops_view = if is_expanded {
                                if op_count == 0 {
                                    Some(view! {
                                        <div class="seq-ops">
                                            <span class="seq-ops-purged">"Operations unavailable \u{2014} version history purged by deletion"</span>
                                        </div>
                                    }.into_any())
                                } else {
                                    let cards = state.cards.get_untracked();
                                    let tags = state.tags.get_untracked();
                                    let ops = entry.operations.clone();
                                    Some(view! {
                                        <div class="seq-ops">
                                            {ops.into_iter().map(|op| {
                                                let (kind_label, kind_class) = match op.kind {
                                                    SequenceOperationKind::CardCreated => ("Card created", "seq-kind-card-created"),
                                                    SequenceOperationKind::CardUpdated => ("Card updated", "seq-kind-card-updated"),
                                                    SequenceOperationKind::TagCreated => ("Tag created", "seq-kind-tag-created"),
                                                    SequenceOperationKind::TagUpdated => ("Tag updated", "seq-kind-tag-updated"),
                                                    SequenceOperationKind::EntityDeleted => ("Deleted", "seq-kind-deleted"),
                                                };
                                                let entity_name = match op.kind {
                                                    SequenceOperationKind::CardCreated | SequenceOperationKind::CardUpdated => {
                                                        cards.iter()
                                                            .find(|c| c.id() == op.entity_id)
                                                            .map(|c| blazelist_client_lib::display::card_preview(c.content(), 80)
                                                                .unwrap_or_else(|| "(empty)".to_string()))
                                                            .unwrap_or_else(|| op.entity_id.to_string())
                                                    }
                                                    SequenceOperationKind::TagCreated | SequenceOperationKind::TagUpdated => {
                                                        tags.iter()
                                                            .find(|t| t.id() == op.entity_id)
                                                            .map(|t| t.title().to_string())
                                                            .unwrap_or_else(|| op.entity_id.to_string())
                                                    }
                                                    SequenceOperationKind::EntityDeleted => {
                                                        op.entity_id.to_string()
                                                    }
                                                };
                                                let entity_id = op.entity_id;
                                                let on_click = move |ev: web_sys::MouseEvent| {
                                                    ev.stop_propagation();
                                                    state.selected_card.set(Some(entity_id));
                                                    sync_query_params(&state);
                                                };
                                                view! {
                                                    <div class="seq-op clickable" on:click=on_click>
                                                        <span class={format!("seq-kind {kind_class}")}>{kind_label}</span>
                                                        <span class="seq-op-name">{entity_name}</span>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any())
                                }
                            } else {
                                None
                            };

                            let op_label = if op_count == 0 {
                                "operations purged".to_string()
                            } else if op_count == 1 {
                                format!("{op_count} operation")
                            } else {
                                format!("{op_count} operations")
                            };

                            view! {
                                <div class=entry_class>
                                    <div class="seq-entry-row" on:click=move |_| on_toggle_seq(seq)>
                                        <span class=num_class>{seq_display}</span>
                                        <span class=hash_class>{full_hash.clone()}</span>
                                        <span class="seq-op-count">{op_label}</span>
                                        {
                                            let ts_display = created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                            view! { <span class="seq-timestamp">{ts_display}</span> }.into_any()
                                        }
                                    </div>
                                    {ops_view}
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

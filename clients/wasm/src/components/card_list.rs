use crate::components::card_item::CardItem;
use crate::components::hooks::use_click_outside_close;
use crate::state::store::{AppState, NewCardPosition, confirm_discard_changes, sync_query_params};
use blazelist_protocol::{Card, Entity};
use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[component]
pub fn CardList() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let filtered = state.filtered_cards();

    // Direct link counts only — O(N) single pass.
    let link_counts = Memo::new(move |_| {
        let all = state.cards.get();
        blazelist_client_lib::display::compute_all_link_counts(&all)
    });

    let card_map: Memo<HashMap<Uuid, Arc<Card>>> = Memo::new(move |_| {
        filtered
            .get()
            .into_iter()
            .map(|c| (c.id(), Arc::new(c)))
            .collect()
    });

    let card_positions: Memo<HashMap<Uuid, (usize, usize)>> = Memo::new(move |_| {
        let cards = filtered.get();
        let total = cards.len();
        cards
            .into_iter()
            .enumerate()
            .map(|(i, c)| (c.id(), (i + 1, total)))
            .collect()
    });

    // Background computation of the link graph cache via requestIdleCallback.
    // Processes as many cards as fit in the browser's idle budget per callback,
    // automatically backing off during user interaction.
    {
        let generation = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let gen_for_effect = generation.clone();
        let last_fingerprint = std::rc::Rc::new(std::cell::Cell::new(0u64));
        Effect::new(move |_| {
            let enabled = state.show_list_link_counts.get();
            let cards = state.cards.get();
            let _ = state.recursive_links.get();

            if !enabled || !state.recursive_links.get_untracked() {
                // Advance generation to cancel any in-flight
                // requestIdleCallback chains that would refill the cache.
                gen_for_effect.set(gen_for_effect.get().wrapping_add(1));
                state.link_graph_cache.set(HashMap::new());
                state.link_cache_progress.set((0, 0));
                last_fingerprint.set(0);
                return;
            }

            // Fingerprint from card count + version sums. Every edit increments
            // a card's count, so this catches all content changes cheaply.
            let fp: u64 = cards.iter().map(|c| u64::from(c.count())).fold(
                (cards.len() as u64).wrapping_mul(0x517c_c1b7_2722_0a95),
                |acc, v| acc.wrapping_add(v),
            );
            let prev_fp = last_fingerprint.get();
            if fp == prev_fp {
                return;
            }
            last_fingerprint.set(fp);

            let cur_gen = gen_for_effect.get().wrapping_add(1);
            gen_for_effect.set(cur_gen);
            let gen_rc = generation.clone();

            let direct = blazelist_client_lib::display::compute_all_link_counts(&cards);

            // Count ALL cards with links (the grand total for progress display).
            let all_linked: Vec<Uuid> = cards
                .iter()
                .filter(|c| {
                    direct
                        .get(&c.id())
                        .map(|lc| lc.forward > 0 || lc.back > 0 || lc.mutual > 0)
                        .unwrap_or(false)
                })
                .map(|c| c.id())
                .collect();
            let grand_total = all_linked.len();

            // Only process cards not already in the cache.
            let existing = state.link_graph_cache.get_untracked();
            let card_ids: Vec<Uuid> = all_linked
                .iter()
                .copied()
                .filter(|id| !existing.contains_key(id))
                .collect();

            // Build a map of card ID → blake3 hash of content, but only for
            // cards that actually need it: cache entries (to detect content
            // changes) and new cache candidates (to store alongside the
            // entry). Previously this hashed every card in the workspace on
            // every edit, which was O(N) blake3 work per keystroke — a
            // noticeable regression at 1000+ cards.
            let needed_hashes: std::collections::HashSet<Uuid> = existing
                .keys()
                .copied()
                .chain(card_ids.iter().copied())
                .collect();
            let content_hashes: HashMap<Uuid, [u8; 32]> = cards
                .iter()
                .filter(|c| needed_hashes.contains(&c.id()))
                .map(|c| (c.id(), *blake3::hash(c.content().as_bytes()).as_bytes()))
                .collect();

            // Selective invalidation: remove cache entries where the card's
            // content changed or a card in its reachable set changed content.
            // Always runs (including first load from OPFS) to prune stale entries.
            {
                let card_set: std::collections::HashSet<Uuid> =
                    cards.iter().map(|c| c.id()).collect();

                // Find cards whose content hash changed.
                let changed: std::collections::HashSet<Uuid> = existing
                    .iter()
                    .filter_map(|(id, (cached_hash, _))| {
                        match content_hashes.get(id) {
                            Some(cur_hash) if cur_hash != cached_hash => Some(*id),
                            None => Some(*id), // card deleted
                            _ => None,
                        }
                    })
                    .collect();

                if !changed.is_empty() {
                    state.link_graph_cache.update(|cache| {
                        // Remove entries for changed cards.
                        for id in &changed {
                            cache.remove(id);
                        }
                        // Remove entries that had a changed card in their
                        // reachable set (transitive invalidation).
                        cache.retain(|_, (_, reachable)| {
                            !reachable.iter().any(|r| changed.contains(r))
                        });
                        // Prune deleted cards.
                        cache.retain(|id, _| card_set.contains(id));
                    });
                } else {
                    // No content changes — just prune deleted cards.
                    state.link_graph_cache.update(|cache| {
                        cache.retain(|id, _| card_set.contains(id));
                    });
                }
            }

            if card_ids.is_empty() {
                state.link_cache_progress.set((0, 0));
                return;
            }

            // Progress: (grand_total_linked, uncached_remaining).
            // Sidebar reads cache.len() for "done" and grand_total for the denominator.
            state.link_cache_progress.set((grand_total, card_ids.len()));

            let cards = std::rc::Rc::new(cards);
            let content_hashes = std::rc::Rc::new(content_hashes);
            let offset = std::rc::Rc::new(std::cell::Cell::new(0usize));
            let js_fn: std::rc::Rc<std::cell::RefCell<Option<js_sys::Function>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let js_fn2 = js_fn.clone();

            let closure = Closure::wrap(Box::new(move |deadline_val: JsValue| {
                if gen_rc.get() != cur_gen {
                    return;
                }
                let deadline: web_sys::IdleDeadline = deadline_val.unchecked_into();

                // Process cards while we have idle time budget (>1ms headroom).
                // Always process at least 1 card to guarantee forward progress
                // even when the browser reports 0 remaining time.
                let mut batch: Vec<(Uuid, [u8; 32], Vec<Uuid>)> = Vec::new();
                let mut first = true;
                while first || deadline.time_remaining() > 1.0 {
                    first = false;
                    let idx = offset.get();
                    if idx >= card_ids.len() {
                        break;
                    }
                    let cid = card_ids[idx];
                    let expanded = blazelist_client_lib::display::expand_linked_cards(cid, &cards);
                    let hash = content_hashes.get(&cid).copied().unwrap_or([0; 32]);
                    batch.push((cid, hash, expanded));
                    offset.set(idx + 1);
                }

                // Single bulk update — triggers reactive subscribers once.
                if !batch.is_empty() {
                    state.link_graph_cache.update(|cache| {
                        for (id, hash, expanded) in batch {
                            cache.insert(id, (hash, expanded));
                        }
                    });
                }

                let processed = offset.get();
                let remaining = card_ids.len() - processed;
                state.link_cache_progress.set((grand_total, remaining));

                // Persist to OPFS after every batch so progress survives reloads.
                let cache = state.link_graph_cache.get_untracked();
                leptos::task::spawn_local(async move {
                    crate::storage::save_link_cache(&cache).await;
                });

                if processed < card_ids.len() {
                    // Use a 2s timeout so the callback fires even if the
                    // browser can't find idle time (e.g. tab backgrounded).
                    if let Some(ref f) = *js_fn2.borrow() {
                        let opts = web_sys::IdleRequestOptions::new();
                        opts.set_timeout(2_000);
                        let _ = web_sys::window()
                            .unwrap()
                            .request_idle_callback_with_options(f, &opts);
                    }
                } else {
                    state.link_cache_progress.set((0, 0));
                }
            }) as Box<dyn FnMut(JsValue)>);

            let func: js_sys::Function =
                closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
            closure.forget();
            *js_fn.borrow_mut() = Some(func.clone());

            // Initial kick-off with timeout.
            let opts = web_sys::IdleRequestOptions::new();
            opts.set_timeout(2_000);
            let _ = web_sys::window()
                .unwrap()
                .request_idle_callback_with_options(&func, &opts);
        });
    }

    // Progressive rendering: on first load or when the card set changes
    // significantly, render an initial batch immediately and fill the rest
    // via requestIdleCallback. Reorders (same cards, different order) skip
    // the reset entirely to avoid flicker.
    const INITIAL_BATCH: usize = 40;
    let render_limit = RwSignal::new(INITIAL_BATCH);

    {
        let fill_gen = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let fill_gen_effect = fill_gen.clone();
        let last_count = std::rc::Rc::new(std::cell::Cell::new(0usize));
        Effect::new(move |_| {
            let total = filtered.get().len();
            let prev_count = last_count.get();
            let current_limit = render_limit.get_untracked();
            last_count.set(total);

            // If the list is already fully rendered and the count didn't
            // change, this is just a reorder — keep showing everything.
            if current_limit >= prev_count && total == prev_count {
                render_limit.set(total);
                return;
            }

            // If the count changed but we were already showing everything,
            // just update to the new total (card added/removed).
            if current_limit >= prev_count && total > 0 {
                render_limit.set(total);
                return;
            }

            // Otherwise, reset to initial batch (e.g., first load or
            // significant filter change that reduced the visible set).
            render_limit.set(INITIAL_BATCH.min(total));

            let cur = fill_gen_effect.get().wrapping_add(1);
            fill_gen_effect.set(cur);
            let gen_rc = fill_gen.clone();

            if total <= INITIAL_BATCH {
                return;
            }

            let cb = Closure::once(move |_: JsValue| {
                if gen_rc.get() != cur {
                    return;
                }
                render_limit.set(total);
            });

            let _ = web_sys::window()
                .unwrap()
                .request_idle_callback(cb.as_ref().unchecked_ref());
            cb.forget();
        });
    }

    let new_card_dropdown = RwSignal::new(false);
    let new_card_group_ref = NodeRef::<leptos::html::Div>::new();
    use_click_outside_close(new_card_dropdown, new_card_group_ref);

    let start_creating = move |position: NewCardPosition| {
        if !confirm_discard_changes(&state) {
            return;
        }
        state.selected_card.set(None);
        state.editing.set(false);
        state.new_card_position.set(position);
        state.creating_new.set(true);
        state.settings_open.set(false);
        state.shortcuts_open.set(false);
        new_card_dropdown.set(false);
        sync_query_params(&state);
    };

    let on_new_card = move |_| {
        start_creating(NewCardPosition::Bottom);
    };

    view! {
        <div class="card-list">
            <div class="btn-new-card-group" node_ref=new_card_group_ref>
                <button class="btn-new-card" on:click=on_new_card>
                    "+ New Card"
                </button>
                <button class="btn-new-card-dropdown" on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    new_card_dropdown.update(|v| *v = !*v);
                }>
                    {move || if new_card_dropdown.get() { "\u{25B4}" } else { "\u{25BE}" }}
                </button>
                {move || new_card_dropdown.get().then(|| {
                    let has_selected = state.selected_card.get_untracked().is_some();
                    view! {
                        <div class="new-card-dropdown-menu">
                            <button class="save-dropdown-item" on:click=move |_| {
                                start_creating(NewCardPosition::Bottom);
                            }>"Add to bottom"</button>
                            <button class="save-dropdown-item" on:click=move |_| {
                                start_creating(NewCardPosition::Top);
                            }>"Add to top"</button>
                            <button
                                class="save-dropdown-item"
                                disabled=!has_selected
                                on:click=move |_| {
                                    if let Some(id) = state.selected_card.get_untracked() {
                                        start_creating(NewCardPosition::Above(id));
                                    }
                                }
                            >"Add above selected"</button>
                            <button
                                class="save-dropdown-item"
                                disabled=!has_selected
                                on:click=move |_| {
                                    if let Some(id) = state.selected_card.get_untracked() {
                                        start_creating(NewCardPosition::Below(id));
                                    }
                                }
                            >"Add below selected"</button>
                        </div>
                    }
                })}
            </div>
            <div class="cards">
                <For
                    each=move || {
                        let cards = filtered.get();
                        let limit = render_limit.get();
                        cards.into_iter().take(limit).collect::<Vec<_>>()
                    }
                    key=|card| card.id()
                    children=move |card: Card| {
                        let id = card.id();
                        view! {
                            <CardItem
                                card_id=id
                                card_map=card_map
                                card_positions=card_positions
                                link_counts=link_counts
                            />
                        }
                    }
                />
            </div>
        </div>
    }
}

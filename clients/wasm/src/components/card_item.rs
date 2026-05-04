use crate::components::link_indicators::link_indicators_view;
use crate::state::store::{
    AppState, SwipeToast, confirm_discard_changes, format_due_date_badge, format_relative_time,
    select_card_view, sync_query_params,
};
use crate::state::sync::push_card_or_queue;
use blazelist_client_lib::display::LinkCounts;
use blazelist_protocol::{Card, Entity, Utc};
use leptos::prelude::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setTimeout")]
    fn set_timeout_js(handler: &js_sys::Function, timeout: i32) -> i32;
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout_js(handle: i32);
}

fn show_swipe_toast(state: &AppState, message: String, original_card: Card) {
    if let Some(prev) = state.swipe_toast.get_untracked() {
        clear_timeout_js(prev.timeout_handle);
    }
    let s = *state;
    let dismiss_cb = Closure::once_into_js(move || {
        s.swipe_toast.set(None);
    });
    let timeout_ms = state.swipe_undo_timeout_ms.get_untracked() as i32;
    let handle = set_timeout_js(dismiss_cb.unchecked_ref(), timeout_ms);
    state.swipe_toast.set(Some(SwipeToast {
        message,
        original_card,
        timeout_handle: handle,
    }));
}

#[component]
pub fn CardItem(
    card_id: Uuid,
    card_map: Memo<HashMap<Uuid, Arc<Card>>>,
    card_positions: Memo<HashMap<Uuid, (usize, usize)>>,
    link_counts: Memo<HashMap<Uuid, LinkCounts>>,
) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let current_card: Memo<Option<Arc<Card>>> =
        Memo::new(move |_| card_map.get().get(&card_id).cloned());

    let is_blazed = move || current_card.get().map(|c| c.blazed()).unwrap_or(false);

    let preview_data: Memo<(String, &'static str)> = Memo::new(move |_| {
        let Some(card) = current_card.get() else {
            return ("(empty)".to_string(), "card-preview empty");
        };
        let raw =
            blazelist_client_lib::display::card_preview(card.content(), 200).unwrap_or_default();
        if raw.is_empty() {
            ("(empty)".to_string(), "card-preview empty")
        } else {
            (raw, "card-preview")
        }
    });

    let number = Memo::new(move |_| {
        let (index, total) = card_positions
            .get()
            .get(&card_id)
            .copied()
            .unwrap_or((0, 0));
        let width = total.max(1).ilog10() as usize + 1;
        format!("{index:0>width$}")
    });

    let on_click = move |_| {
        let current = state.selected_card.get_untracked();
        if current == Some(card_id) {
            if !confirm_discard_changes(&state) {
                return;
            }
            state.selected_card.set(None);
            sync_query_params(&state);
        } else {
            select_card_view(&state, card_id);
        }
    };

    let card_class = move || {
        let mut cls = String::from("card-item");
        if is_blazed() {
            cls.push_str(" blazed");
        }
        if state.selected_card.get() == Some(card_id) {
            cls.push_str(" selected");
        }
        cls
    };

    let time_text = move || {
        let _ = state.tick.get();
        current_card
            .get()
            .map(|c| format_relative_time(&c.modified_at()))
            .unwrap_or_default()
    };

    let due_badge = move || {
        let _ = state.tick.get();
        current_card.get().and_then(|c| c.due_date()).map(|d| {
            let (text, class) = format_due_date_badge(&d);
            let cls = format!("card-due {class}");
            view! { <span class=cls>{text}</span> }
        })
    };

    let task_progress = move || {
        current_card
            .get()
            .and_then(|c| blazelist_client_lib::display::task_progress(c.content()))
    };

    let link_indicators = move || {
        let lc = link_counts.get().get(&card_id).copied().unwrap_or_default();
        // Read transitive count from background cache — tracked reads so the
        // indicator updates as the cache fills progressively.
        let transitive = if state.show_list_link_counts.get() {
            state
                .link_graph_cache
                .get()
                .get(&card_id)
                .map(|(_, v)| v.len().saturating_sub(lc.forward + lc.back + lc.mutual))
                .unwrap_or(0)
        } else {
            0
        };
        link_indicators_view(LinkCounts { transitive, ..lc })
    };

    let tag_dots = move || -> Option<leptos::prelude::AnyView> {
        let card = current_card.get()?;
        let tags_state = state.tags.get();

        // Split into custom-colored (shown as dots) and default-accent
        // (counted into the "+N" overflow only). A default-colored dot
        // carries no visual information and just clutters the 2×2 grid,
        // so we surface them as a compact count instead.
        let mut colored: Vec<(String, String)> = Vec::new();
        let mut uncolored_count: usize = 0;
        for tag_id in card.tags() {
            if let Some(t) = tags_state.iter().find(|t| t.id() == *tag_id) {
                match t.color() {
                    Some(c) => {
                        let hex = format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
                        colored.push((t.title().to_lowercase(), hex));
                    }
                    None => {
                        uncolored_count += 1;
                    }
                }
            }
        }

        if colored.is_empty() && uncolored_count == 0 {
            return None;
        }

        colored.sort_by(|a, b| a.0.cmp(&b.0));
        let max_visible = 4;
        let visible_colors: Vec<String> = colored
            .iter()
            .take(max_visible)
            .map(|(_, c)| c.clone())
            .collect();
        let extra_colored = colored.len().saturating_sub(max_visible);
        let overflow_total = extra_colored + uncolored_count;
        let overflow = (overflow_total > 0).then_some(overflow_total);

        Some(
            view! {
                <div class="card-tag-dots">
                    {(!visible_colors.is_empty()).then(|| view! {
                        <div class="card-tag-dots-grid">
                            {visible_colors.into_iter().map(|c| {
                                let style = format!("background: {c};");
                                view! { <span class="card-tag-dot" style=style></span> }
                            }).collect::<Vec<_>>()}
                        </div>
                    })}
                    {overflow.map(|n| view! {
                        <span class="card-tag-overflow">{format!("+{n}")}</span>
                    })}
                </div>
            }
            .into_any(),
        )
    };

    // --- Touch swipe ---
    let swipe_offset = RwSignal::new(0.0f64);
    let touch_start_x = Rc::new(Cell::new(0.0f64));
    let touch_start_y = Rc::new(Cell::new(0.0f64));
    let swiping = Rc::new(Cell::new(false));

    fn read_card(
        card_id: Uuid,
        card_map: &Memo<HashMap<Uuid, Arc<Card>>>,
        state: &AppState,
    ) -> Option<Card> {
        card_map
            .get_untracked()
            .get(&card_id)
            .map(|arc| Card::clone(arc))
            .or_else(|| {
                state
                    .cards
                    .get_untracked()
                    .into_iter()
                    .find(|c| c.id() == card_id)
            })
    }

    let on_touchstart = {
        let tsx = touch_start_x.clone();
        let tsy = touch_start_y.clone();
        let sw = swiping.clone();
        move |ev: web_sys::TouchEvent| {
            if !state.touch_swipe_enabled.get_untracked() {
                return;
            }
            if let Some(touch) = ev.touches().get(0) {
                tsx.set(touch.client_x() as f64);
                tsy.set(touch.client_y() as f64);
                sw.set(false);
                swipe_offset.set(0.0);
            }
        }
    };

    let on_touchmove = {
        let tsx = touch_start_x.clone();
        let tsy = touch_start_y.clone();
        let sw = swiping.clone();
        move |ev: web_sys::TouchEvent| {
            if !state.touch_swipe_enabled.get_untracked() {
                return;
            }
            if let Some(touch) = ev.touches().get(0) {
                let dx = touch.client_x() as f64 - tsx.get();
                let dy = touch.client_y() as f64 - tsy.get();
                if !sw.get() {
                    if dx.abs() > 10.0 && dx.abs() > dy.abs() * 1.5 {
                        sw.set(true);
                    } else {
                        return;
                    }
                }
                if sw.get() {
                    ev.prevent_default();
                    let threshold_r = state.swipe_threshold_right.get_untracked() as f64;
                    let threshold_l = state.swipe_threshold_left.get_untracked() as f64;
                    let offset = if dx > 0.0 {
                        if dx <= threshold_r {
                            dx
                        } else {
                            let extra = dx - threshold_r;
                            let brake = threshold_r * 1.2;
                            threshold_r + brake * extra / (extra + brake)
                        }
                    } else {
                        let adx = dx.abs();
                        if adx <= threshold_l {
                            dx
                        } else {
                            let extra = adx - threshold_l;
                            let brake = threshold_l * 1.2;
                            -(threshold_l + brake * extra / (extra + brake))
                        }
                    };
                    swipe_offset.set(offset);
                }
            }
        }
    };

    let on_touchend = {
        let sw = swiping.clone();
        move |_: web_sys::TouchEvent| {
            if !state.touch_swipe_enabled.get_untracked() || !sw.get() {
                swipe_offset.set(0.0);
                return;
            }
            let offset = swipe_offset.get_untracked();
            swipe_offset.set(0.0);
            sw.set(false);

            let Some(c) = read_card(card_id, &card_map, &state) else {
                return;
            };
            let threshold_r = state.swipe_threshold_right.get_untracked() as f64;
            let threshold_l = state.swipe_threshold_left.get_untracked() as f64;
            if offset > threshold_r {
                let msg = if c.blazed() {
                    "Extinguished \u{1F680}".to_string()
                } else {
                    "Blazed \u{1F525}".to_string()
                };
                let original = c.clone();
                let updated = c.next(
                    c.content().to_string(),
                    c.priority(),
                    c.tags().to_vec(),
                    !c.blazed(),
                    Utc::now(),
                    c.due_date(),
                );
                state.upsert_card(updated.clone());
                leptos::task::spawn_local(async move {
                    push_card_or_queue(&state, updated).await;
                });
                show_swipe_toast(&state, msg, original);
            } else if offset < -threshold_l {
                let today_date = blazelist_protocol::Utc::now().date_naive();
                let today = today_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                let tomorrow = today + chrono::Duration::days(1);
                let in_two = today + chrono::Duration::days(2);

                // Toggle mode: cycle based on current due date.
                let current_date = c.due_date().map(|d| d.date_naive());
                let (new_due, msg) = if current_date == Some(today_date) {
                    (Some(tomorrow), "Due: Tomorrow".to_string())
                } else if current_date == Some(today_date + chrono::Days::new(1)) {
                    (
                        Some(in_two),
                        format!(
                            "Due: {}",
                            (today_date + chrono::Days::new(2)).format("%Y-%m-%d")
                        ),
                    )
                } else if current_date == Some(today_date + chrono::Days::new(2)) {
                    (None, "Due: Cleared".to_string())
                } else {
                    (Some(today), "Due: Today".to_string())
                };
                let original = c.clone();
                let updated = c.next(
                    c.content().to_string(),
                    c.priority(),
                    c.tags().to_vec(),
                    c.blazed(),
                    Utc::now(),
                    new_due,
                );
                state.upsert_card(updated.clone());
                leptos::task::spawn_local(async move {
                    push_card_or_queue(&state, updated).await;
                });
                show_swipe_toast(&state, msg, original);
            }
        }
    };

    let swipe_style = move || {
        let offset = swipe_offset.get();
        if offset.abs() < 1.0 {
            String::new()
        } else {
            format!("transform:translateX({offset:.0}px);transition:none;")
        }
    };

    let swipe_bg_class = move || {
        let offset = swipe_offset.get();
        let threshold_r = state.swipe_threshold_right.get() as f64;
        let threshold_l = state.swipe_threshold_left.get() as f64;
        let right_kind = if is_blazed() {
            "swipe-bg-extinguish"
        } else {
            "swipe-bg-blaze"
        };
        if offset >= threshold_r {
            match right_kind {
                "swipe-bg-extinguish" => "swipe-bg swipe-bg-extinguish swipe-commit",
                _ => "swipe-bg swipe-bg-blaze swipe-commit",
            }
        } else if offset > 40.0 {
            match right_kind {
                "swipe-bg-extinguish" => "swipe-bg swipe-bg-extinguish",
                _ => "swipe-bg swipe-bg-blaze",
            }
        } else if offset <= -threshold_l {
            "swipe-bg swipe-bg-due swipe-commit"
        } else if offset < -55.0 {
            "swipe-bg swipe-bg-due"
        } else {
            "swipe-bg"
        }
    };

    let swipe_label_style = move || {
        let offset = swipe_offset.get();
        let fade = 15.0_f64;
        let opacity = if offset > 40.0 {
            ((offset - 40.0) / fade).min(1.0)
        } else if offset < -55.0 {
            ((offset.abs() - 55.0) / fade).min(1.0)
        } else {
            0.0
        };
        if opacity >= 1.0 {
            String::new()
        } else {
            format!("opacity:{opacity:.2}")
        }
    };

    let swipe_label = move || {
        let offset = swipe_offset.get();
        if offset > 40.0 {
            if is_blazed() { "Extinguish" } else { "Blaze" }
        } else if offset < -55.0 {
            let today_date = blazelist_protocol::Utc::now().date_naive();
            let current_date = current_card
                .get()
                .and_then(|c| c.due_date())
                .map(|d| d.date_naive());
            if current_date == Some(today_date) {
                "Tomorrow"
            } else if current_date == Some(today_date + chrono::Days::new(1)) {
                "In 2 days"
            } else if current_date == Some(today_date + chrono::Days::new(2)) {
                "Clear due"
            } else {
                "Today"
            }
        } else {
            ""
        }
    };

    let wrapper_class = move || {
        let mut cls = String::from("card-item-wrapper");
        if is_blazed() {
            cls.push_str(" blazed");
        }
        if state.selected_card.get() == Some(card_id) {
            cls.push_str(" selected");
        }
        cls
    };

    view! {
        <div class=wrapper_class>
            <div class=swipe_bg_class>
                <span class="swipe-label" style=swipe_label_style>{swipe_label}</span>
            </div>
            <div
                class=card_class
                style=swipe_style
                on:click=on_click
                on:touchstart=on_touchstart
                on:touchmove=on_touchmove
                on:touchend=on_touchend
            >
                <span class="card-number">{move || number.get()}</span>
                <div class=move || preview_data.get().1>
                    {move || preview_data.get().0}
                </div>
                {link_indicators}
                {tag_dots}
                {move || task_progress().map(|(done, total)| view! {
                    <span class="card-tasks">{format!("{done}/{total}")}</span>
                })}
                {due_badge}
                <span class="card-time">{time_text}</span>
            </div>
        </div>
    }
}

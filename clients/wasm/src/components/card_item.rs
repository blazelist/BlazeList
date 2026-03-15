use crate::state::store::{
    AppState, confirm_discard_changes, format_due_date_badge, format_relative_time,
    select_card_view, sync_query_params,
};
use crate::state::sync::push_card_or_queue;
use blazelist_protocol::{Card, Entity, Utc};
use leptos::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

#[component]
pub fn CardItem(
    card: Card,
    /// 1-based index of this card in the filtered list.
    index: usize,
    /// Total number of cards in the filtered list (for zero-padding width).
    total: usize,
    /// Number of forward links (this card → others).
    #[prop(default = 0)]
    link_forward: usize,
    /// Number of back links (others → this card).
    #[prop(default = 0)]
    link_back: usize,
) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");
    let card_id = card.id();
    let is_blazed = card.blazed();

    let modified_at = card.modified_at();
    let due_date = card.due_date();
    let preview_text =
        blazelist_client_lib::display::card_preview(card.content(), 200).unwrap_or_default();
    let has_content = !preview_text.is_empty();

    // Zero-padded number like TUI: width = digits in total count
    let width = total.max(1).ilog10() as usize + 1;
    let number = format!("{:0>width$}", index, width = width);

    let on_click = move |_| {
        let current = state.selected_card.get_untracked();
        if current == Some(card_id) {
            // Toggle off — deselect the current card.
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
        if is_blazed {
            cls.push_str(" blazed");
        }
        if state.selected_card.get() == Some(card_id) {
            cls.push_str(" selected");
        }
        cls
    };

    let preview_class = if has_content {
        "card-preview"
    } else {
        "card-preview empty"
    };
    let preview_display = if has_content {
        preview_text
    } else {
        "(empty)".to_string()
    };

    let time_text = move || {
        // Read tick to re-evaluate periodically
        let _ = state.tick.get();
        format_relative_time(&modified_at)
    };

    let due_badge = move || {
        let _ = state.tick.get();
        due_date.map(|d| {
            let (text, class) = format_due_date_badge(&d);
            let cls = format!("card-due {class}");
            view! { <span class=cls>{text}</span> }
        })
    };

    let task_progress = blazelist_client_lib::display::task_progress(card.content());

    let has_links = link_forward > 0 || link_back > 0;
    let link_indicators = has_links.then(|| {
        let fwd = (link_forward > 0).then(|| {
            let text = format!("\u{2192}{link_forward}");
            view! { <span class="card-link-forward">{text}</span> }
        });
        let bck = (link_back > 0).then(|| {
            let text = format!("\u{2190}{link_back}");
            view! { <span class="card-link-back">{text}</span> }
        });
        view! { <span class="card-link-indicators">{fwd}{bck}</span> }
    });

    // Collect tag colors sorted alphabetically by tag title.
    // Tags with a custom color use that color; others use the default accent.
    let tag_colors: Vec<String> = {
        let tags_state = state.tags.get_untracked();
        let mut matched: Vec<_> = card
            .tags()
            .iter()
            .filter_map(|tag_id| {
                tags_state.iter().find(|t| t.id() == *tag_id).map(|t| {
                    let color = t
                        .color()
                        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                        .unwrap_or_else(|| "var(--accent)".to_string());
                    (t.title().to_lowercase(), color)
                })
            })
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched.into_iter().map(|(_, c)| c).collect()
    };

    let tag_dots = if tag_colors.is_empty() {
        None
    } else {
        let total = tag_colors.len();
        let max_visible = 9;
        let overflow = if total > max_visible {
            Some(total - max_visible)
        } else {
            None
        };
        let visible: Vec<String> = tag_colors.into_iter().take(max_visible).collect();
        Some(view! {
            <div class="card-tag-dots">
                <div class="card-tag-dots-grid">
                    {visible.into_iter().map(|c| {
                        let style = format!("background: {c};");
                        view! { <span class="card-tag-dot" style=style></span> }
                    }).collect::<Vec<_>>()}
                </div>
                {overflow.map(|n| view! {
                    <span class="card-tag-overflow">{format!("+{n}")}</span>
                })}
            </div>
        })
    };

    // --- Touch swipe state ---
    let swipe_offset = RwSignal::new(0.0f64);
    let touch_start_x = Rc::new(Cell::new(0.0f64));
    let touch_start_y = Rc::new(Cell::new(0.0f64));
    let swiping = Rc::new(Cell::new(false));

    let stored_card = StoredValue::new(card.clone());
    let card_due = card.due_date();

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
                // Only start swiping if horizontal movement dominates
                if !sw.get() {
                    if dx.abs() > 10.0 && dx.abs() > dy.abs() * 1.5 {
                        sw.set(true);
                    } else if dy.abs() > 10.0 {
                        return;
                    } else {
                        return;
                    }
                }
                if sw.get() {
                    ev.prevent_default();
                    // Clamp to reasonable range
                    swipe_offset.set(dx.clamp(-120.0, 120.0));
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

            const THRESHOLD: f64 = 60.0;
            if offset > THRESHOLD {
                // Swipe right → blaze/extinguish
                let c = stored_card.get_value();
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
            } else if offset < -THRESHOLD {
                // Swipe left → set due date to today (or tomorrow if already today)
                let c = stored_card.get_value();
                let today = blazelist_protocol::Utc::now()
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
                let new_due = if card_due == Some(today) {
                    let tomorrow = today + chrono::Duration::days(1);
                    Some(tomorrow)
                } else {
                    Some(today)
                };
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
        if offset > 40.0 {
            "swipe-bg swipe-bg-blaze"
        } else if offset < -40.0 {
            "swipe-bg swipe-bg-due"
        } else {
            "swipe-bg"
        }
    };

    let swipe_label = move || {
        let offset = swipe_offset.get();
        if offset > 40.0 {
            if is_blazed { "Extinguish" } else { "Blaze" }
        } else if offset < -40.0 {
            if card_due == Some(
                blazelist_protocol::Utc::now()
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc(),
            ) {
                "Tomorrow"
            } else {
                "Today"
            }
        } else {
            ""
        }
    };

    view! {
        <div class="card-item-wrapper">
            <div class=swipe_bg_class>
                <span class="swipe-label">{swipe_label}</span>
            </div>
            <div
                class=card_class
                style=swipe_style
                on:click=on_click
                on:touchstart=on_touchstart
                on:touchmove=on_touchmove
                on:touchend=on_touchend
            >
                <span class="card-number">{number}</span>
                <div class=preview_class>{preview_display}</div>
                {link_indicators}
                {tag_dots}
                {task_progress.map(|(done, total)| view! {
                    <span class="card-tasks">{format!("{done}/{total}")}</span>
                })}
                {due_badge}
                <span class="card-time">{time_text}</span>
            </div>
        </div>
    }
}

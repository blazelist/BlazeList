use crate::components::card_list::DragState;
use crate::state::store::{
    AppState, confirm_discard_changes, format_due_date_badge, format_relative_time,
    sync_query_params,
};
use blazelist_protocol::{Card, Entity};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

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
    let drag_state = use_context::<DragState>().expect("DragState not provided");
    let card_id = card.id();
    let card_id_str = card_id.to_string();
    let is_blazed = card.blazed();
    // 0-based index in the filtered list
    let list_index = index - 1;

    let modified_at = card.modified_at();
    let due_date = card.due_date();
    let preview_text =
        blazelist_client_lib::display::card_preview(card.content(), 200).unwrap_or_default();
    let has_content = !preview_text.is_empty();

    // Zero-padded number like TUI: width = digits in total count
    let width = total.max(1).ilog10() as usize + 1;
    let number = format!("{:0>width$}", index, width = width);

    let on_click = move |_| {
        if !confirm_discard_changes(&state) {
            return;
        }
        let current = state.selected_card.get_untracked();
        if current == Some(card_id) {
            state.selected_card.set(None);
        } else {
            state.selected_card.set(Some(card_id));
            state.editing.set(false);
            state.creating_new.set(false);
            state.creating_new_tag.set(false);
            state.settings_open.set(false);
        }
        sync_query_params(&state);
    };

    let card_class = move || {
        let mut cls = String::from("card-item");
        if is_blazed {
            cls.push_str(" blazed");
        }
        if state.selected_card.get() == Some(card_id) {
            cls.push_str(" selected");
        }
        // Show drop indicator above or below this card
        if let Some(target) = drag_state.drop_target_index.get() {
            let dragged = drag_state.dragged_card_id.get();
            if !dragged.is_empty() {
                if target == list_index {
                    cls.push_str(" drop-above");
                } else if target == list_index + 1 {
                    cls.push_str(" drop-below");
                }
            }
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

    // Drag-and-drop handlers (only active when setting enabled)
    let on_dragstart = {
        let card_id_str = card_id_str.clone();
        move |ev: web_sys::DragEvent| {
            if !state.drag_drop_reorder.get_untracked() {
                ev.prevent_default();
                return;
            }
            if let Some(dt) = ev.data_transfer() {
                let _ = dt.set_data("text/plain", &card_id_str);
                let _ = dt.set_drop_effect("move");
            }
            drag_state.dragged_card_id.set(card_id_str.clone());
        }
    };

    let on_dragend = move |_: web_sys::DragEvent| {
        drag_state.dragged_card_id.set(String::new());
        drag_state.drop_target_index.set(None);
    };

    let on_dragover = move |ev: web_sys::DragEvent| {
        if !state.drag_drop_reorder.get_untracked() {
            return;
        }
        ev.prevent_default();
        // Determine if we're in the top or bottom half of the card
        if let Some(target) = ev.current_target() {
            let el: web_sys::Element = target.unchecked_into();
            let rect = el.get_bounding_client_rect();
            let mid = rect.top() + rect.height() / 2.0;
            let y = ev.client_y() as f64;
            if y < mid {
                drag_state.drop_target_index.set(Some(list_index));
            } else {
                drag_state.drop_target_index.set(Some(list_index + 1));
            }
        }
    };

    let is_draggable = move || state.drag_drop_reorder.get();

    view! {
        <div
            class=card_class
            on:click=on_click
            draggable=move || if is_draggable() { "true" } else { "false" }
            on:dragstart=on_dragstart
            on:dragend=on_dragend
            on:dragover=on_dragover
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
    }
}

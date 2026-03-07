use blazelist_client_lib::filter::{DueDateFilter, SortOrder, TagFilterMode};
use blazelist_protocol::CardFilter;
use uuid::Uuid;

use super::store::AppState;

pub fn get_query_params() -> web_sys::UrlSearchParams {
    let window = web_sys::window().unwrap();
    let search = window.location().search().unwrap_or_default();
    web_sys::UrlSearchParams::new_with_str(&search).unwrap()
}

pub fn parse_filter_from_params(params: &web_sys::UrlSearchParams) -> CardFilter {
    match params.get("f.status").as_deref() {
        Some("all") => CardFilter::All,
        Some("blazed") => CardFilter::Blazed,
        Some("extinguished") => CardFilter::Extinguished,
        _ => CardFilter::Extinguished,
    }
}

pub fn parse_due_date_filter_from_params(params: &web_sys::UrlSearchParams) -> DueDateFilter {
    match params.get("f.due").as_deref() {
        Some("overdue") => DueDateFilter::Overdue,
        Some("today") => DueDateFilter::Today,
        Some("upcoming") => DueDateFilter::Upcoming,
        _ => DueDateFilter::All,
    }
}

fn due_date_filter_to_str(f: DueDateFilter) -> &'static str {
    match f {
        DueDateFilter::All => "all",
        DueDateFilter::Overdue => "overdue",
        DueDateFilter::Today => "today",
        DueDateFilter::Upcoming => "upcoming",
    }
}

pub fn parse_tag_mode_from_params(params: &web_sys::UrlSearchParams) -> TagFilterMode {
    match params.get("f.tag_mode").as_deref() {
        Some("and") => TagFilterMode::And,
        _ => TagFilterMode::Or,
    }
}

pub fn parse_tags_from_params(params: &web_sys::UrlSearchParams) -> Vec<Uuid> {
    let all = params.get_all("f.tag");
    let mut tags = Vec::new();
    for i in 0..all.length() {
        if let Some(s) = all.get(i).as_string() {
            if let Ok(id) = s.parse::<Uuid>() {
                tags.push(id);
            }
        }
    }
    tags
}

pub fn parse_no_tags_from_params(params: &web_sys::UrlSearchParams) -> bool {
    params.get("f.no_tags").as_deref() == Some("1")
}

pub fn parse_selected_card_from_params(params: &web_sys::UrlSearchParams) -> Option<Uuid> {
    params.get("card").and_then(|s| s.parse::<Uuid>().ok())
}

pub fn parse_linked_cards_from_params(params: &web_sys::UrlSearchParams) -> Vec<Uuid> {
    let all = params.get_all("f.linked");
    let mut links = Vec::new();
    for i in 0..all.length() {
        if let Some(s) = all.get(i).as_string() {
            if let Ok(id) = s.parse::<Uuid>() {
                links.push(id);
            }
        }
    }
    links
}

pub fn parse_sort_from_params(params: &web_sys::UrlSearchParams) -> SortOrder {
    params
        .get("f.sort")
        .map(|s| SortOrder::from_url_value(&s))
        .unwrap_or_default()
}

fn filter_to_str(f: CardFilter) -> &'static str {
    match f {
        CardFilter::All => "all",
        CardFilter::Blazed => "blazed",
        CardFilter::Extinguished => "extinguished",
    }
}

/// Push current filter state to the URL query string without reloading.
pub fn sync_query_params(state: &AppState) {
    use leptos::prelude::*;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let params = web_sys::UrlSearchParams::new().unwrap();

    let filter = state.filter.get_untracked();
    if filter != CardFilter::Extinguished {
        params.set("f.status", filter_to_str(filter));
    }

    let due = state.due_date_filter.get_untracked();
    if due != DueDateFilter::All {
        params.set("f.due", due_date_filter_to_str(due));
    }

    let sort = state.sort_order.get_untracked();
    if let Some(val) = sort.url_value() {
        params.set("f.sort", val);
    }

    let mode = state.tag_filter_mode.get_untracked();
    if mode == TagFilterMode::And {
        params.set("f.tag_mode", "and");
    }

    if state.no_tags_filter.get_untracked() {
        params.set("f.no_tags", "1");
    }

    let tags = state.tag_filter.get_untracked();
    for tag_id in &tags {
        params.append("f.tag", &tag_id.to_string());
    }

    if let Some(card_id) = state.selected_card.get_untracked() {
        params.set("card", &card_id.to_string());
    }

    let links = state.linked_card_filter.get_untracked();
    for link_id in &links {
        params.append("f.linked", &link_id.to_string());
    }

    let qs = params.to_string().as_string().unwrap_or_default();
    let new_url = if qs.is_empty() {
        window.location().pathname().unwrap_or_default()
    } else {
        format!(
            "{}?{}",
            window.location().pathname().unwrap_or_default(),
            qs
        )
    };

    let _ = window.history().unwrap().replace_state_with_url(
        &wasm_bindgen::JsValue::NULL,
        "",
        Some(&new_url),
    );
}

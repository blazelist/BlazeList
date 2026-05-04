//! Reusable date/time display component.
//!
//! All timestamps across the UI (card/tag metadata, version history, due
//! dates, sequence history) should render through this component so that
//! format tweaks, timezone handling, and tooltip behaviour stay consistent
//! in one place.

use crate::state::store::format_relative_time;
use blazelist_protocol::{DateTime, Utc};
use leptos::prelude::*;

/// Canonical full-UTC display format used for exact timestamps.
pub const FULL_UTC_FORMAT: &str = "%Y-%m-%d %H:%M:%S UTC";

/// Format a [`DateTime<Utc>`] as the canonical full UTC string.
pub fn format_full_utc(dt: &DateTime<Utc>) -> String {
    dt.format(FULL_UTC_FORMAT).to_string()
}

/// Render a timestamp as a `<span>`.
///
/// By default this shows the full UTC timestamp in the canonical format.
/// Pass `relative=true` to show a relative time (e.g. "3 minutes ago"),
/// in which case the full UTC timestamp becomes the `title` tooltip so the
/// user can still see the exact value on hover.
#[component]
pub fn Timestamp(
    /// The instant to render.
    datetime: DateTime<Utc>,
    /// When true, display as relative time with the full value in the tooltip.
    #[prop(optional)]
    relative: bool,
    /// CSS class applied to the `<span>`.
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    let full = format_full_utc(&datetime);
    let text = if relative {
        format_relative_time(&datetime)
    } else {
        full.clone()
    };
    view! {
        <span class=class title=full>{text}</span>
    }
}

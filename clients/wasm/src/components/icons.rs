//! Inline SVG icons for the app's button affordances.
//!
//! Drawn rather than written as characters. The glyphs these replace fall into
//! two traps: some are emoji-capable, so a platform whose emoji font wins the
//! fallback renders them in colour; others (`⧉`, `⤒`, `⤓`, `⛶`, `⤢`, `↻`, `⋯`)
//! are absent from the symbol fonts lean devices ship, leaving tofu where a
//! button should be. A drawn path takes the font out of the question.
//!
//! Every icon is a 24x24 `viewBox` carrying the `icon` class, which supplies the
//! sizing and `currentColor` stroke (see `style/base.css`) so each one follows
//! the font-size and colour of whatever it sits in, hover states included.

use leptos::prelude::*;

/// Settings gear.
#[component]
pub fn IconGear() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="12" cy="12" r="6.6" />
            <circle cx="12" cy="12" r="2.8" />
            <path class="gear-teeth" d="M12.00 5.50L12.00 3.00M17.63 8.75L19.79 7.50M17.63 15.25L19.79 16.50M12.00 18.50L12.00 21.00M6.37 15.25L4.21 16.50M6.37 8.75L4.21 7.50" />
        </svg>
    }
}

/// Dismiss / close.
#[component]
pub fn IconClose() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M7 7l10 10M17 7L7 17" />
        </svg>
    }
}

/// Rename / edit.
#[component]
pub fn IconPencil() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M3.5 20.5L5 16L16 5L19 8L8 19Z" />
            <path d="M14.2 6.8l3 3" />
        </svg>
    }
}

/// Sidebar toggle.
#[component]
pub fn IconMenu() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M5 5.5h14M5 12h14M5 18.5h14" />
        </svg>
    }
}

/// Copy to clipboard.
#[component]
pub fn IconCopy() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <rect x="4" y="4" width="13" height="13" rx="1.5" />
            <rect x="7" y="7" width="13" height="13" rx="1.5" />
        </svg>
    }
}

// The chevrons span the close cross's 7-17 band: the three share the detail
// header's action row and need to read at one height.

/// Step to the previous card.
#[component]
pub fn IconPrev() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M14.5 7l-5 5 5 5" />
        </svg>
    }
}

/// Step to the next card.
#[component]
pub fn IconNext() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M9.5 7l5 5-5 5" />
        </svg>
    }
}

/// Expand the detail panel to fullscreen.
#[component]
pub fn IconFullscreen() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M5 9.5V5h4.5M14.5 5H19v4.5M19 14.5V19h-4.5M9.5 19H5v-4.5" />
        </svg>
    }
}

/// Collapse the detail panel back to the side.
#[component]
pub fn IconCollapse() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M6 18L18 6M18 6h-4.6M18 6v4.6M6 18h4.6M6 18v-4.6" />
        </svg>
    }
}

/// Move the card to the top of the list.
#[component]
pub fn IconMoveTop() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M6.5 3.5h11M12 20.5V7.5M7.2 12.3L12 7.5l4.8 4.8" />
        </svg>
    }
}

/// Move the card up one position.
#[component]
pub fn IconMoveUp() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 20.5V3.5M6.4 9.1L12 3.5l5.6 5.6" />
        </svg>
    }
}

/// Move the card down one position.
#[component]
pub fn IconMoveDown() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 3.5v17M6.4 14.9L12 20.5l5.6-5.6" />
        </svg>
    }
}

/// Move the card to the bottom of the list.
#[component]
pub fn IconMoveBottom() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M6.5 20.5h11M12 3.5v13M7.2 11.7L12 16.5l4.8-4.8" />
        </svg>
    }
}

/// Sync now.
#[component]
pub fn IconRefresh() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M16.31 6.48A7 7 0 1 1 7.69 6.48" />
            <path d="M7.18 10.65L7.69 6.48L3.52 5.97" />
        </svg>
    }
}

/// A link this card points at.
#[component]
pub fn IconLinkForward() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M3 12h18M15.6 6.6L21 12l-5.4 5.4" />
        </svg>
    }
}

/// A link pointing at this card.
#[component]
pub fn IconLinkBack() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M21 12H3M8.4 17.4L3 12l5.4-5.4" />
        </svg>
    }
}

/// A pair of cards pointing at each other.
#[component]
pub fn IconLinkMutual() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M3 12h18M7.2 7.8L3 12l4.2 4.2M16.8 7.8L21 12l-4.2 4.2" />
        </svg>
    }
}

/// A link reached through another card rather than directly.
#[component]
pub fn IconLinkTransitive() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path class="link-dots" d="M5.5 12h.01M12 12h.01M18.5 12h.01" />
        </svg>
    }
}

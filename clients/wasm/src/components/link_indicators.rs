use blazelist_client_lib::display::LinkCounts;
use leptos::prelude::*;

/// A horizontal row of link-count indicators (mutual / forward / back /
/// transitive) used by both the card list and the card detail summary.
///
/// Returns `None` when all counts are zero so callers can omit the wrapper
/// entirely. Order is: mutual, forward, back, transitive.
pub fn link_indicators_view(counts: LinkCounts) -> Option<AnyView> {
    let LinkCounts {
        forward,
        back,
        mutual,
        transitive,
    } = counts;
    if forward == 0 && back == 0 && mutual == 0 && transitive == 0 {
        return None;
    }

    let mut_view = (mutual > 0).then(|| {
        let text = format!("\u{2194}{mutual}");
        let tip = format!("{mutual} mutual link{}", plural(mutual));
        view! { <span class="card-link-mutual" title=tip>{text}</span> }
    });
    let fwd_view = (forward > 0).then(|| {
        let text = format!("\u{2192}{forward}");
        let tip = format!("{forward} forward link{}", plural(forward));
        view! { <span class="card-link-forward" title=tip>{text}</span> }
    });
    let back_view = (back > 0).then(|| {
        let text = format!("\u{2190}{back}");
        let tip = format!("{back} back link{}", plural(back));
        view! { <span class="card-link-back" title=tip>{text}</span> }
    });
    let trans_view = (transitive > 0).then(|| {
        let text = format!("\u{22EF}{transitive}");
        let tip = format!("{transitive} transitive link{}", plural(transitive));
        view! { <span class="card-link-transitive" title=tip>{text}</span> }
    });

    Some(
        view! {
            <span class="card-link-indicators">
                {mut_view}{fwd_view}{back_view}{trans_view}
            </span>
        }
        .into_any(),
    )
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

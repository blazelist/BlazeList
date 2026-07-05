use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::components::card_detail::CardDetail;
use crate::components::card_list::CardList;
use crate::components::filter_bar::FilterBar;
use crate::components::header::Header;
use crate::components::keyboard::{ShortcutsPanel, SubMenuPopup};
use crate::components::settings_panel::SettingsPanel;
use crate::components::tag_sidebar::TagSidebar;
use crate::components::toast::{CopyToast, ErrorToast, SwipeToastBar};
use crate::state::store::AppState;

/// Start a column resize drag operation.
///
/// Attaches global mousemove/mouseup listeners to the document. On mousemove,
/// updates the `width` signal. On mouseup, removes listeners and cleans up.
///
/// `direction`: `"right"` means dragging right increases width (sidebar),
/// `"left"` means dragging left increases width (detail panel).
fn start_resize(
    start_x: f64,
    start_width: f64,
    width: RwSignal<f64>,
    min_w: f64,
    max_w: f64,
    direction: &'static str,
) {
    let document = web_sys::window().unwrap().document().unwrap();
    let doc = document.clone();

    // We store the closures in leaked boxes so they stay alive for the
    // duration of the drag. The mouseup handler cleans them up.
    let move_cb: *mut Option<Closure<dyn FnMut(web_sys::MouseEvent)>> =
        Box::into_raw(Box::new(None));
    let up_cb: *mut Option<Closure<dyn FnMut(web_sys::MouseEvent)>> = Box::into_raw(Box::new(None));

    let on_move = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
        let delta = ev.client_x() - start_x;
        let new_width = if direction == "right" {
            start_width + delta
        } else {
            start_width - delta
        };
        width.set(new_width.clamp(min_w, max_w));
    }) as Box<dyn FnMut(web_sys::MouseEvent)>);

    let on_up = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
        // SAFETY: these pointers are valid — they were created above and only
        // freed here, exactly once, when mouseup fires.
        unsafe {
            if let Some(cb) = (*move_cb).as_ref() {
                let _ = doc
                    .remove_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref());
            }
            if let Some(cb) = (*up_cb).as_ref() {
                let _ =
                    doc.remove_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref());
            }
            // Drop the closures and free the boxes
            drop(Box::from_raw(move_cb));
            drop(Box::from_raw(up_cb));
        }
    }) as Box<dyn FnMut(web_sys::MouseEvent)>);

    let _ =
        document.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref());
    let _ = document.add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref());

    // Store the closures so they aren't dropped yet
    unsafe {
        *move_cb = Some(on_move);
        *up_cb = Some(on_up);
    }
}

#[component]
pub fn Home() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    // Memo ensures the DynChild only re-renders when visibility truly
    // changes (false↔true), not when underlying signals change while
    // the panel stays open.
    let detail_open = Memo::new(move |_| {
        state.selected_card().get().is_some()
            || state.creating_new().get()
            || state.creating_new_tag().get()
            || state.settings_open.get()
            || state.shortcuts_open.get()
    });
    let sidebar_visible = move || state.sidebar_visible.get();

    let sidebar_style = move || {
        format!(
            "width:{}px;min-width:0;flex-shrink:0",
            state.sidebar_width.get() as i32
        )
    };
    // When the detail panel is in expanded (fullscreen) mode it takes
    // the full main-layout area via absolute positioning, so skip the
    // fixed-width inline style entirely — otherwise it fights the CSS.
    let detail_style = move || {
        if state.detail_expanded.get() {
            String::new()
        } else {
            format!(
                "width:{}px;min-width:0;flex-shrink:0",
                state.detail_width.get() as i32
            )
        }
    };
    let detail_panel_class = move || {
        if state.detail_expanded.get() {
            "detail-panel expanded"
        } else {
            "detail-panel"
        }
    };

    let close_sidebar_mobile = move |_| {
        state.sidebar_visible.set(false);
    };

    // Only mark the container as "detail-expanded" when the detail panel
    // is both visible AND in expanded mode. That way the sidebar toggle
    // (which the `.detail-expanded` CSS rule hides) stays available
    // whenever the user is on the card list with nothing open.
    let app_container_class = move || {
        if state.detail_expanded.get() && detail_open.get() {
            "app-container detail-expanded"
        } else {
            "app-container"
        }
    };

    view! {
        <div class=app_container_class>
            <Header />
            <SubMenuPopup />
            <div class="main-layout">
                <div class=move || {
                    if sidebar_visible() { "sidebar-overlay visible" } else { "sidebar-overlay" }
                } on:click=close_sidebar_mobile />
                <aside
                    class=move || {
                        if sidebar_visible() { "tag-sidebar" } else { "tag-sidebar sidebar-hidden" }
                    }
                    style=sidebar_style
                >
                    <TagSidebar />
                </aside>
                {move || sidebar_visible().then(|| view! {
                    <div class="resize-handle"
                        on:mousedown=move |ev: web_sys::MouseEvent| {
                            ev.prevent_default();
                            start_resize(
                                ev.client_x(),
                                state.sidebar_width.get_untracked(),
                                state.sidebar_width,
                                80.0, 500.0,
                                "right",
                            );
                        }
                    />
                })}
                <main class="content">
                    <FilterBar />
                    <CardList />
                </main>
                {move || detail_open.get().then(|| view! {
                    {move || (!state.detail_expanded.get()).then(|| view! {
                        <div class="resize-handle"
                            on:mousedown=move |ev: web_sys::MouseEvent| {
                                ev.prevent_default();
                                start_resize(
                                    ev.client_x(),
                                    state.detail_width.get_untracked(),
                                    state.detail_width,
                                    200.0, 1400.0,
                                    "left",
                                );
                            }
                        />
                    })}
                    <aside class=detail_panel_class style=detail_style>
                        {move || if state.settings_open.get() {
                            view! { <SettingsPanel /> }.into_any()
                        } else if state.shortcuts_open.get() {
                            view! { <ShortcutsPanel /> }.into_any()
                        } else {
                            view! { <CardDetail /> }.into_any()
                        }}
                    </aside>
                })}
                <SwipeToastBar />
                <CopyToast />
                <ErrorToast />
            </div>
        </div>
    }
}

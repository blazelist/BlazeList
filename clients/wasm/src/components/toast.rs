use crate::state::store::AppState;
use crate::state::sync::push_card_or_queue;
use blazelist_protocol::Utc;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "clearTimeout")]
    fn clear_timeout_js(handle: i32);
}

/// Dismiss the active swipe toast: clear its JS timeout and hide it.
fn dismiss_toast(state: &AppState) {
    if let Some(toast) = state.swipe_toast.get_untracked() {
        clear_timeout_js(toast.timeout_handle);
    }
    state.swipe_toast.set(None);
}

/// Undo the last swipe action by reverting the card to its pre-swipe state.
fn undo_swipe(state: &AppState) {
    let toast = match state.swipe_toast.get_untracked() {
        Some(t) => t,
        None => return,
    };
    clear_timeout_js(toast.timeout_handle);

    // Create a new version that restores the original card's values.
    let orig = toast.original_card;
    let reverted = orig.next(
        orig.content().to_string(),
        orig.priority(),
        orig.tags().to_vec(),
        orig.blazed(),
        Utc::now(),
        orig.due_date(),
    );
    state.upsert_card(reverted.clone());

    let s = *state;
    leptos::task::spawn_local(async move {
        crate::state::pending_priority::flush_now(&s).await;
        push_card_or_queue(&s, reverted).await;
    });

    state.swipe_toast.set(None);
}

/// Show a prominent error toast that auto-dismisses.
///
/// Writes to the [`AppState::error_toast`] signal which the
/// [`ErrorToast`] component renders with a distinct, more-visible
/// style than the neutral [`CopyToast`] (used for "Copied ID" feedback).
/// Use this for failures the user needs to notice: offline action
/// attempts, server rejections, etc.
pub fn show_error_toast(state: AppState, msg: &str, duration_ms: i32) {
    state.error_toast.set(Some(msg.to_string()));
    let cb = Closure::once(move || {
        state.error_toast.set(None);
    });
    let func = cb.into_js_value();
    let _ = web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(func.unchecked_ref(), duration_ms);
}

/// Brief toast notification that auto-dismisses (e.g. "Copied ID").
#[component]
pub fn CopyToast() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    move || {
        state.copy_toast.get().map(|msg| {
            view! {
                <div class="copy-toast">{msg}</div>
            }
        })
    }
}

/// Prominent error toast (e.g. "Can't delete cards while offline").
///
/// Larger and more visually distinct than [`CopyToast`] — uses the
/// danger accent so the user actually notices failed actions.
#[component]
pub fn ErrorToast() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    move || {
        state.error_toast.get().map(|msg| {
            view! {
                <div class="error-toast">{msg}</div>
            }
        })
    }
}

/// Toast notification bar shown after a swipe action.
///
/// Displays the action description and an "Undo" button. Auto-dismisses
/// after the timeout set by the swipe handler.
#[component]
pub fn SwipeToastBar() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState not provided");

    let on_undo = move |_: web_sys::MouseEvent| {
        undo_swipe(&state);
    };

    let on_dismiss = move |_: web_sys::MouseEvent| {
        dismiss_toast(&state);
    };

    move || {
        state.swipe_toast.get().map(|toast| {
            view! {
                <div class="swipe-toast">
                    <span class="swipe-toast-msg">{toast.message.clone()}</span>
                    <button class="swipe-toast-undo" on:click=on_undo aria-label="Undo">"Undo"</button>
                    <button class="swipe-toast-close" on:click=on_dismiss aria-label="Dismiss">"\u{2715}"</button>
                </div>
            }
        })
    }
}

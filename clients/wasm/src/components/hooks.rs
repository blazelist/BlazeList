use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// Toggle an expand/collapse signal: sets to `Some(id)` if not already
/// that value, or back to `None` if it is.
pub fn toggle_expanded<T: PartialEq + Copy + Send + Sync + 'static>(
    expanded: RwSignal<Option<T>>,
    id: T,
) {
    expanded.update(|current| {
        if *current == Some(id) {
            *current = None;
        } else {
            *current = Some(id);
        }
    });
}

/// Close a dropdown when the user clicks outside its container element.
///
/// Registers a global `click` listener when `open` becomes `true` and removes
/// it on cleanup or when the dropdown closes.
pub fn use_click_outside_close(open: RwSignal<bool>, container_ref: NodeRef<leptos::html::Div>) {
    Effect::new(move |_| {
        if !open.get() {
            return;
        }

        let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if let Some(container) = container_ref.get() {
                let el: &web_sys::Element = &container;
                if let Some(target) = ev.target() {
                    let target_node: web_sys::Node = target.unchecked_into();
                    if !el.contains(Some(&target_node)) {
                        open.set(false);
                    }
                }
            }
        });

        let window = web_sys::window().unwrap();
        let _ = window.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
        let cb_ref = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
        cb.forget();

        on_cleanup(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback("click", &cb_ref);
            }
        });
    });
}

/// Handle a click event on a code-block copy button (`.code-copy-btn`).
///
/// If the click target is (or is inside) a `.code-copy-btn`, finds the
/// parent `.code-block-wrapper`, reads the `<pre>` text content, and
/// copies it to the clipboard.  Returns `true` when a copy was handled
/// so the caller can short-circuit further event processing.
pub fn handle_code_copy_click(ev: &web_sys::MouseEvent) -> bool {
    let target = match ev.target() {
        Some(t) => t,
        None => return false,
    };
    let el = match target.dyn_into::<web_sys::HtmlElement>() {
        Ok(el) => el,
        Err(_) => return false,
    };
    if let Ok(Some(btn)) = el.closest(".code-copy-btn") {
        if let Ok(Some(wrapper)) = btn.closest(".code-block-wrapper") {
            if let Ok(Some(pre)) = wrapper.query_selector("pre") {
                let text = pre.text_content().unwrap_or_default();
                // CommonMark renders a trailing newline inside <code> — remove it
                let text = text.strip_suffix('\n').unwrap_or(&text);
                if let Some(w) = web_sys::window() {
                    let clipboard = w.navigator().clipboard();
                    let _ = clipboard.write_text(&text);
                }
            }
        }
        return true;
    }
    false
}

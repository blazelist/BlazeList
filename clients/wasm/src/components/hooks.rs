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

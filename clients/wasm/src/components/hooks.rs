use leptos::prelude::*;
use rgb::RGB8;
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
        if let Ok(Some(wrapper)) = btn.closest(".code-block-wrapper")
            && let Ok(Some(pre)) = wrapper.query_selector("pre")
        {
            let text = pre.text_content().unwrap_or_default();
            // CommonMark renders a trailing newline inside <code> — remove it
            let text = text.strip_suffix('\n').unwrap_or(&text);
            if let Some(w) = web_sys::window() {
                let clipboard = w.navigator().clipboard();
                let _ = clipboard.write_text(text);
            }
        }
        return true;
    }
    false
}

/// Parse a `#RRGGBB` hex string into an [`RGB8`] color.
///
/// Returns `None` if the input is not a valid 6-digit hex color.
pub fn parse_hex_color(hex: &str) -> Option<RGB8> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    match (
        u8::from_str_radix(&hex[0..2], 16),
        u8::from_str_radix(&hex[2..4], 16),
        u8::from_str_radix(&hex[4..6], 16),
    ) {
        (Ok(r), Ok(g), Ok(b)) => Some(RGB8::new(r, g, b)),
        _ => None,
    }
}

/// A color picker row for editing a tag's color.
///
/// Renders the color input, a preview swatch, the hex value, and a "Clear"
/// button when a color is active.  The caller owns the signals; this component
/// is purely a view over them.
#[component]
pub fn TagColorPicker(
    /// Signal holding the current hex color string (e.g. `"#808080"`).
    color_input: RwSignal<String>,
    /// Signal indicating whether the color is enabled (true) or cleared.
    use_color: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="tag-color-section">
            <span class="tag-color-label">"Color"</span>
        </div>
        <div class="tag-color-row">
            <input
                class="tag-color-input"
                type="color"
                prop:value=move || color_input.get()
                on:input=move |ev| {
                    color_input.set(event_target_value(&ev));
                    use_color.set(true);
                }
            />
            <span
                class=move || if use_color.get() { "tag-color-preview" } else { "tag-color-preview tag-color-placeholder" }
                style=move || format!("background: {};", color_input.get())
            ></span>
            {move || use_color.get().then(|| {
                let hex = color_input.get();
                view! {
                    <span class="tag-color-hex">{hex}</span>
                }
            })}
            {move || use_color.get().then(|| view! {
                <button class="btn-cancel tag-color-btn" on:click=move |_| {
                    use_color.set(false);
                    color_input.set(String::from("#808080"));
                }>"Clear"</button>
            })}
        </div>
    }
}

/// A two-step delete confirmation prompt.
///
/// Renders nothing when `step == 0`.  Shows a first confirmation prompt at
/// `step == 1` and a permanent-action warning at `step == 2`.
///
/// The caller is responsible for showing the initial delete trigger button
/// (which should set `step` to `1`).  The `entity_label` closure returns the
/// human-readable label shown in the permanent-deletion warning, e.g.
/// `"Card: My shopping list"` or `"Tag: Groceries"`.
#[component]
pub fn ConfirmDeletePrompt(
    /// The current confirmation step (0 = inactive, 1 = first prompt, 2 = permanent).
    step: RwSignal<u8>,
    /// Closure returning the text shown in the step-1 prompt, e.g.
    /// `|| "Delete?".to_string()`. Taking a closure (rather than an
    /// `&'static str`) lets the same prompt primitive carry dynamic
    /// messages.
    first_prompt: impl Fn() -> String + Copy + Send + Sync + 'static,
    /// Returns the entity description displayed in the permanent-deletion warning.
    entity_label: impl Fn() -> String + Copy + Send + Sync + 'static,
    /// Called when the user confirms permanent deletion.
    on_confirm: impl Fn() + Copy + Send + Sync + 'static,
    /// Called when the user cancels at any step.
    on_cancel: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        {move || match step.get() {
            2 => Some(view! {
                <div class="confirm-delete-permanent">
                    <span class="confirm-text-permanent">"This action is permanent and cannot be undone."</span>
                    <span class="confirm-entity-info">{entity_label()}</span>
                    <div class="confirm-permanent-buttons">
                        <button class="btn-confirm-permanent" on:click=move |_| on_confirm()>"Delete permanently"</button>
                        <button class="btn-confirm-no" on:click=move |_| on_cancel()>"Cancel"</button>
                    </div>
                </div>
            }.into_any()),
            1 => Some(view! {
                <div class="confirm-delete">
                    <span class="confirm-text">{first_prompt()}</span>
                    <button class="btn-confirm-yes" on:click=move |_| step.set(2)>"Yes"</button>
                    <button class="btn-confirm-no" on:click=move |_| on_cancel()>"No"</button>
                </div>
            }.into_any()),
            _ => None,
        }}
    }
}

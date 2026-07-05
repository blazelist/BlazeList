pub mod client;
pub mod connection;
pub mod wire;

use wasm_bindgen::JsValue;

/// Convert a `JsValue` to a human-readable string for error reporting.
///
/// Prefers the value's own string representation, falling back to a JSON
/// stringification and finally to the `Debug` formatting.
pub(crate) fn js_value_to_string(val: &JsValue) -> String {
    val.as_string()
        .or_else(|| {
            js_sys::JSON::stringify(val)
                .ok()
                .and_then(|s| s.as_string())
        })
        .unwrap_or_else(|| format!("{val:?}"))
}

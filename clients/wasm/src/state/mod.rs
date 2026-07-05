// `settings`, `pending_priority`, and `drag` are the host-buildable
// submodules: their unit tests (settings migration / validation,
// priority-burst coalescing, drag-position arithmetic) are pure logic
// and run under `cargo test`. The rest of the state module pulls in
// `transport`, `storage`, and Leptos runtime types that only the wasm
// target brings in (see main.rs).
#[cfg(any(target_arch = "wasm32", test))]
pub mod drag;
#[cfg(any(target_arch = "wasm32", test))]
pub mod pending_priority;
#[cfg(any(target_arch = "wasm32", test))]
pub mod settings;

#[cfg(target_arch = "wasm32")]
pub mod query_params;
#[cfg(target_arch = "wasm32")]
pub mod store;
#[cfg(target_arch = "wasm32")]
pub mod sync;

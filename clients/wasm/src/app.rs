use leptos::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::components::keyboard::{KeyboardHelp, register_keyboard_shortcuts};
use crate::pages::home::Home;
use crate::state::settings;
use crate::state::store::{AppState, ConnectionStatus, get_client, restore_from_query_params, set_client};
use crate::state::sync::{incremental_sync, initial_sync, load_local_state, run_subscription};
use crate::storage;
use crate::transport::client::Client;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setInterval")]
    fn set_interval_js(handler: &js_sys::Function, timeout: i32) -> i32;
}

thread_local! {
    /// Set by browser event listeners (`online`, `visibilitychange`) to
    /// interrupt the retry countdown and reconnect immediately.
    static RECONNECT_NOW: Cell<bool> = const { Cell::new(false) };
}

/// Request an immediate reconnection attempt, interrupting the retry countdown.
pub fn request_reconnect() {
    RECONNECT_NOW.with(|f| f.set(true));
}

fn take_reconnect_request() -> bool {
    RECONNECT_NOW.with(|f| f.replace(false))
}

/// Root application component.
///
/// Provides the [`AppState`] via Leptos context. Starts a persistent
/// connection loop that automatically reconnects on failure.
#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state.clone());

    // Conditional tooltips: show title on .meta-value only when text is truncated
    js_sys::eval(
        r#"document.addEventListener('mouseenter', function(e) {
            if (e.target.classList && e.target.classList.contains('meta-value')) {
                if (e.target.scrollHeight > e.target.clientHeight) {
                    e.target.title = e.target.textContent;
                } else {
                    e.target.removeAttribute('title');
                }
            }
        }, true);"#,
    )
    .ok();

    // Tick every second to update relative timestamps
    let tick_cb = Closure::wrap(Box::new(move || {
        state.tick.update(|t| *t += 1);
    }) as Box<dyn FnMut()>);
    set_interval_js(tick_cb.as_ref().unchecked_ref(), 1_000);
    tick_cb.forget(); // lives for app lifetime

    // Countdown ticks (every 1 second): auto-sync + push debounce
    let countdown_cb = Closure::wrap(Box::new(move || {
        // Push debounce countdown (visual only — the actual push is fired by setTimeout)
        let debounce = state.push_debounce_countdown.get_untracked();
        if debounce > 0 {
            state.push_debounce_countdown.set(debounce - 1);
        }

        // Auto-sync countdown
        let enabled = state.auto_sync_enabled.get_untracked();
        let connected =
            state.connection_status.get_untracked() == ConnectionStatus::Connected;

        if !enabled || !connected {
            state.auto_sync_countdown.set(0);
            return;
        }

        let current = state.auto_sync_countdown.get_untracked();
        if current == 0 {
            let interval = state.auto_sync_interval_secs.get_untracked();
            state.auto_sync_countdown.set(interval);
        } else if current == 1 {
            state.auto_sync_countdown.set(0);
            if let Some(client) = get_client() {
                leptos::task::spawn_local(async move {
                    if let Err(e) = incremental_sync(&client, &state).await {
                        log::error!("Automatic sync failed: {e}");
                    }
                });
            }
        } else {
            state.auto_sync_countdown.set(current - 1);
        }
    }) as Box<dyn FnMut()>);
    set_interval_js(countdown_cb.as_ref().unchecked_ref(), 1_000);
    countdown_cb.forget();

    register_reconnect_listeners(state);
    register_beforeunload_guard(state);
    register_keyboard_shortcuts(state);
    register_popstate_listener(state);
    leptos::task::spawn_local(async move {
        // Require OPFS support — hard fail if unavailable.
        storage::require_opfs().await;

        // Fetch server-side default config and apply where user hasn't
        // set a preference in localStorage yet.
        apply_server_config(&state).await;

        // Request persistent storage to reduce eviction risk.
        if storage::request_persistent_storage().await {
            log::info!("Persistent storage granted by browser");
        }

        // Load cached state from OPFS so the UI renders instantly.
        load_local_state(&state).await;
        storage::load_history_cache().await;

        // Then enter the connection loop for background sync.
        connection_loop(state).await;
    });

    view! {
        <Home />
        <KeyboardHelp />
    }
}

/// Main connection loop with automatic reconnection on a fixed 5-second retry.
async fn connection_loop(state: AppState) {
    const RETRY_SECS: u32 = 5;

    loop {
        state.connection_status.set(ConnectionStatus::Connecting);
        state.reconnect_countdown.set(0);
        take_reconnect_request(); // clear any stale flag

        connect_and_run(&state).await;

        state.connection_status.set(ConnectionStatus::Disconnected);

        // Count down from RETRY_SECS, updating the signal each second.
        for remaining in (1..=RETRY_SECS).rev() {
            state.reconnect_countdown.set(remaining);
            sleep_ms(1_000).await;
            if take_reconnect_request() {
                break;
            }
        }
        state.reconnect_countdown.set(0);
    }
}

/// Attempt a full connection cycle: connect → sync → subscribe.
///
/// Uses incremental sync when local state exists (from OPFS or a previous
/// connection), falling back to a full initial sync when starting fresh.
async fn connect_and_run(state: &AppState) {
    let url = state.server_url.get_untracked();

    let cert_hash = match fetch_cert_hash(&url).await {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to fetch certificate hash: {e}");
            return;
        }
    };

    let client = match Client::connect(&url, &cert_hash).await {
        Ok(c) => Rc::new(c),
        Err(e) => {
            log::error!("Connection failed: {e}");
            return;
        }
    };

    set_client(Rc::clone(&client));

    // Use incremental sync when we already have local state, otherwise
    // perform a full initial sync.
    let sync_result = if state.root.get_untracked().is_some() {
        incremental_sync(&client, state).await
    } else {
        initial_sync(&client, state).await
    };

    if let Err(e) = sync_result {
        log::error!("Synchronization failed: {e}");
        return;
    }

    if let Err(e) = run_subscription(Rc::clone(&client), state).await {
        log::error!("Subscription stream ended: {e}");
    }
}

async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .expect("no window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .expect("setTimeout failed");
    });
    let _ = JsFuture::from(promise).await;
}

/// Register `visibilitychange` and `online` listeners that interrupt the
/// retry countdown so reconnection happens immediately when the user returns.
fn register_reconnect_listeners(state: AppState) {
    let window = web_sys::window().expect("no window");

    // visibilitychange — fires when the user switches back to the tab or
    // unlocks their phone.
    if let Some(document) = window.document() {
        let cb = Closure::wrap(Box::new(move || {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if !doc.hidden()
                    && state.connection_status.get_untracked() == ConnectionStatus::Disconnected
                {
                    log::info!("Page visible while disconnected, requesting reconnection");
                    request_reconnect();
                }
            }
        }) as Box<dyn FnMut()>);
        document
            .add_event_listener_with_callback("visibilitychange", cb.as_ref().unchecked_ref())
            .ok();
        cb.forget();
    }

    // online — fires when the browser regains network connectivity.
    {
        let cb = Closure::wrap(Box::new(move || {
            if state.connection_status.get_untracked() == ConnectionStatus::Disconnected {
                log::info!("Browser online while disconnected, requesting reconnection");
                request_reconnect();
            }
        }) as Box<dyn FnMut()>);
        window
            .add_event_listener_with_callback("online", cb.as_ref().unchecked_ref())
            .ok();
        cb.forget();
    }
}

/// Register a `popstate` listener so browser back/forward buttons restore
/// the app state from the URL query parameters.
fn register_popstate_listener(state: AppState) {
    let window = web_sys::window().expect("no window");
    let cb = Closure::wrap(Box::new(move |_: web_sys::PopStateEvent| {
        restore_from_query_params(&state);
    }) as Box<dyn FnMut(web_sys::PopStateEvent)>);
    window
        .add_event_listener_with_callback("popstate", cb.as_ref().unchecked_ref())
        .ok();
    cb.forget();
}

/// Register a `beforeunload` listener that warns when the user tries to close
/// the tab or navigate away while unsaved changes exist.
fn register_beforeunload_guard(state: AppState) {
    let window = web_sys::window().expect("no window");
    let cb = Closure::wrap(Box::new(move |ev: web_sys::BeforeUnloadEvent| {
        if state.has_unsaved_changes.get_untracked() {
            ev.prevent_default();
            ev.set_return_value("");
        }
    }) as Box<dyn FnMut(web_sys::BeforeUnloadEvent)>);
    window
        .add_event_listener_with_callback("beforeunload", cb.as_ref().unchecked_ref())
        .ok();
    cb.forget();
}

/// Fetch the server's self-signed certificate SHA-256 hash from its HTTP endpoint.
///
/// Given a WebTransport URL like `https://host:47400`, derives the cert-hash
/// endpoint at `http://host:47600/cert-hash` (port + 200) and returns the raw bytes.
async fn fetch_cert_hash(wt_url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let location = window.location();
    let page_protocol = location.protocol().unwrap_or_default();

    let fetch_url = if page_protocol == "https:" {
        // Served over HTTPS (e.g. LAN/Tailscale via --static-dir) — fetch
        // cert-hash from the same origin to avoid mixed-content blocking.
        let page_origin = location.origin().map_err(|_| "no origin".to_string())?;
        format!("{page_origin}/cert-hash")
    } else {
        // Plain HTTP (localhost dev via Trunk) — use existing port math.
        let url = web_sys::Url::new(wt_url).map_err(|_| "invalid server URL".to_string())?;
        let host = url.hostname();
        let wt_port: u16 = url.port().parse().unwrap_or(443);
        let http_port = wt_port + 200;
        format!("http://{host}:{http_port}/cert-hash")
    };

    let resp_val = JsFuture::from(window.fetch_with_str(&fetch_url))
        .await
        .map_err(|e| format!("{e:?}"))?;
    let resp: web_sys::Response = resp_val.unchecked_into();

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let hex = text.as_string().ok_or("response was not text")?;

    hex_to_bytes(hex.trim()).ok_or_else(|| "invalid hex in cert hash response".to_string())
}

/// Fetch server-side client defaults from `/config` and apply them to
/// state signals where the user hasn't set a preference in localStorage.
async fn apply_server_config(state: &AppState) {
    let config = match fetch_config().await {
        Ok(c) => c,
        Err(e) => {
            log::info!("Server configuration fetch skipped: {e}");
            return;
        }
    };

    fn get_bool(obj: &JsValue, key: &str) -> Option<bool> {
        js_sys::Reflect::get(obj, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_bool())
    }
    fn get_u32(obj: &JsValue, key: &str) -> Option<u32> {
        js_sys::Reflect::get(obj, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_f64())
            .map(|n| n as u32)
    }

    if !settings::has_auto_save() {
        if let Some(v) = get_bool(&config, "auto_save") {
            state.auto_save_enabled.set(v);
        }
    }
    if !settings::has_auto_save_delay() {
        if let Some(v) = get_u32(&config, "auto_save_delay") {
            state.auto_save_delay_secs.set(v);
        }
    }
    if !settings::has_show_preview() {
        if let Some(v) = get_bool(&config, "show_preview") {
            state.show_preview.set(v);
        }
    }
    if !settings::has_drag_drop_reorder() {
        if let Some(v) = get_bool(&config, "drag_drop") {
            state.drag_drop_reorder.set(v);
        }
    }
    if !settings::has_auto_sync() {
        if let Some(v) = get_bool(&config, "auto_sync") {
            state.auto_sync_enabled.set(v);
        }
    }
    if !settings::has_auto_sync_interval() {
        if let Some(v) = get_u32(&config, "auto_sync_interval") {
            state.auto_sync_interval_secs.set(v);
        }
    }
    if !settings::has_debounce_enabled() {
        if let Some(v) = get_bool(&config, "debounce_enabled") {
            state.debounce_enabled.set(v);
        }
    }
    if !settings::has_debounce_delay() {
        if let Some(v) = get_u32(&config, "debounce_delay") {
            state.debounce_delay_secs.set(v);
        }
    }
    if !settings::has_keyboard_shortcuts() {
        if let Some(v) = get_bool(&config, "keyboard_shortcuts") {
            state.keyboard_shortcuts_enabled.set(v);
        }
    }

    log::info!("Applied server configuration defaults");
}

/// Fetch the `/config` JSON from the server.
async fn fetch_config() -> Result<JsValue, String> {
    let window = web_sys::window().ok_or("no window")?;
    let location = window.location();
    let page_protocol = location.protocol().unwrap_or_default();

    let fetch_url = if page_protocol == "https:" {
        let page_origin = location.origin().map_err(|_| "no origin".to_string())?;
        format!("{page_origin}/config")
    } else {
        let host = location.hostname().unwrap_or_else(|_| "127.0.0.1".to_string());
        let page_port: u16 = location
            .port()
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(47800);
        let http_port = page_port.wrapping_sub(200);
        format!("http://{host}:{http_port}/config")
    };

    let resp_val = JsFuture::from(window.fetch_with_str(&fetch_url))
        .await
        .map_err(|e| format!("{e:?}"))?;
    let resp: web_sys::Response = resp_val.unchecked_into();

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let json_str = text.as_string().ok_or("not text")?;

    js_sys::JSON::parse(&json_str).map_err(|_| "invalid JSON".to_string())
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

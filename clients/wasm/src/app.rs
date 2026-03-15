use leptos::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::components::keyboard::register_keyboard_shortcuts;
use crate::components::settings_panel::{apply_ui_density, apply_ui_scale};
use crate::pages::home::Home;
use crate::state::settings;
use crate::state::store::{AppState, ConnectionStatus, clear_client, get_client, restore_from_query_params, set_client};
use crate::state::sync::{
    flush_offline_queue, incremental_sync, initial_sync, load_local_state, run_subscription,
};
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

    // Conditional tooltips: show title only when text is truncated
    js_sys::eval(
        r#"document.addEventListener('mouseenter', function(e) {
            var t = e.target;
            if (!t.classList) return;
            if (t.classList.contains('meta-value')) {
                if (t.scrollHeight > t.clientHeight) {
                    t.title = t.textContent;
                } else {
                    t.removeAttribute('title');
                }
            } else if (t.classList.contains('card-preview') || t.classList.contains('tag-title')) {
                if (t.scrollWidth > t.clientWidth) {
                    t.title = t.textContent;
                } else {
                    t.removeAttribute('title');
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
                        tracing::error!(%e, "Automatic sync failed");
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
            tracing::info!("Persistent storage granted by browser");
        }

        // Load cached state from OPFS so the UI renders instantly.
        load_local_state(&state).await;
        storage::load_history_cache().await;
        state.offline_queue.set(storage::load_offline_queue().await);

        // Then enter the connection loop for background sync.
        connection_loop(state).await;
    });

    // Apply initial UI customizations from settings
    apply_ui_scale(state.ui_scale.get_untracked());
    apply_ui_density(&state.ui_density.get_untracked());

    view! {
        <Home />
    }
}

/// Main connection loop with automatic reconnection on a fixed 5-second retry.
///
/// Each connection attempt is spawned in a subtask so it can be interrupted
/// by a timeout or a reconnect request (e.g., `visibilitychange` / `online`
/// listeners). This prevents the loop from getting stuck on a hanging
/// WebTransport handshake when the device returns from sleep.
async fn connection_loop(state: AppState) {
    const CONNECT_TIMEOUT_SECS: u32 = 15;
    const RETRY_SECS: u32 = 5;

    loop {
        state.connection_status.set(ConnectionStatus::Connecting);
        state.reconnect_countdown.set(0);
        take_reconnect_request(); // clear any stale flag

        // Spawn the connection attempt so we can poll for timeout/interrupt.
        let finished = Rc::new(Cell::new(false));
        {
            let f = finished.clone();
            let s = state;
            leptos::task::spawn_local(async move {
                connect_and_run(&s).await;
                f.set(true);
            });
        }

        // Phase 1: Wait for connection to establish (with timeout).
        let mut was_connected = false;
        let timeout_checks = CONNECT_TIMEOUT_SECS * 10; // 100ms intervals
        for _ in 0..timeout_checks {
            sleep_ms(100).await;
            if finished.get() {
                break;
            }
            if take_reconnect_request() {
                break;
            }
            let status = state.connection_status.get_untracked();
            if matches!(status, ConnectionStatus::Connected | ConnectionStatus::Syncing) {
                was_connected = true;
                break;
            }
        }

        // Phase 2: If connected, wait for connection to end naturally.
        if was_connected {
            while !finished.get() {
                sleep_ms(200).await;
                if take_reconnect_request() {
                    break;
                }
            }
        } else if !finished.get() {
            tracing::warn!(timeout_secs = CONNECT_TIMEOUT_SECS, "Connection attempt timed out");
        }

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
    // Clear any stale client from a previous connection so pushes during
    // sync go to the offline queue rather than a dead transport.
    clear_client();

    let url = state.server_url.get_untracked();

    let cert_hash = match fetch_cert_hash(&url).await {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(%e, "Failed to fetch certificate hash");
            return;
        }
    };

    let client = match Client::connect(&url, &cert_hash).await {
        Ok(c) => Rc::new(c),
        Err(e) => {
            tracing::error!(%e, "Connection failed");
            return;
        }
    };

    // Sync before exposing the client globally. While sync is in progress,
    // get_client() returns None so any user-triggered pushes go to the
    // offline queue instead of racing with stale OPFS-cached ancestor hashes.
    let sync_result = if state.root.get_untracked().is_some() {
        incremental_sync(&client, state).await
    } else {
        initial_sync(&client, state).await
    };

    if let Err(e) = sync_result {
        tracing::error!(%e, "Synchronization failed");
        return;
    }

    set_client(Rc::clone(&client));

    // Flush any cards that were queued while offline.
    flush_offline_queue(&client, state).await;

    if let Err(e) = run_subscription(Rc::clone(&client), state).await {
        tracing::error!(%e, "Subscription stream ended");
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
    // unlocks their phone. Also handles `Connecting` state to interrupt
    // stale connection attempts that may be hanging.
    if let Some(document) = window.document() {
        let cb = Closure::wrap(Box::new(move || {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if !doc.hidden()
                    && matches!(
                        state.connection_status.get_untracked(),
                        ConnectionStatus::Disconnected | ConnectionStatus::Connecting
                    )
                {
                    tracing::info!("Page visible while not connected, requesting reconnection");
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
            if matches!(
                state.connection_status.get_untracked(),
                ConnectionStatus::Disconnected | ConnectionStatus::Connecting
            ) {
                tracing::info!("Browser online while not connected, requesting reconnection");
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
            tracing::info!(%e, "Server configuration fetch skipped");
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
    if !settings::has_search_tags() {
        if let Some(v) = get_bool(&config, "search_tags") {
            state.search_tags.set(v);
        }
    }
    if !settings::has_ui_scale() {
        if let Some(v) = get_u32(&config, "ui_scale") {
            state.ui_scale.set(v);
            apply_ui_scale(v);
        }
    }
    if !settings::has_ui_density() {
        if let Some(v) = js_sys::Reflect::get(&config, &JsValue::from_str("ui_density"))
            .ok()
            .and_then(|v| v.as_string())
        {
            apply_ui_density(&v);
            state.ui_density.set(v);
        }
    }
    if !settings::has_touch_swipe() {
        if let Some(v) = get_bool(&config, "touch_swipe") {
            state.touch_swipe_enabled.set(v);
        }
    }
    if !settings::has_swipe_threshold_right() {
        if let Some(v) = get_u32(&config, "swipe_threshold_right") {
            state.swipe_threshold_right.set(v);
        }
    }
    if !settings::has_swipe_threshold_left() {
        if let Some(v) = get_u32(&config, "swipe_threshold_left") {
            state.swipe_threshold_left.set(v);
        }
    }
    if !settings::has_clear_tag_search() {
        if let Some(v) = get_bool(&config, "clear_tag_search") {
            state.clear_tag_search.set(v);
        }
    }
    if !settings::has_default_sidebar_width() {
        if let Some(v) = get_u32(&config, "default_sidebar_width") {
            state.default_sidebar_width.set(v);
        }
    }
    if !settings::has_default_detail_width() {
        if let Some(v) = get_u32(&config, "default_detail_width") {
            state.default_detail_width.set(v);
        }
    }
    if !settings::has_override_sidebar_width() {
        if let Some(v) = get_bool(&config, "override_sidebar_width") {
            state.override_sidebar_width.set(v);
        }
    }
    if !settings::has_override_detail_width() {
        if let Some(v) = get_bool(&config, "override_detail_width") {
            state.override_detail_width.set(v);
        }
    }

    tracing::info!("Applied server configuration defaults");
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

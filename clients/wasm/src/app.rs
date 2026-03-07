use leptos::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::pages::home::Home;
use crate::state::store::{AppState, ConnectionStatus, set_client};
use crate::state::sync::{initial_sync, run_subscription};
use crate::transport::client::Client;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setInterval")]
    fn set_interval_js(handler: &js_sys::Function, timeout: i32) -> i32;
}

thread_local! {
    /// Set by browser event listeners (`online`, `visibilitychange`) to
    /// interrupt the backoff sleep and reconnect immediately.
    static RECONNECT_NOW: Cell<bool> = const { Cell::new(false) };
}

/// Request an immediate reconnection attempt, interrupting the backoff sleep.
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

    // Tick every 30 seconds to update relative timestamps
    let tick_cb = Closure::wrap(Box::new(move || {
        state.tick.update(|t| *t += 1);
    }) as Box<dyn FnMut()>);
    set_interval_js(tick_cb.as_ref().unchecked_ref(), 30_000);
    tick_cb.forget(); // lives for app lifetime

    register_reconnect_listeners(state);
    leptos::task::spawn_local(connection_loop(state));

    view! {
        <Home />
    }
}

/// Main connection loop with automatic reconnection and exponential backoff.
async fn connection_loop(state: AppState) {
    let mut backoff_ms: u32 = 1_000;
    const MAX_BACKOFF_MS: u32 = 30_000;

    loop {
        state.connection_status.set(ConnectionStatus::Connecting);
        take_reconnect_request(); // clear any stale flag

        let was_connected = connect_and_run(&state).await;

        state.connection_status.set(ConnectionStatus::Disconnected);

        if was_connected {
            // Was connected but lost — reset backoff for fast recovery.
            backoff_ms = 1_000;
        }

        interruptible_sleep(backoff_ms).await;
        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
    }
}

/// Attempt a full connection cycle: connect → sync → subscribe.
///
/// Returns `true` if the connection was established successfully (even if the
/// subscription later broke), `false` if we couldn't connect at all.
async fn connect_and_run(state: &AppState) -> bool {
    let url = state.server_url.get_untracked();

    let cert_hash = match fetch_cert_hash(&url).await {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to fetch cert hash: {e}");
            return false;
        }
    };

    let client = match Client::connect(&url, &cert_hash).await {
        Ok(c) => Rc::new(c),
        Err(e) => {
            log::error!("Connection failed: {e}");
            return false;
        }
    };

    set_client(Rc::clone(&client));

    if let Err(e) = initial_sync(&client, state).await {
        log::error!("Initial sync failed: {e}");
        return false;
    }

    if let Err(e) = run_subscription(Rc::clone(&client), state).await {
        log::error!("Subscription ended: {e}");
    }

    true
}

/// Sleep in 500 ms chunks, returning early if a reconnect is requested.
async fn interruptible_sleep(total_ms: u32) {
    const CHUNK_MS: u32 = 500;
    let mut remaining = total_ms;

    while remaining > 0 {
        let chunk = remaining.min(CHUNK_MS);
        sleep_ms(chunk).await;
        remaining = remaining.saturating_sub(chunk);
        if take_reconnect_request() {
            return;
        }
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
/// backoff sleep so reconnection happens immediately when the user returns.
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
                    log::info!("Page visible while disconnected, requesting reconnect");
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
                log::info!("Browser online while disconnected, requesting reconnect");
                request_reconnect();
            }
        }) as Box<dyn FnMut()>);
        window
            .add_event_listener_with_callback("online", cb.as_ref().unchecked_ref())
            .ok();
        cb.forget();
    }
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

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

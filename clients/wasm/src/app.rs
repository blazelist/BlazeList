use leptos::prelude::*;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::pages::home::Home;
use crate::state::store::{AppState, ConnectionStatus, set_client};
use crate::state::sync::{initial_sync, start_subscription};
use crate::transport::client::Client;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setInterval")]
    fn set_interval_js(handler: &js_sys::Function, timeout: i32) -> i32;
}

/// Root application component.
///
/// Provides the [`AppState`] via Leptos context. Auto-connects to the default
/// localhost server on mount.
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

    // Auto-connect on mount
    leptos::task::spawn_local({
        let state = state.clone();
        async move {
            let url = state.server_url.get_untracked();
            state.connection_status.set(ConnectionStatus::Connecting);

            let cert_hash = match fetch_cert_hash(&url).await {
                Ok(h) => h,
                Err(e) => {
                    log::error!("Failed to fetch cert hash: {e}");
                    state.connection_status.set(ConnectionStatus::Disconnected);
                    return;
                }
            };

            let client = match Client::connect(&url, &cert_hash).await {
                Ok(c) => Rc::new(c),
                Err(e) => {
                    log::error!("Connection failed: {e}");
                    state.connection_status.set(ConnectionStatus::Disconnected);
                    return;
                }
            };

            set_client(Rc::clone(&client));

            if let Err(e) = initial_sync(&client, &state).await {
                log::error!("Initial sync failed: {e}");
                state.connection_status.set(ConnectionStatus::Disconnected);
                return;
            }

            start_subscription(Rc::clone(&client), state.clone());
        }
    });

    view! {
        <Home />
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

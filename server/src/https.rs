//! HTTP and HTTPS servers for certificate hashing and static-file serving.
//!
//! Contains:
//! - A plain HTTP server that exposes the certificate SHA-256 hash so WASM
//!   clients can auto-fetch it for `serverCertificateHashes`.
//! - An HTTPS static-file server for serving the WASM frontend in a secure
//!   context (required for WebTransport on non-localhost origins).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Build a [`TlsAcceptor`] from raw DER certificate and key bytes.
pub fn tls_acceptor(
    cert_der: &[u8],
    key_der: &[u8],
) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let certs = vec![CertificateDer::from(cert_der.to_vec())];
    let key = PrivatePkcs8KeyDer::from(key_der.to_vec());

    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key.into())?;

    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Build a JSON config string from `BLAZELIST_DEFAULT_*` env vars.
///
/// Returns defaults for client settings that can be overridden per-device
/// in the browser's localStorage.
pub fn build_client_config_json() -> String {
    let show_preview = std::env::var("BLAZELIST_DEFAULT_SHOW_PREVIEW").ok();
    let auto_sync = std::env::var("BLAZELIST_DEFAULT_AUTO_SYNC").ok();
    let auto_sync_interval_ms = std::env::var("BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS").ok();
    let priority_debounce_enabled =
        std::env::var("BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_ENABLED").ok();
    let priority_debounce_delay_ms =
        std::env::var("BLAZELIST_DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS").ok();
    let keyboard_shortcuts = std::env::var("BLAZELIST_DEFAULT_KEYBOARD_SHORTCUTS").ok();
    let search_tags = std::env::var("BLAZELIST_DEFAULT_SEARCH_TAGS").ok();
    let ui_scale = std::env::var("BLAZELIST_DEFAULT_UI_SCALE").ok();
    let ui_density = std::env::var("BLAZELIST_DEFAULT_UI_DENSITY").ok();
    let touch_swipe = std::env::var("BLAZELIST_DEFAULT_TOUCH_SWIPE").ok();
    let swipe_threshold_right_cycle =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_CYCLE").ok();
    let swipe_threshold_right_levels =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS").ok();
    let swipe_threshold_left_cycle =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_CYCLE").ok();
    let swipe_threshold_left_levels =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS").ok();
    let swipe_undo_timeout_ms = std::env::var("BLAZELIST_DEFAULT_SWIPE_UNDO_TIMEOUT_MS").ok();
    let swipe_left_mode = std::env::var("BLAZELIST_DEFAULT_SWIPE_LEFT_MODE").ok();
    let swipe_levels_zone_today_width =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TODAY_WIDTH").ok();
    let swipe_levels_zone_tomorrow_width =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH").ok();
    let swipe_levels_zone_soon_width =
        std::env::var("BLAZELIST_DEFAULT_SWIPE_LEVELS_ZONE_SOON_WIDTH").ok();
    let clear_tag_search = std::env::var("BLAZELIST_DEFAULT_CLEAR_TAG_SEARCH").ok();
    let default_sidebar_width = std::env::var("BLAZELIST_DEFAULT_SIDEBAR_WIDTH").ok();
    let default_detail_width = std::env::var("BLAZELIST_DEFAULT_DETAIL_WIDTH").ok();
    let override_sidebar_width = std::env::var("BLAZELIST_DEFAULT_OVERRIDE_SIDEBAR_WIDTH").ok();
    let override_detail_width = std::env::var("BLAZELIST_DEFAULT_OVERRIDE_DETAIL_WIDTH").ok();
    let recursive_links = std::env::var("BLAZELIST_DEFAULT_RECURSIVE_LINKS").ok();
    let show_list_link_counts = std::env::var("BLAZELIST_DEFAULT_SHOW_LIST_LINK_COUNTS").ok();
    let show_due_today_button = std::env::var("BLAZELIST_DEFAULT_SHOW_DUE_TODAY_BUTTON").ok();
    let show_card_time = std::env::var("BLAZELIST_DEFAULT_SHOW_CARD_TIME").ok();
    let extinguish_on_due_set = std::env::var("BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_SET").ok();
    let extinguish_on_due_clear = std::env::var("BLAZELIST_DEFAULT_EXTINGUISH_ON_DUE_CLEAR").ok();
    let clear_due_on_blaze = std::env::var("BLAZELIST_DEFAULT_CLEAR_DUE_ON_BLAZE").ok();
    let drag_and_drop_enabled = std::env::var("BLAZELIST_DEFAULT_DRAG_AND_DROP_ENABLED").ok();
    let drag_and_drop_mode = std::env::var("BLAZELIST_DEFAULT_DRAG_AND_DROP_MODE").ok();

    // Only include env vars that are explicitly set.
    let mut pairs = Vec::new();
    if let Some(v) = show_preview {
        pairs.push(format!(r#""show_preview":{}"#, v == "true"));
    }
    if let Some(v) = auto_sync {
        pairs.push(format!(r#""auto_sync":{}"#, v == "true"));
    }
    if let Some(v) = auto_sync_interval_ms
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""auto_sync_interval_ms":{n}"#));
    }
    if let Some(v) = priority_debounce_enabled {
        pairs.push(format!(r#""priority_debounce_enabled":{}"#, v == "true"));
    }
    if let Some(v) = priority_debounce_delay_ms
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""priority_debounce_delay_ms":{n}"#));
    }
    if let Some(v) = keyboard_shortcuts {
        pairs.push(format!(r#""keyboard_shortcuts":{}"#, v == "true"));
    }
    if let Some(v) = search_tags {
        pairs.push(format!(r#""search_tags":{}"#, v == "true"));
    }
    if let Some(v) = ui_scale
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""ui_scale":{n}"#));
    }
    if let Some(v) = ui_density {
        pairs.push(format!(r#""ui_density":"{}""#, v.replace('"', "")));
    }
    if let Some(v) = touch_swipe {
        pairs.push(format!(r#""touch_swipe":{}"#, v == "true"));
    }
    if let Some(v) = swipe_threshold_right_cycle
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_threshold_right_cycle":{n}"#));
    }
    if let Some(v) = swipe_threshold_right_levels
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_threshold_right_levels":{n}"#));
    }
    if let Some(v) = swipe_threshold_left_cycle
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_threshold_left_cycle":{n}"#));
    }
    if let Some(v) = swipe_threshold_left_levels
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_threshold_left_levels":{n}"#));
    }
    if let Some(v) = swipe_undo_timeout_ms
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_undo_timeout_ms":{n}"#));
    }
    if let Some(v) = swipe_left_mode {
        pairs.push(format!(r#""swipe_left_mode":"{}""#, v.replace('"', "")));
    }
    if let Some(v) = swipe_levels_zone_today_width
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_levels_zone_today_width":{n}"#));
    }
    if let Some(v) = swipe_levels_zone_tomorrow_width
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_levels_zone_tomorrow_width":{n}"#));
    }
    if let Some(v) = swipe_levels_zone_soon_width
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""swipe_levels_zone_soon_width":{n}"#));
    }
    if let Some(v) = clear_tag_search {
        pairs.push(format!(r#""clear_tag_search":{}"#, v == "true"));
    }
    if let Some(v) = default_sidebar_width
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""default_sidebar_width":{n}"#));
    }
    if let Some(v) = default_detail_width
        && let Ok(n) = v.parse::<u32>()
    {
        pairs.push(format!(r#""default_detail_width":{n}"#));
    }
    if let Some(v) = override_sidebar_width {
        pairs.push(format!(r#""override_sidebar_width":{}"#, v == "true"));
    }
    if let Some(v) = override_detail_width {
        pairs.push(format!(r#""override_detail_width":{}"#, v == "true"));
    }
    if let Some(v) = recursive_links {
        pairs.push(format!(r#""recursive_links":{}"#, v == "true"));
    }
    if let Some(v) = show_list_link_counts {
        pairs.push(format!(r#""show_list_link_counts":{}"#, v == "true"));
    }
    if let Some(v) = show_due_today_button {
        pairs.push(format!(r#""show_due_today_button":{}"#, v == "true"));
    }
    if let Some(v) = show_card_time {
        pairs.push(format!(r#""show_card_time":{}"#, v == "true"));
    }
    if let Some(v) = extinguish_on_due_set {
        pairs.push(format!(r#""extinguish_on_due_set":{}"#, v == "true"));
    }
    if let Some(v) = extinguish_on_due_clear {
        pairs.push(format!(r#""extinguish_on_due_clear":{}"#, v == "true"));
    }
    if let Some(v) = clear_due_on_blaze {
        pairs.push(format!(r#""clear_due_on_blaze":{}"#, v == "true"));
    }
    if let Some(v) = drag_and_drop_enabled {
        pairs.push(format!(r#""drag_and_drop_enabled":{}"#, v == "true"));
    }
    if let Some(v) = drag_and_drop_mode {
        pairs.push(format!(r#""drag_and_drop_mode":"{}""#, v.replace('"', "")));
    }

    format!("{{{}}}", pairs.join(","))
}

/// Run the HTTPS static-file server.
///
/// Serves files from `static_dir` over TLS and exposes `/cert-hash` so the
/// WASM client can fetch the certificate hash from the same origin (avoiding
/// mixed-content blocking on HTTPS pages).
pub async fn run_https_server(
    addr: SocketAddr,
    static_dir: PathBuf,
    cert_hash_hex: String,
    config_json: String,
    acceptor: TlsAcceptor,
) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(%addr, error = %e, "failed to bind HTTPS server");
            return;
        }
    };

    let static_dir = Arc::new(static_dir);
    let cert_hash_hex = Arc::new(cert_hash_hex);
    let config_json = Arc::new(config_json);

    loop {
        let (tcp_stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let acceptor = acceptor.clone();
        let static_dir = Arc::clone(&static_dir);
        let cert_hash_hex = Arc::clone(&cert_hash_hex);
        let config_json = Arc::clone(&config_json);

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(tcp_stream).await {
                Ok(s) => s,
                Err(_) => return,
            };

            handle_connection(tls_stream, &static_dir, &cert_hash_hex, &config_json).await;
        });
    }
}

async fn handle_connection(
    mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    static_dir: &Path,
    cert_hash_hex: &str,
    config_json: &str,
) {
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf).await {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };

    let request = String::from_utf8_lossy(&buf[..n]);
    let request_line = request.lines().next().unwrap_or("");
    let path = parse_request_path(request_line);

    let response = match path.as_deref() {
        Some("/cert-hash") => build_cors_response("text/plain", cert_hash_hex),
        Some("/config") => build_cors_response("application/json", config_json),
        Some(p) => serve_static_file(static_dir, p),
        None => build_error_response(400, "Bad Request"),
    };

    let _ = stream.write_all(&response).await;
    let _ = stream.shutdown().await;
}

fn parse_request_path(request_line: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let raw_path = parts.next()?;

    if method != "GET" {
        return None;
    }

    // Clear query string.
    let path = raw_path.split('?').next().unwrap_or(raw_path);
    Some(path.to_string())
}

fn serve_static_file(static_dir: &Path, request_path: &str) -> Vec<u8> {
    let clean_path = request_path.trim_start_matches('/');

    let canonical_dir = match static_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return build_error_response(500, "Internal Server Error"),
    };

    // Path traversal is rejected textually on the cleaned request path —
    // not by canonicalizing and fencing under `canonical_dir`. When
    // `static_dir` is a Nix `symlinkJoin` output, every leaf file is a
    // symlink into a sibling store path, so a canonical-path fence would
    // reject every asset and dump it into the SPA fallback. The request
    // path is never URL-decoded, so a literal-segment compare is enough:
    // sequences like `%2E%2E` are opaque directory names and won't match
    // anything on disk.
    let traversal = clean_path.split('/').any(|s| s == "..");
    let candidate = canonical_dir.join(clean_path);
    let file_path = if !traversal && candidate.is_file() {
        candidate
    } else {
        canonical_dir.join("index.html")
    };

    match std::fs::read(&file_path) {
        Ok(contents) => build_file_response(&file_path, &contents),
        Err(_) => build_error_response(404, "Not Found"),
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("ico") => "image/x-icon",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webmanifest") => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

fn build_file_response(path: &Path, body: &[u8]) -> Vec<u8> {
    let content_type = content_type_for(path);
    let header = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}

fn build_cors_response(content_type: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        body.len(),
        body
    )
    .into_bytes()
}

fn build_error_response(status: u16, reason: &str) -> Vec<u8> {
    let body = format!("{status} {reason}");
    format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    )
    .into_bytes()
}

// -- Plain-HTTP cert-hash endpoint -------------------------------------------

/// Encode raw bytes as lowercase hex.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Minimal HTTP/1.1 server for cert-hash and config endpoints.
///
/// Used by WASM clients to auto-fetch the server certificate hash before
/// establishing a WebTransport connection, and to get server-default settings.
pub async fn run_cert_hash_server(addr: SocketAddr, cert_hash_hex: String, config_json: String) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(%addr, error = %e, "failed to bind cert-hash HTTP server");
            return;
        }
    };

    let cert_hash_hex = Arc::new(cert_hash_hex);
    let config_json = Arc::new(config_json);

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let cert_hash_hex = Arc::clone(&cert_hash_hex);
        let config_json = Arc::clone(&config_json);
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let n = match tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };

            let request = String::from_utf8_lossy(&buf[..n]);
            let request_line = request.lines().next().unwrap_or("");
            let path = parse_request_path(request_line);

            let response = match path.as_deref() {
                Some("/config") => build_cors_response("application/json", &config_json),
                _ => build_cors_response("text/plain", &cert_hash_hex),
            };

            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_request_path, serve_static_file};

    /// Split a raw HTTP response into (status line, body) for assertions.
    fn split_response(response: &[u8]) -> (String, Vec<u8>) {
        let text = String::from_utf8_lossy(response);
        let status_line = text.lines().next().unwrap_or("").to_string();
        // Body starts after the blank line terminating the headers.
        let body = match response.windows(4).position(|w| w == b"\r\n\r\n") {
            Some(i) => response[i + 4..].to_vec(),
            None => Vec::new(),
        };
        (status_line, body)
    }

    // -- parse_request_path -------------------------------------------------

    #[test]
    fn parse_request_path_get_strips_query() {
        assert_eq!(
            parse_request_path("GET /foo?x=1 HTTP/1.1"),
            Some("/foo".to_string())
        );
    }

    #[test]
    fn parse_request_path_get_without_query() {
        assert_eq!(
            parse_request_path("GET /cert-hash HTTP/1.1"),
            Some("/cert-hash".to_string())
        );
    }

    #[test]
    fn parse_request_path_post_is_none() {
        assert_eq!(parse_request_path("POST /foo HTTP/1.1"), None);
    }

    #[test]
    fn parse_request_path_non_get_methods_are_none() {
        // Any method other than GET is rejected.
        assert_eq!(parse_request_path("PUT /foo HTTP/1.1"), None);
        assert_eq!(parse_request_path("HEAD /foo HTTP/1.1"), None);
        assert_eq!(parse_request_path("OPTIONS /foo HTTP/1.1"), None);
    }

    #[test]
    fn parse_request_path_malformed_lines_are_none() {
        // Empty line: no method token.
        assert_eq!(parse_request_path(""), None);
        // Only a method, no path token.
        assert_eq!(parse_request_path("GET"), None);
        assert_eq!(parse_request_path("GET "), None);
    }

    #[test]
    fn parse_request_path_method_is_case_sensitive() {
        // The compare is exact "GET", so a lowercase verb is not accepted.
        assert_eq!(parse_request_path("get /foo HTTP/1.1"), None);
    }

    // -- serve_static_file --------------------------------------------------

    #[test]
    fn serve_static_file_existing_file_returns_contents() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), b"INDEX").unwrap();
        std::fs::write(dir.path().join("app.js"), b"console.log(1)").unwrap();

        let response = serve_static_file(dir.path(), "/app.js");
        let (status, body) = split_response(&response);

        assert!(status.starts_with("HTTP/1.1 200 OK"), "status: {status}");
        assert_eq!(body, b"console.log(1)");
    }

    /// Build a unique tempdir holding `root/` (the served static root, with
    /// an index.html) and a sibling `secret.txt` one directory above the
    /// root — exactly where a `..` traversal would land if the guard broke.
    /// Everything lives inside the per-test `TempDir`, so parallel tests
    /// (and other users of the machine) never share paths, and Drop cleans
    /// up the secret.
    fn static_root_with_sibling_secret() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("index.html"), b"INDEX").unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"TOP-SECRET").unwrap();
        (dir, root)
    }

    #[test]
    fn serve_static_file_dotdot_segment_falls_back_to_index_without_escape() {
        let (_dir, root) = static_root_with_sibling_secret();

        let response = serve_static_file(&root, "/../secret.txt");
        let (status, body) = split_response(&response);

        // The textual `..` check rejects the traversal: we get index.html,
        // never the file outside the root.
        assert!(status.starts_with("HTTP/1.1 200 OK"), "status: {status}");
        assert_eq!(body, b"INDEX");
        assert_ne!(body, b"TOP-SECRET");
    }

    #[test]
    fn serve_static_file_percent_encoded_dotdot_is_literal_and_falls_back() {
        let (_dir, root) = static_root_with_sibling_secret();

        // The request path is never URL-decoded, so `%2E%2E` is an opaque
        // directory name that matches nothing on disk -> SPA fallback.
        let response = serve_static_file(&root, "/%2E%2E/secret.txt");
        let (status, body) = split_response(&response);

        assert!(status.starts_with("HTTP/1.1 200 OK"), "status: {status}");
        assert_eq!(body, b"INDEX");
        assert_ne!(body, b"TOP-SECRET");
    }

    #[test]
    fn serve_static_file_unknown_path_falls_back_to_index() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), b"INDEX").unwrap();

        let response = serve_static_file(dir.path(), "/does/not/exist");
        let (status, body) = split_response(&response);

        assert!(status.starts_with("HTTP/1.1 200 OK"), "status: {status}");
        assert_eq!(body, b"INDEX");
    }
}

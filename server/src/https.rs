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

/// Run the HTTPS static-file server.
///
/// Serves files from `static_dir` over TLS and exposes `/cert-hash` so the
/// WASM client can fetch the certificate hash from the same origin (avoiding
/// mixed-content blocking on HTTPS pages).
pub async fn run_https_server(
    addr: SocketAddr,
    static_dir: PathBuf,
    cert_hash_hex: String,
    acceptor: TlsAcceptor,
) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind HTTPS server on {addr}: {e}");
            return;
        }
    };

    let static_dir = Arc::new(static_dir);
    let cert_hash_hex = Arc::new(cert_hash_hex);

    loop {
        let (tcp_stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let acceptor = acceptor.clone();
        let static_dir = Arc::clone(&static_dir);
        let cert_hash_hex = Arc::clone(&cert_hash_hex);

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(tcp_stream).await {
                Ok(s) => s,
                Err(_) => return,
            };

            handle_connection(tls_stream, &static_dir, &cert_hash_hex).await;
        });
    }
}

async fn handle_connection(
    mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    static_dir: &Path,
    cert_hash_hex: &str,
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
        Some("/cert-hash") => build_cert_hash_response(cert_hash_hex),
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

    // Strip query string.
    let path = raw_path.split('?').next().unwrap_or(raw_path);
    Some(path.to_string())
}

fn serve_static_file(static_dir: &Path, request_path: &str) -> Vec<u8> {
    let clean_path = request_path.trim_start_matches('/');

    let canonical_dir = match static_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return build_error_response(500, "Internal Server Error"),
    };

    // Try the requested file, then fall back to index.html (SPA routing).
    let file_path = if clean_path.is_empty() {
        canonical_dir.join("index.html")
    } else {
        let resolved = static_dir.join(clean_path);
        match resolved.canonicalize() {
            Ok(p) if p.starts_with(&canonical_dir) && p.is_file() => p,
            _ => canonical_dir.join("index.html"),
        }
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

fn build_cert_hash_response(hex: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        hex.len(),
        hex
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

/// Minimal HTTP/1.1 server that responds to any request with the cert hash.
///
/// Used by WASM clients to auto-fetch the server certificate hash before
/// establishing a WebTransport connection.
pub async fn run_cert_hash_server(addr: SocketAddr, cert_hash_hex: String) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind cert-hash HTTP server on {addr}: {e}");
            return;
        }
    };

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let body = cert_hash_hex.clone();
        tokio::spawn(async move {
            // Read (and discard) the request — we only need to drain enough to
            // unblock the client, then send our response.
            let mut buf = [0u8; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: text/plain\r\n\
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
            );

            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}

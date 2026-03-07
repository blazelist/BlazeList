//! WebTransport server loop.
//!
//! Mirrors the QUIC server in [`crate::quic::server`] but accepts browser
//! connections over WebTransport (HTTP/3). Uses the same transport-agnostic
//! [`handle_request`](crate::handler::handle_request) dispatcher.

use std::sync::Arc;

use blazelist_protocol::handshake::server_handshake;
use blazelist_protocol::wire::{read_message, write_message};
use blazelist_protocol::{Request, Response, RootState};
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use wtransport::config::IpBindConfig;
use wtransport::tls::{Certificate, CertificateChain, Identity, PrivateKey};
use wtransport::{Endpoint, ServerConfig};

use crate::SERVER_VERSION;
use crate::handler::handle_request;
use crate::storage::Storage;

/// Result of building the WebTransport server config, including the cert hash
/// needed by WASM clients using `serverCertificateHashes`.
pub struct WtServerConfig {
    pub config: ServerConfig,
    /// Raw SHA-256 hash of the server certificate (32 bytes).
    pub cert_hash: [u8; 32],
}

pub fn webtransport_server_config(
    cert_der: &[u8],
    key_der: &[u8],
    addr: std::net::SocketAddr,
) -> Result<WtServerConfig, Box<dyn std::error::Error>> {
    let certificate = Certificate::from_der(cert_der.to_vec())
        .map_err(|e| format!("invalid certificate DER: {e:?}"))?;
    let cert_hash = certificate.hash();
    println!("WebTransport cert SHA-256 hash: {cert_hash}");

    let hash_bytes: [u8; 32] = *cert_hash.as_ref();

    let chain = CertificateChain::single(certificate);
    let private_key = PrivateKey::from_der_pkcs8(key_der.to_vec());
    let identity = Identity::new(chain, private_key);

    let ip = addr.ip();
    let port = addr.port();

    // Use dual-stack (IPv4 + IPv6) for loopback/unspecified so that both
    // `localhost` (::1) and `127.0.0.1` work. For a specific IP, bind directly.
    let builder = ServerConfig::builder();
    let builder = if ip.is_loopback() {
        builder.with_bind_config(IpBindConfig::LocalDual, port)
    } else if ip.is_unspecified() {
        builder.with_bind_config(IpBindConfig::InAddrAnyDual, port)
    } else {
        builder.with_bind_address(addr)
    };

    let config = builder
        .with_identity(identity)
        .max_idle_timeout(Some(std::time::Duration::from_secs(300)))
        .map_err(|e| format!("invalid idle timeout: {e}"))?
        .keep_alive_interval(Some(std::time::Duration::from_secs(5)))
        .build();

    Ok(WtServerConfig {
        config,
        cert_hash: hash_bytes,
    })
}

/// Run the WebTransport server loop.
///
/// Accepts WebTransport sessions, performs the version handshake on the first
/// bidirectional stream, then handles request/response pairs on subsequent
/// streams — identical to the QUIC server loop.
pub async fn run_webtransport_server<S: Storage + Send + Sync + 'static>(
    config: ServerConfig,
    storage: Arc<S>,
    notify_tx: broadcast::Sender<RootState>,
) {
    let endpoint = match Endpoint::server(config) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("failed to create WebTransport endpoint: {e}");
            return;
        }
    };

    loop {
        let incoming = endpoint.accept().await;
        let storage = Arc::clone(&storage);
        let notify_tx = notify_tx.clone();

        tokio::spawn(async move {
            let session_request = match incoming.await {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("WebTransport session failed: {e}");
                    return;
                }
            };
            println!("WebTransport: new session request");

            let connection = match session_request.accept().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("WebTransport accept failed: {e}");
                    return;
                }
            };
            println!("WebTransport: session accepted");

            // --- Version handshake on the first bidirectional stream ---
            let (mut send, mut recv) = match connection.accept_bi().await {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("WebTransport accept_bi failed: {e}");
                    return;
                }
            };
            println!("WebTransport: bi-stream accepted, starting handshake");
            if let Err(e) = server_handshake(&mut send, &mut recv, &SERVER_VERSION).await {
                eprintln!("WebTransport handshake failed: {e}");
                return;
            }
            println!("WebTransport: handshake complete, flushing");
            let _ = send.flush().await;
            let _ = send.finish().await;
            println!("WebTransport: handshake stream finished, ready for requests");

            // --- Normal request/response loop ---
            loop {
                let stream = connection.accept_bi().await;
                match stream {
                    Ok((mut send, mut recv)) => {
                        let storage = Arc::clone(&storage);
                        let notify_tx = notify_tx.clone();
                        tokio::spawn(async move {
                            let request: Request = match read_message(&mut recv).await {
                                Ok(r) => r,
                                Err(e) => {
                                    eprintln!("WebTransport read request failed: {e}");
                                    return;
                                }
                            };

                            if matches!(request, Request::Subscribe) {
                                // Subscribe: keep the stream open and push notifications.
                                if write_message(&mut send, &Response::Ok).await.is_err() {
                                    return;
                                }
                                let _ = send.flush().await;
                                let mut rx = notify_tx.subscribe();
                                loop {
                                    match rx.recv().await {
                                        Ok(root) => {
                                            if write_message(
                                                &mut send,
                                                &Response::Notification(root),
                                            )
                                            .await
                                            .is_err()
                                            {
                                                break; // client gone
                                            }
                                            let _ = send.flush().await;
                                        }
                                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                        Err(broadcast::error::RecvError::Closed) => break,
                                    }
                                }
                                return;
                            }

                            let is_mutation = matches!(
                                request,
                                Request::PushCardVersions(_)
                                    | Request::PushTagVersions(_)
                                    | Request::PushBatch(_)
                                    | Request::DeleteCard { .. }
                                    | Request::DeleteTag { .. }
                            );
                            let response = handle_request(storage.as_ref(), request);
                            let succeeded = !matches!(response, Response::Error(_));
                            let _ = write_message(&mut send, &response).await;
                            let _ = send.flush().await;
                            let _ = send.finish().await;

                            if is_mutation
                                && succeeded
                                && let Ok(root) = storage.get_root()
                            {
                                let _ = notify_tx.send(root);
                            }
                        });
                    }
                    Err(_) => break, // connection closed
                }
            }
        });
    }
}

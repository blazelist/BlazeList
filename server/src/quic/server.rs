//! QUIC server loop and client helper.

use std::sync::Arc;
use std::time::Duration;

use blazelist_protocol::handshake::{HandshakeError, client_handshake, server_handshake};
use blazelist_protocol::wire::{WireError, read_message, write_message};
use blazelist_protocol::{ProtocolError, Request, Response, RootState};
use quinn::Endpoint;
use tokio::sync::broadcast;

use crate::SERVER_VERSION;
use crate::handler::handle_request;
use crate::storage::Storage;

/// Maximum time allowed for the version handshake to complete.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Run the QUIC server loop. Accepts connections and processes requests.
///
/// Uses the provided broadcast channel for change notifications. When a
/// client sends [`Request::Subscribe`], the server keeps the stream open and
/// pushes [`Response::Notification`] messages whenever the root state changes.
pub async fn run_server<S: Storage + Send + Sync + 'static>(
    endpoint: Endpoint,
    storage: Arc<S>,
    notify_tx: broadcast::Sender<RootState>,
) {
    while let Some(incoming) = endpoint.accept().await {
        let storage = Arc::clone(&storage);
        let notify_tx = notify_tx.clone();
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("connection failed: {e}");
                    return;
                }
            };

            // --- Version handshake with timeout ---
            let handshake = async {
                let (mut send, mut recv) = connection.accept_bi().await.map_err(|_| ())?;
                server_handshake(&mut send, &mut recv, &SERVER_VERSION)
                    .await
                    .map_err(|_| ())?;
                let _ = send.finish();
                Ok::<(), ()>(())
            };
            match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await {
                Ok(Ok(())) => {}
                _ => return,
            }

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
                                Err(WireError::Deserialize) => {
                                    let _ = write_message(
                                        &mut send,
                                        &Response::Error(ProtocolError::UnsupportedRequest),
                                    )
                                    .await;
                                    let _ = send.finish();
                                    return;
                                }
                                Err(_) => return,
                            };

                            if matches!(request, Request::Subscribe) {
                                // Subscribe: keep the stream open and push notifications.
                                if write_message(&mut send, &Response::Ok).await.is_err() {
                                    return;
                                }
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
                            let _ = send.finish();

                            if is_mutation
                                && succeeded
                                && let Ok(root) = storage.get_root()
                            {
                                // Ignore send errors — no subscribers is fine.
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

/// Perform the client-side version handshake and then send a single request.
/// Used by integration tests; production clients use `Client::connect`.
pub async fn send_request(
    connection: &quinn::Connection,
    request: &Request,
) -> Result<Response, WireError> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|_| WireError::StreamClosed)?;
    write_message(&mut send, request).await?;
    send.finish().map_err(|_| WireError::WriteFailed)?;
    read_message(&mut recv).await
}

/// Perform the version handshake on a fresh connection.
/// Returns `Ok(())` if versions match, or an error on mismatch / wire failure.
pub async fn perform_version_handshake(
    connection: &quinn::Connection,
) -> Result<(), HandshakeError> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|_| HandshakeError::Wire(WireError::StreamClosed))?;
    client_handshake(&mut send, &mut recv, &SERVER_VERSION).await
}

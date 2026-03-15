//! High-level BlazeList client for the WASM PWA.
//!
//! Mirrors the CLI's `client.rs` but uses WebTransport (via web-sys)
//! instead of QUIC (via quinn). Each request opens a new bidirectional
//! stream, sends the request, and reads the response.

use blazelist_client_lib::error::ClientError;
use blazelist_protocol::{Request, Response, RootState, Version};
use blazelist_protocol::{VersionCheck, VersionResult};

use super::connection::WtConnection;
use super::wire::{self, BufReader};

/// The client's protocol version, derived from Cargo package metadata at
/// compile time.
const CLIENT_VERSION: Version = blazelist_protocol::PROTOCOL_VERSION;

/// A BlazeList client connected over WebTransport.
pub struct Client {
    connection: WtConnection,
}

impl blazelist_client_lib::client::Client for Client {
    async fn request(&self, req: &Request) -> Result<Response, ClientError> {
        let (writer, reader) = self
            .connection
            .open_bi()
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        let mut reader = BufReader::new(reader);
        wire::write_message(&writer, req)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        let resp: Response = wire::read_message(&mut reader)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        Ok(resp)
    }
}

impl Client {
    /// Connect to a BlazeList server and perform the version handshake.
    ///
    /// `url` is the WebTransport URL (e.g. `https://localhost:47400`).
    /// `cert_hash` is the SHA-256 digest of the server's self-signed certificate.
    pub async fn connect(url: &str, cert_hash: &[u8]) -> Result<Self, ClientError> {
        tracing::info!(%url, "Connecting");
        let connection = WtConnection::connect(url, cert_hash)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        tracing::info!("WebTransport connected, opening bidirectional stream for handshake");

        // Perform the version handshake on the first bidirectional stream.
        let (writer, reader) = connection
            .open_bi()
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        let mut reader = BufReader::new(reader);
        tracing::info!("Bidirectional stream open, sending version check");
        let check = VersionCheck {
            version: CLIENT_VERSION.clone(),
        };
        wire::write_message(&writer, &check)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        tracing::info!("Version check sent, reading response");
        let result: VersionResult = wire::read_message(&mut reader)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        tracing::info!("Handshake response received");

        match result {
            VersionResult::Ok => {}
            VersionResult::Mismatch { server_version } => {
                return Err(ClientError::VersionMismatch { server_version });
            }
        }

        Ok(Self { connection })
    }

    /// Subscribe to real-time change notifications.
    ///
    /// Opens a bidirectional stream, sends a `Subscribe` request, reads the
    /// initial `Ok` response, and returns a [`SubscribeHandle`] that can be
    /// used to read subsequent `Notification` messages.
    pub async fn subscribe(&self) -> Result<SubscribeHandle, ClientError> {
        let (writer, reader) = self
            .connection
            .open_bi()
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        let mut reader = BufReader::new(reader);
        wire::write_message(&writer, &Request::Subscribe)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;

        let resp: Response = wire::read_message(&mut reader)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        match resp {
            Response::Ok => {}
            Response::Error(e) => return Err(ClientError::Protocol(e)),
            _ => return Err(ClientError::UnexpectedResponse),
        }

        Ok(SubscribeHandle { reader })
    }
}

/// A handle to an open subscription stream.
///
/// The server pushes `Notification(RootState)` messages on this stream
/// whenever any mutation occurs. Call [`next_notification`](SubscribeHandle::next_notification)
/// to await the next one.
pub struct SubscribeHandle {
    reader: BufReader,
}

impl SubscribeHandle {
    /// Read the next notification from the subscription stream.
    ///
    /// Returns the new [`RootState`] after a server-side mutation, or an
    /// error if the stream was closed or a non-notification message was
    /// received.
    pub async fn next_notification(&mut self) -> Result<RootState, ClientError> {
        let resp: Response = wire::read_message(&mut self.reader)
            .await
            .map_err(|_| ClientError::ConnectionLost)?;
        match resp {
            Response::Notification(root) => Ok(root),
            Response::Error(e) => Err(ClientError::Protocol(e)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }
}

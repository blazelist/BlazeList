//! Shared client error type for BlazeList clients.
//!
//! [`ClientError`] captures the four actionable states a client can encounter:
//! protocol errors from the server, unexpected response variants, version
//! mismatches, and connection loss.

use blazelist_protocol::{ProtocolError, ResponseExtractError, Version};

/// Errors that can occur during client operations.
#[derive(Debug)]
pub enum ClientError {
    /// The server returned a protocol-level error.
    Protocol(ProtocolError),
    /// The response variant did not match what was expected.
    UnexpectedResponse,
    /// Version mismatch during handshake.
    VersionMismatch { server_version: Version },
    /// Connection lost, timed out, or could not be established.
    ConnectionLost,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Protocol(e) => write!(f, "protocol error: {e}"),
            ClientError::UnexpectedResponse => write!(f, "unexpected response variant"),
            ClientError::VersionMismatch { server_version } => {
                write!(f, "version mismatch: server={server_version}")
            }
            ClientError::ConnectionLost => write!(f, "connection lost"),
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ClientError::Protocol(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ProtocolError> for ClientError {
    fn from(e: ProtocolError) -> Self {
        ClientError::Protocol(e)
    }
}

impl From<ResponseExtractError> for ClientError {
    fn from(e: ResponseExtractError) -> Self {
        match e {
            ResponseExtractError::Protocol(p) => ClientError::Protocol(p),
            ResponseExtractError::UnexpectedVariant => ClientError::UnexpectedResponse,
        }
    }
}

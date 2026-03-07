use crate::Version;

use crate::wire::WireError;

/// Errors that can occur during the version handshake.
#[derive(Debug)]
pub enum HandshakeError {
    /// The remote side has an incompatible protocol version.
    VersionMismatch { local: Version, remote: Version },
    /// A wire-level error occurred during the handshake.
    Wire(WireError),
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandshakeError::VersionMismatch { local, remote } => {
                write!(
                    f,
                    "protocol version mismatch: local={local}, remote={remote}"
                )
            }
            HandshakeError::Wire(e) => write!(f, "handshake wire error: {e}"),
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HandshakeError::Wire(e) => Some(e),
            _ => None,
        }
    }
}

impl From<WireError> for HandshakeError {
    fn from(e: WireError) -> Self {
        HandshakeError::Wire(e)
    }
}

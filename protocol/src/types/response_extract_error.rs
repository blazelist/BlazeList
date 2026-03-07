use super::protocol_error::ProtocolError;

/// Error returned when extracting a specific variant from a [`super::Response`].
///
/// This type separates two distinct failure modes:
/// - [`Protocol`](ResponseExtractError::Protocol) — the server returned an
///   error response. The contained [`ProtocolError`] traveled over the wire.
/// - [`UnexpectedVariant`](ResponseExtractError::UnexpectedVariant) — the
///   caller used the wrong extraction method for the response variant received.
///   This is a client-side programming error that never goes over the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseExtractError {
    /// The server explicitly returned an error.
    Protocol(ProtocolError),
    /// The response variant did not match what the extraction method expected.
    UnexpectedVariant,
}

impl std::fmt::Display for ResponseExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseExtractError::Protocol(e) => write!(f, "{e}"),
            ResponseExtractError::UnexpectedVariant => {
                write!(f, "unexpected response variant")
            }
        }
    }
}

impl std::error::Error for ResponseExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ResponseExtractError::Protocol(e) => Some(e),
            ResponseExtractError::UnexpectedVariant => None,
        }
    }
}

impl From<ProtocolError> for ResponseExtractError {
    fn from(e: ProtocolError) -> Self {
        ResponseExtractError::Protocol(e)
    }
}

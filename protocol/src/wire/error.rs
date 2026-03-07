/// Wire-level errors.
#[derive(Debug)]
pub enum WireError {
    /// The remote side closed the stream before a complete message was read.
    StreamClosed,
    /// The incoming message exceeds [`super::MAX_MSG_SIZE`].
    MessageTooLarge,
    /// Postcard deserialization failed.
    Deserialize,
    /// Postcard serialization failed.
    Serialize,
    /// Writing to the stream failed.
    WriteFailed,
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::StreamClosed => write!(f, "stream closed"),
            WireError::MessageTooLarge => write!(f, "message too large"),
            WireError::Deserialize => write!(f, "deserialization error"),
            WireError::Serialize => write!(f, "serialization error"),
            WireError::WriteFailed => write!(f, "write failed"),
        }
    }
}

impl std::error::Error for WireError {}

use crate::NonNegativeI64;
use serde::{Deserialize, Serialize};

use super::batch_item_error::BatchItemError;
use super::push_error::PushError;

/// Protocol-level errors returned by the server.
///
/// These are structured error responses that a client can match on
/// programmatically. The generic [`Error`](ProtocolError::Error) variant
/// is reserved for internal server failures.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolError {
    /// Entity not found.
    NotFound,

    /// Entity was already deleted.
    AlreadyDeleted,

    /// A push operation failed with a typed domain error.
    PushFailed(PushError),

    /// A batch push failed at the given item index. The entire batch was
    /// rolled back. The `error` carries the full typed error for that item.
    BatchFailed { index: u32, error: BatchItemError },

    /// Root hash mismatch detected during incremental sync.
    /// The client's claimed root hash at the specified sequence does not match
    /// the server's history. The client's state is corrupted and needs a full
    /// re-sync rather than applying a delta.
    RootHashMismatch {
        sequence: NonNegativeI64,
        expected_hash: blake3::Hash,
    },

    /// The server received a request variant it does not understand.
    UnsupportedRequest,

    /// An internal server error.
    Internal,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::NotFound => write!(f, "entity not found"),
            ProtocolError::AlreadyDeleted => write!(f, "entity already deleted"),
            ProtocolError::PushFailed(e) => write!(f, "push failed: {e}"),
            ProtocolError::BatchFailed { index, error } => {
                write!(f, "batch push failed at index {index}: {error}")
            }
            ProtocolError::RootHashMismatch {
                sequence,
                expected_hash,
            } => write!(
                f,
                "root hash mismatch at sequence {sequence}: expected {expected_hash}"
            ),
            ProtocolError::UnsupportedRequest => write!(f, "unsupported request"),
            ProtocolError::Internal => write!(f, "internal error"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<PushError> for ProtocolError {
    fn from(e: PushError) -> Self {
        ProtocolError::PushFailed(e)
    }
}

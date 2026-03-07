use serde::{Deserialize, Serialize};

use super::push_error::PushError;

/// Error for an individual item in a batch push.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchItemError {
    /// A push domain error (ancestor mismatch, hash verification, etc.).
    Push(PushError),
    /// The delete target was not found.
    NotFound,
    /// The delete target was already deleted.
    AlreadyDeleted,
    /// An unexpected server-side error.
    Internal,
}

impl std::fmt::Display for BatchItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchItemError::Push(e) => write!(f, "{e}"),
            BatchItemError::NotFound => write!(f, "entity not found"),
            BatchItemError::AlreadyDeleted => write!(f, "entity already deleted"),
            BatchItemError::Internal => write!(f, "internal error"),
        }
    }
}

impl std::error::Error for BatchItemError {}

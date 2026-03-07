use crate::{Card, NonNegativeI64, Tag};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Errors returned by push operations.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushError {
    /// The ancestor hash of the first pushed version does not match the
    /// server's latest hash for the entity.
    CardAncestorMismatch(Box<Card>),
    TagAncestorMismatch(Box<Tag>),
    /// The entity has been deleted.
    AlreadyDeleted,
    /// A pushed version failed hash verification.
    HashVerificationFailed,
    /// The version chain was empty.
    EmptyChain,
    /// Another card already has this priority value. The conflicting card's
    /// UUID and priority are returned so the client can resolve the collision.
    DuplicatePriority {
        conflicting_id: Uuid,
        priority: NonNegativeI64,
    },
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::CardAncestorMismatch(_) => write!(f, "card ancestor hash mismatch"),
            PushError::TagAncestorMismatch(_) => write!(f, "tag ancestor hash mismatch"),
            PushError::AlreadyDeleted => write!(f, "entity already deleted"),
            PushError::HashVerificationFailed => write!(f, "hash verification failed"),
            PushError::EmptyChain => write!(f, "empty version chain"),
            PushError::DuplicatePriority {
                conflicting_id,
                priority,
            } => write!(
                f,
                "duplicate priority {priority} (conflicts with {conflicting_id})"
            ),
        }
    }
}

impl std::error::Error for PushError {}

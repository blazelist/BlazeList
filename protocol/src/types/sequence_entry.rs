use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::NonNegativeI64;

/// The kind of operation that occurred in a sequence entry.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceOperationKind {
    CardCreated,
    CardUpdated,
    TagCreated,
    TagUpdated,
    EntityDeleted,
}

/// A single operation within a sequence entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceOperation {
    pub entity_id: Uuid,
    pub kind: SequenceOperationKind,
}

/// One entry in the full sequence history — represents a single root
/// state transition along with the operations that caused it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceHistoryEntry {
    pub sequence: NonNegativeI64,
    pub hash: blake3::Hash,
    pub operations: Vec<SequenceOperation>,
    /// Server-side UTC wall-clock time when this sequence entry was recorded.
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
}

use crate::{Card, DeletedEntity, RootState, Tag};
use serde::{Deserialize, Serialize};

/// A set of changes since a given root sequence, used for incremental sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    /// Cards that were created or updated since the given root sequence.
    pub cards: Vec<Card>,
    /// Tags that were created or updated since the given root sequence.
    pub tags: Vec<Tag>,
    /// Entities that were deleted since the given root sequence.
    pub deleted: Vec<DeletedEntity>,
    /// The current root state (after all changes).
    pub root: RootState,
}

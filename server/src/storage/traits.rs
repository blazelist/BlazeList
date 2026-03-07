//! Abstract storage trait.

use blazelist_protocol::{
    Card, DeletedEntity, NonNegativeI64, RootState, SequenceHistoryEntry, Tag,
};
use blazelist_protocol::{CardFilter, ChangeSet, PushItem};
use uuid::Uuid;

use super::error::{BatchError, PushOpError, StorageError};

/// Abstract storage backend for the Blaze List server.
pub trait Storage {
    // -- Cards ---------------------------------------------------------------
    fn push_card_versions(&self, versions: &[Card]) -> Result<(), PushOpError>;
    fn get_card(&self, id: Uuid) -> Result<Option<Card>, StorageError>;
    fn get_card_history(&self, id: Uuid, limit: Option<u32>) -> Result<Vec<Card>, StorageError>;
    fn list_cards(&self, filter: CardFilter, limit: Option<u32>)
    -> Result<Vec<Card>, StorageError>;
    fn delete_card(&self, id: Uuid) -> Result<DeletedEntity, StorageError>;

    // -- Tags ----------------------------------------------------------------
    fn push_tag_versions(&self, versions: &[Tag]) -> Result<(), PushOpError>;
    fn get_tag(&self, id: Uuid) -> Result<Option<Tag>, StorageError>;
    fn get_tag_history(&self, id: Uuid, limit: Option<u32>) -> Result<Vec<Tag>, StorageError>;
    fn list_tags(&self) -> Result<Vec<Tag>, StorageError>;
    fn delete_tag(&self, id: Uuid) -> Result<DeletedEntity, StorageError>;

    // -- Root ----------------------------------------------------------------
    fn get_root(&self) -> Result<RootState, StorageError>;

    // -- Sync ----------------------------------------------------------------
    /// Return all entities that changed since the given root sequence.
    /// Validates that the hash at the client's claimed sequence matches the
    /// expected hash. Returns an error if the hashes don't match (client state
    /// corrupted).
    fn get_changes_since(
        &self,
        sequence: NonNegativeI64,
        expected_hash: blake3::Hash,
    ) -> Result<ChangeSet, StorageError>;

    // -- Batch ---------------------------------------------------------------
    /// Atomically push multiple items (cards, tags, deletions) in a single
    /// transaction. If any item fails, the entire batch is rolled back.
    fn push_batch(&self, items: &[PushItem]) -> Result<(), BatchError>;

    // -- History -------------------------------------------------------------
    /// Get the sequence history — every root state transition along with
    /// the operations that caused it.
    ///
    /// Use `after_sequence` to paginate (only entries with sequence < value
    /// are returned) and `limit` to cap the result count.
    fn get_sequence_history(
        &self,
        after_sequence: Option<NonNegativeI64>,
        limit: Option<u32>,
    ) -> Result<Vec<SequenceHistoryEntry>, StorageError>;
}

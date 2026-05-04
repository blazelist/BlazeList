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
    /// Push a chain of card versions.
    ///
    /// The server does **not** auto-add transitively-implied tags to the
    /// card: if the final version's tag set is not closed under the tag
    /// implication graph, the push is rejected with
    /// [`PushError::TagImplicationViolation`]. Clients must compute the
    /// closure themselves (via
    /// [`blazelist_protocol::TagGraph::missing_for_card`]) and submit a
    /// version that already includes every implied tag. This keeps the
    /// BLAKE3 hash chain authored by the client, so the server never has
    /// to fabricate a version on the user's behalf.
    ///
    /// [`PushError::TagImplicationViolation`]: blazelist_protocol::PushError::TagImplicationViolation
    fn push_card_versions(&self, versions: &[Card]) -> Result<(), PushOpError>;
    fn get_card(&self, id: Uuid) -> Result<Option<Card>, StorageError>;
    fn get_card_history(&self, id: Uuid, limit: Option<u32>) -> Result<Vec<Card>, StorageError>;
    fn list_cards(&self, filter: CardFilter, limit: Option<u32>)
    -> Result<Vec<Card>, StorageError>;
    fn delete_card(&self, id: Uuid) -> Result<DeletedEntity, StorageError>;

    // -- Tags ----------------------------------------------------------------
    /// Push a chain of tag versions.
    ///
    /// The same tag-implication validator that runs on card pushes also
    /// runs here: if the new `implies` list introduces a cycle, references
    /// an unknown/deleted tag, or leaves any live card non-compliant under
    /// the post-push graph, the push is rejected. The server never
    /// rewrites cards as a side effect of a tag push — clients that want
    /// to change implies and simultaneously bring cards into compliance
    /// must use `push_batch` with the card updates included.
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
    ///
    /// After every per-item write succeeds, the tag-implication validator
    /// runs over the post-batch state (live tags + batch overlay):
    /// cycles, dangling-implies references, and non-compliant cards all
    /// cause the whole batch to roll back. This is the mechanism callers
    /// use to co-push "change tag X's implies + update the N cards that
    /// would otherwise become non-compliant" atomically — the server
    /// will not compute the N card updates for you.
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

    /// Get version histories for all (or selected) cards in bulk.
    ///
    /// If `card_ids` is `Some`, only histories for those cards are returned.
    /// If `limit_per_card` is `Some`, each card's history is capped.
    fn get_all_card_histories(
        &self,
        limit_per_card: Option<u32>,
        card_ids: Option<&[Uuid]>,
    ) -> Result<std::collections::HashMap<Uuid, Vec<Card>>, StorageError>;

    /// Get version histories for all (or selected) tags in bulk.
    ///
    /// If `tag_ids` is `Some`, only histories for those tags are returned.
    /// If `limit_per_tag` is `Some`, each tag's history is capped.
    fn get_all_tag_histories(
        &self,
        limit_per_tag: Option<u32>,
        tag_ids: Option<&[Uuid]>,
    ) -> Result<std::collections::HashMap<Uuid, Vec<Tag>>, StorageError>;
}

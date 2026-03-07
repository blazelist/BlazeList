use crate::{Card, NonNegativeI64, Tag};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::card_filter::CardFilter;
use super::push_item::PushItem;

/// A client request.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
///
/// # Streaming
/// Most variants follow the single request / single response pattern.
/// The exception is [`Subscribe`](Request::Subscribe), which keeps the
/// stream open for push notifications — use [`is_streaming`](Request::is_streaming)
/// to distinguish the two modes at the transport layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Push a chain of card versions. The first version's ancestor hash
    /// must match the server's latest hash for that card (or be the zero
    /// hash for a new card).
    ///
    /// On success the server responds with [`Response::Root`](super::Response::Root)
    /// containing the new root state.
    PushCardVersions(Vec<Card>),

    /// Get the latest version of a card by UUID.
    GetCard { id: Uuid },

    /// Get the full version history of a card.
    GetCardHistory { id: Uuid, limit: Option<u32> },

    /// List cards matching the given filter.
    ListCards {
        filter: CardFilter,
        limit: Option<u32>,
    },

    /// Delete a card by UUID.
    DeleteCard { id: Uuid },

    /// Push a chain of tag versions.
    ///
    /// On success the server responds with [`Response::Root`](super::Response::Root)
    /// containing the new root state.
    PushTagVersions(Vec<Tag>),

    /// Get a tag by UUID.
    GetTag { id: Uuid },

    /// List all tags.
    ListTags,

    /// Delete a tag by UUID.
    DeleteTag { id: Uuid },

    /// Get the current root state (hash + sequence).
    GetRoot,

    /// Get all entities that changed since the given root sequence.
    /// Returns a [`ChangeSet`](super::ChangeSet) with only the cards, tags, and deletions
    /// that occurred after the specified sequence.
    ///
    /// The server validates that the client's state at the claimed sequence
    /// matches the expected hash. If the hashes don't match, the server returns
    /// `RootHashMismatch` indicating that the client's state is corrupted and
    /// needs a full re-sync.
    GetChangesSince {
        sequence: NonNegativeI64,
        root_hash: blake3::Hash,
    },

    /// Atomically push multiple items (cards, tags, deletions) in a single
    /// transaction. If any item fails, the entire batch is rolled back.
    ///
    /// On success the server responds with [`Response::Root`](super::Response::Root)
    /// containing the new root state.
    PushBatch(Vec<PushItem>),

    /// Subscribe to change notifications. The server responds with
    /// [`Response::Ok`](super::Response::Ok) and then keeps the stream open,
    /// sending [`Response::Notification`](super::Response::Notification)
    /// messages whenever the root state changes (i.e. after any mutation).
    ///
    /// Unlike all other variants, this request keeps the bidirectional stream
    /// open indefinitely. Use [`is_streaming`](Request::is_streaming) to detect
    /// this at the transport layer.
    Subscribe,

    /// Get the full version history of a tag.
    GetTagHistory { id: Uuid, limit: Option<u32> },

    /// Get the sequence history — every root state transition
    /// along with the operations that caused it.
    GetSequenceHistory {
        after_sequence: Option<NonNegativeI64>,
        limit: Option<u32>,
    },
}

impl Request {
    /// Returns `true` if this request keeps the stream open for push
    /// notifications rather than following the standard single-response
    /// pattern.
    pub fn is_streaming(&self) -> bool {
        matches!(self, Request::Subscribe)
    }

    /// Returns `true` if this request mutates server state (pushes,
    /// deletions). Useful at the transport layer for triggering change
    /// notifications to subscribed clients.
    pub fn is_mutation(&self) -> bool {
        matches!(
            self,
            Request::PushCardVersions(_)
                | Request::PushTagVersions(_)
                | Request::PushBatch(_)
                | Request::DeleteCard { .. }
                | Request::DeleteTag { .. }
        )
    }
}

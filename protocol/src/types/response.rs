use crate::{Card, DeletedEntity, RootState, Tag};
use serde::{Deserialize, Serialize};

use super::change_set::ChangeSet;
use super::protocol_error::ProtocolError;
use super::response_extract_error::ResponseExtractError;
use super::sequence_entry::SequenceHistoryEntry;

/// A server response.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// Operation succeeded with no payload.
    Ok,

    /// A single card.
    Card(Card),

    /// A list of cards.
    Cards(Vec<Card>),

    /// A single tag.
    Tag(Tag),

    /// A list of tags.
    Tags(Vec<Tag>),

    /// Root state.
    Root(RootState),

    /// Entity was deleted.
    Deleted(DeletedEntity),

    /// A set of changes since a given root count.
    Changes(ChangeSet),

    /// A change notification pushed by the server to subscribed clients.
    /// Contains the new root state after a mutation.
    Notification(RootState),

    /// A protocol error.
    Error(ProtocolError),

    /// Version history for a single card.
    CardHistory(Vec<Card>),

    /// Version history for a single tag.
    TagHistory(Vec<Tag>),

    /// Full sequence history.
    SequenceHistory(Vec<SequenceHistoryEntry>),
}

/// Generate a `Response` extraction method that unwraps a specific variant
/// or returns a `ResponseExtractError`.
macro_rules! extract {
    ($(#[$meta:meta])* $method:ident -> $variant:ident => $ty:ty) => {
        $(#[$meta])*
        pub fn $method(self) -> Result<$ty, ResponseExtractError> {
            match self {
                Self::$variant(val) => Ok(val),
                Self::Error(e) => Err(ResponseExtractError::Protocol(e)),
                _ => Err(ResponseExtractError::UnexpectedVariant),
            }
        }
    };
}

impl Response {
    /// Returns `Ok(())` if this is `Response::Ok`, or the contained
    /// [`ProtocolError`] otherwise.
    pub fn into_ok(self) -> Result<(), ResponseExtractError> {
        match self {
            Self::Ok => Ok(()),
            Self::Error(e) => Err(ResponseExtractError::Protocol(e)),
            _ => Err(ResponseExtractError::UnexpectedVariant),
        }
    }

    extract!(
        /// Extract a single [`Card`] from this response.
        into_card -> Card => Card
    );
    extract!(
        /// Extract a list of [`Card`]s from this response.
        into_cards -> Cards => Vec<Card>
    );
    extract!(
        /// Extract a single [`Tag`] from this response.
        into_tag -> Tag => Tag
    );
    extract!(
        /// Extract a list of [`Tag`]s from this response.
        into_tags -> Tags => Vec<Tag>
    );
    extract!(
        /// Extract the [`RootState`] from this response.
        into_root -> Root => RootState
    );
    extract!(
        /// Extract the [`DeletedEntity`] from this response.
        into_deleted -> Deleted => DeletedEntity
    );
    extract!(
        /// Extract the [`ChangeSet`] from this response.
        into_changes -> Changes => ChangeSet
    );
    extract!(
        /// Extract the [`RootState`] from a notification response.
        into_notification -> Notification => RootState
    );
    extract!(
        /// Extract card version history from this response.
        into_card_history -> CardHistory => Vec<Card>
    );
    extract!(
        /// Extract tag version history from this response.
        into_tag_history -> TagHistory => Vec<Tag>
    );
    extract!(
        /// Extract the sequence history from this response.
        into_sequence_history -> SequenceHistory => Vec<SequenceHistoryEntry>
    );
}

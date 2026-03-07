use crate::{Card, Tag};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single item in a batch push.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushItem {
    /// Push a chain of card versions.
    Cards(Vec<Card>),
    /// Push a chain of tag versions.
    Tags(Vec<Tag>),
    /// Delete a card by UUID.
    DeleteCard { id: Uuid },
    /// Delete a tag by UUID.
    DeleteTag { id: Uuid },
}

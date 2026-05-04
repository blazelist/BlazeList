use crate::{Card, Tag};
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
    /// A tag cannot be deleted while cards still reference it.
    OrphanedTagReference {
        tag_id: Uuid,
        referencing_card_ids: Vec<Uuid>,
    },
    /// A pushed version failed hash verification.
    HashVerificationFailed,
    /// The version chain was empty.
    EmptyChain,
    /// Another card already has this priority value. The conflicting card's
    /// UUID and priority are returned so the client can resolve the collision.
    DuplicatePriority {
        conflicting_id: Uuid,
        priority: i64,
    },
    /// A card's tag set was not closed under the tag implication relation:
    /// at least one transitively-implied tag was missing. The server includes
    /// the offending card id and the sorted list of tag ids that must be
    /// added for the card to become compliant.
    TagImplicationViolation {
        card_id: Uuid,
        missing: Vec<Uuid>,
    },
    /// A tag push (or batch containing tag updates) would introduce a cycle
    /// in the implication graph. The `cycle` field lists the tag ids along
    /// the discovered cycle, starting and ending with the same id so the
    /// round-trip is explicit.
    TagImplicationCycle {
        cycle: Vec<Uuid>,
    },
    /// A tag cannot be deleted while another live tag still lists it in
    /// its `implies` set. Without this check, a card holding the implying
    /// tag would permanently trip [`PushError::TagImplicationViolation`]
    /// because the implied target has been moved to `deleted_entities`.
    OrphanedTagImpliesReference {
        tag_id: Uuid,
        referencing_tag_ids: Vec<Uuid>,
    },
    /// A pushed tag's `implies` list references one or more tag ids that
    /// are not present as live tags (either never seen or already
    /// deleted). `tag_id` is the offending tag; `missing` is the sorted
    /// list of unknown/deleted parent ids.
    TagImpliesUnknown {
        tag_id: Uuid,
        missing: Vec<Uuid>,
    },
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::CardAncestorMismatch(_) => write!(f, "card ancestor hash mismatch"),
            PushError::TagAncestorMismatch(_) => write!(f, "tag ancestor hash mismatch"),
            PushError::AlreadyDeleted => write!(f, "entity already deleted"),
            PushError::OrphanedTagReference {
                tag_id,
                referencing_card_ids,
            } => write!(
                f,
                "tag {tag_id} cannot be deleted: still referenced by {} card(s)",
                referencing_card_ids.len()
            ),
            PushError::HashVerificationFailed => write!(f, "hash verification failed"),
            PushError::EmptyChain => write!(f, "empty version chain"),
            PushError::DuplicatePriority {
                conflicting_id,
                priority,
            } => write!(
                f,
                "duplicate priority {priority} (conflicts with {conflicting_id})"
            ),
            PushError::TagImplicationViolation { card_id, missing } => write!(
                f,
                "card {card_id} is missing {} transitively implied tag(s)",
                missing.len()
            ),
            PushError::TagImplicationCycle { cycle } => write!(
                f,
                "tag implication graph contains a cycle ({} nodes)",
                cycle.len().saturating_sub(1)
            ),
            PushError::OrphanedTagImpliesReference {
                tag_id,
                referencing_tag_ids,
            } => write!(
                f,
                "tag {tag_id} cannot be deleted: still implied by {} tag(s)",
                referencing_tag_ids.len()
            ),
            PushError::TagImpliesUnknown { tag_id, missing } => write!(
                f,
                "tag {tag_id} implies {} unknown or deleted tag(s)",
                missing.len()
            ),
        }
    }
}

impl std::error::Error for PushError {}

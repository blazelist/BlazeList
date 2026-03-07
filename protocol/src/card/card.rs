use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::NonNegativeI64;
use crate::hash::{Entity, HashVerificationError, ZERO_HASH, canonical_card_hash};

/// A card in the Blaze List.
///
/// Every card carries its full state plus hash chain fields (`count`,
/// `ancestor_hash`, `hash`). Each stored instance is a snapshot at a
/// point in time — the chain of ancestor hashes links them into a
/// verifiable linear history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    id: Uuid,
    content: String,
    priority: NonNegativeI64,
    tags: Vec<Uuid>,
    blazed: bool,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    modified_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    due_date: Option<DateTime<Utc>>,
    count: NonNegativeI64,
    ancestor_hash: blake3::Hash,
    hash: blake3::Hash,
}

impl Entity for Card {
    fn id(&self) -> Uuid {
        self.id
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }

    fn count(&self) -> NonNegativeI64 {
        self.count
    }

    fn ancestor_hash(&self) -> blake3::Hash {
        self.ancestor_hash
    }

    fn hash(&self) -> blake3::Hash {
        self.hash
    }

    fn expected_hash(&self) -> blake3::Hash {
        canonical_card_hash(
            &self.id,
            &self.content,
            i64::from(self.priority),
            &self.tags,
            self.blazed,
            self.created_at.timestamp_millis(),
            self.modified_at.timestamp_millis(),
            i64::from(self.count),
            &self.ancestor_hash,
            self.due_date.map(|d| d.timestamp_millis()),
        )
    }
}

impl Card {
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn priority(&self) -> NonNegativeI64 {
        self.priority
    }

    pub fn tags(&self) -> &[Uuid] {
        &self.tags
    }

    pub fn blazed(&self) -> bool {
        self.blazed
    }

    pub fn due_date(&self) -> Option<DateTime<Utc>> {
        self.due_date
    }

    /// Reconstruct a card from all stored fields (e.g. from a database row).
    ///
    /// Returns an error if the stored hash does not match the computed hash.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        content: String,
        priority: NonNegativeI64,
        tags: Vec<Uuid>,
        blazed: bool,
        created_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
        count: NonNegativeI64,
        ancestor_hash: blake3::Hash,
        hash: blake3::Hash,
        due_date: Option<DateTime<Utc>>,
    ) -> Result<Self, HashVerificationError> {
        let card = Self {
            id,
            content,
            priority,
            tags,
            blazed,
            created_at,
            modified_at,
            due_date,
            count,
            ancestor_hash,
            hash,
        };
        if card.verify() {
            Ok(card)
        } else {
            Err(HashVerificationError)
        }
    }

    /// Create the first version of a new card.
    pub fn first(
        id: Uuid,
        content: String,
        priority: NonNegativeI64,
        mut tags: Vec<Uuid>,
        blazed: bool,
        created_at: DateTime<Utc>,
        due_date: Option<DateTime<Utc>>,
    ) -> Self {
        tags.sort();
        let mut card = Self {
            id,
            content,
            priority,
            tags,
            blazed,
            created_at,
            modified_at: created_at,
            due_date,
            count: NonNegativeI64::try_from(1i64).unwrap(),
            ancestor_hash: ZERO_HASH,
            hash: ZERO_HASH, // placeholder
        };
        card.hash = card.expected_hash();
        card
    }

    /// Create the next version in the chain.
    pub fn next(
        &self,
        content: String,
        priority: NonNegativeI64,
        mut tags: Vec<Uuid>,
        blazed: bool,
        modified_at: DateTime<Utc>,
        due_date: Option<DateTime<Utc>>,
    ) -> Self {
        tags.sort();
        let mut card = Self {
            id: self.id,
            content,
            priority,
            tags,
            blazed,
            created_at: self.created_at,
            modified_at,
            due_date,
            count: NonNegativeI64::try_from(u64::from(self.count) + 1).unwrap(),
            ancestor_hash: self.hash,
            hash: ZERO_HASH, // placeholder
        };
        card.hash = card.expected_hash();
        card
    }
}

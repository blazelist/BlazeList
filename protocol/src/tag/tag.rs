use chrono::{DateTime, Utc};
use rgb::RGB8;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::NonNegativeI64;
use crate::hash::{Entity, HashVerificationError, ZERO_HASH, canonical_tag_hash};

/// A tag that can be attached to cards.
///
/// Tags are identified by UUID and carry the same hash chain as cards.
/// Initially only have a title; future extensions: icons (Unicode or
/// custom) and description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    id: Uuid,
    title: String,
    color: Option<RGB8>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    modified_at: DateTime<Utc>,
    count: NonNegativeI64,
    ancestor_hash: blake3::Hash,
    hash: blake3::Hash,
}

impl Entity for Tag {
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
        canonical_tag_hash(
            &self.id,
            &self.title,
            self.created_at.timestamp_millis(),
            self.modified_at.timestamp_millis(),
            i64::from(self.count),
            &self.ancestor_hash,
            self.color.as_ref(),
        )
    }
}

impl Tag {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn color(&self) -> Option<RGB8> {
        self.color
    }

    /// Reconstruct a tag from all stored fields (e.g. from a database row).
    ///
    /// Returns an error if the stored hash does not match the computed hash.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        title: String,
        color: Option<RGB8>,
        created_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
        count: NonNegativeI64,
        ancestor_hash: blake3::Hash,
        hash: blake3::Hash,
    ) -> Result<Self, HashVerificationError> {
        let tag = Self {
            id,
            title,
            color,
            created_at,
            modified_at,
            count,
            ancestor_hash,
            hash,
        };
        if tag.verify() {
            Ok(tag)
        } else {
            Err(HashVerificationError)
        }
    }

    /// Create the first version of a new tag.
    pub fn first(id: Uuid, title: String, color: Option<RGB8>, created_at: DateTime<Utc>) -> Self {
        let mut tag = Self {
            id,
            title,
            color,
            created_at,
            modified_at: created_at,
            count: NonNegativeI64::try_from(1i64).unwrap(),
            ancestor_hash: ZERO_HASH,
            hash: ZERO_HASH, // placeholder
        };
        tag.hash = tag.expected_hash();
        tag
    }

    /// Create the next version in the chain.
    pub fn next(&self, title: String, color: Option<RGB8>, modified_at: DateTime<Utc>) -> Self {
        let mut tag = Self {
            id: self.id,
            title,
            color,
            created_at: self.created_at,
            modified_at,
            count: NonNegativeI64::try_from(u64::from(self.count) + 1).unwrap(),
            ancestor_hash: self.hash,
            hash: ZERO_HASH, // placeholder
        };
        tag.hash = tag.expected_hash();
        tag
    }
}

#[cfg(test)]
impl Tag {
    /// Test-only helper to tamper with the title without recomputing the hash.
    pub(crate) fn tamper_title(&mut self, title: String) {
        self.title = title;
    }
}

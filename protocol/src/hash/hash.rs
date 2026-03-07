use chrono::{DateTime, Utc};
use rgb::RGB8;
use uuid::Uuid;

use crate::NonNegativeI64;

/// Ancestor hash for the first version of an entity (32 zero bytes).
pub const ZERO_HASH: blake3::Hash = blake3::Hash::from_bytes([0u8; 32]);

/// Compute the canonical BLAKE3 hash of a card from its fields.
///
/// # Canonical format
///
/// Fields are written in a fixed order with explicit encodings:
///
/// ```text
/// [16 B: UUID] [8 B: content len u64 BE] [N B: content UTF-8]
/// [8 B: priority i64 BE] [8 B: tag count u64 BE] [16 B * M: tag UUIDs]
/// [1 B: blazed u8] [8 B: created_at ms i64 BE] [8 B: modified_at ms i64 BE]
/// [1 B: has_due_date u8] [8 B: due_date ms i64 BE (only if has_due_date == 1)]
/// [8 B: count i64 BE] [32 B: ancestor_hash]
/// ```
#[allow(clippy::too_many_arguments)]
pub fn canonical_card_hash(
    id: &Uuid,
    content: &str,
    priority: i64,
    tags: &[Uuid],
    blazed: bool,
    created_at_ms: i64,
    modified_at_ms: i64,
    count: i64,
    ancestor_hash: &blake3::Hash,
    due_date_ms: Option<i64>,
) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(id.as_bytes());
    hasher.update(&(content.len() as u64).to_be_bytes());
    hasher.update(content.as_bytes());
    hasher.update(&priority.to_be_bytes());
    let mut sorted_tags = tags.to_vec();
    sorted_tags.sort();
    hasher.update(&(sorted_tags.len() as u64).to_be_bytes());
    for tag in &sorted_tags {
        hasher.update(tag.as_bytes());
    }
    hasher.update(&[blazed as u8]);
    hasher.update(&created_at_ms.to_be_bytes());
    hasher.update(&modified_at_ms.to_be_bytes());
    match due_date_ms {
        None => {
            hasher.update(&[0u8]);
        }
        Some(ms) => {
            hasher.update(&[1u8]);
            hasher.update(&ms.to_be_bytes());
        }
    }
    hasher.update(&count.to_be_bytes());
    hasher.update(ancestor_hash.as_bytes());
    hasher.finalize()
}

/// Compute the canonical BLAKE3 hash of a tag from its fields.
///
/// # Canonical format
///
/// ```text
/// [16 B: UUID] [8 B: title len u64 BE] [N B: title UTF-8]
/// [8 B: created_at ms i64 BE] [8 B: modified_at ms i64 BE]
/// [1 B: has_color u8] [3 B: RGB (only if has_color == 1)]
/// [8 B: count i64 BE] [32 B: ancestor_hash]
/// ```
pub fn canonical_tag_hash(
    id: &Uuid,
    title: &str,
    created_at_ms: i64,
    modified_at_ms: i64,
    count: i64,
    ancestor_hash: &blake3::Hash,
    color: Option<&RGB8>,
) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(id.as_bytes());
    hasher.update(&(title.len() as u64).to_be_bytes());
    hasher.update(title.as_bytes());
    hasher.update(&created_at_ms.to_be_bytes());
    hasher.update(&modified_at_ms.to_be_bytes());
    match color {
        None => {
            hasher.update(&[0u8]);
        }
        Some(rgb) => {
            hasher.update(&[1u8]);
            hasher.update(&[rgb.r, rgb.g, rgb.b]);
        }
    }
    hasher.update(&count.to_be_bytes());
    hasher.update(ancestor_hash.as_bytes());
    hasher.finalize()
}

/// A versioned entity with a verifiable BLAKE3 hash chain.
///
/// All UUID-identified entities that carry linear version history
/// (cards, tags, etc.) implement this trait. Each version references
/// its direct ancestor by hash, forming a chain similar to git commits.
pub trait Entity {
    fn id(&self) -> Uuid;
    fn created_at(&self) -> DateTime<Utc>;
    fn modified_at(&self) -> DateTime<Utc>;
    fn count(&self) -> NonNegativeI64;
    fn ancestor_hash(&self) -> blake3::Hash;
    fn hash(&self) -> blake3::Hash;

    /// Compute the expected hash from all fields (excluding `hash` itself).
    fn expected_hash(&self) -> blake3::Hash;

    /// Verify that the stored hash matches the expected hash.
    fn verify(&self) -> bool {
        self.hash() == self.expected_hash()
    }
}

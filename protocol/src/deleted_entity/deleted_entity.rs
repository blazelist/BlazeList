use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A deleted entity — replaces any UUID-identified entity (card, tag, etc.)
/// after deletion. Only the UUID and its hash are retained; everything else
/// is purged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedEntity {
    id: Uuid,
    /// BLAKE3 hash of just the UUID bytes.
    hash: blake3::Hash,
}

impl DeletedEntity {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn hash(&self) -> blake3::Hash {
        self.hash
    }

    /// Create a `DeletedEntity` from a UUID. The hash is BLAKE3(uuid bytes).
    pub fn new(id: Uuid) -> Self {
        let hash = blake3::hash(id.as_bytes());
        Self { id, hash }
    }

    /// Reconstruct a `DeletedEntity` from stored parts (trusted storage).
    /// The caller is responsible for ensuring the hash is correct; use
    /// [`verify`](Self::verify) if integrity checking is needed.
    pub fn from_parts(id: Uuid, hash: blake3::Hash) -> Self {
        Self { id, hash }
    }

    /// Verify that the hash matches BLAKE3(uuid bytes).
    pub fn verify(&self) -> bool {
        self.hash == blake3::hash(self.id.as_bytes())
    }

    /// Test helper: tamper the id field for verification tests.
    #[cfg(test)]
    pub(crate) fn tamper_id(&mut self, id: Uuid) {
        self.id = id;
    }
}

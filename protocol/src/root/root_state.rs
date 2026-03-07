use serde::{Deserialize, Serialize};

use crate::NonNegativeI64;

/// The root state of the Blaze List.
///
/// The root hash is a BLAKE3 hash composed from all current card hashes.
/// The root sequence increments on any change to any card (creation, edit,
/// or deletion). Used for quick sync checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootState {
    pub hash: blake3::Hash,
    pub sequence: NonNegativeI64,
}

impl RootState {
    /// Initial root state — zero hash with sequence 0 (empty list).
    pub fn empty() -> Self {
        Self {
            hash: crate::ZERO_HASH,
            sequence: NonNegativeI64::MIN,
        }
    }
}

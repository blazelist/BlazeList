use serde::{Deserialize, Serialize};

/// Filter for listing cards.
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardFilter {
    /// All non-deleted cards.
    All,
    /// Only blazed (archived/done) cards.
    Blazed,
    /// Only extinguished (active/todo) cards.
    Extinguished,
}

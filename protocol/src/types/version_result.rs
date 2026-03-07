use crate::Version;
use serde::{Deserialize, Serialize};

/// Sent by the server in reply to a [`VersionCheck`](super::VersionCheck).
///
/// # Wire Stability
/// Postcard encodes variants by position. Do NOT reorder or insert
/// before existing variants — append only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionResult {
    /// Versions are compatible — proceed with requests.
    Ok,
    /// Major versions differ — connection will be closed.
    Mismatch { server_version: Version },
}

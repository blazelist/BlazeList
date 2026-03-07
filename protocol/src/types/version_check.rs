use crate::Version;
use serde::{Deserialize, Serialize};

/// Sent by the client on the first bidirectional stream of each connection.
/// The version should be [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION) so
/// that compatibility is always checked against the protocol crate's version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCheck {
    pub version: Version,
}

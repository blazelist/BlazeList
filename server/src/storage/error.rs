//! Storage error types.

pub use blazelist_protocol::PushError;

/// Server-side push operation error.
///
/// Wraps either a typed domain [`PushError`] (sent to the client) or an
/// internal storage error (logged server-side; client receives a generic
/// `Response::Error`).
#[derive(Debug)]
pub enum PushOpError {
    /// A domain-level push error to be forwarded to the client.
    Domain(PushError),
    /// An internal storage error; not exposed to clients.
    Internal(String),
}

impl From<PushError> for PushOpError {
    fn from(e: PushError) -> Self {
        PushOpError::Domain(e)
    }
}

impl From<rusqlite::Error> for PushOpError {
    fn from(e: rusqlite::Error) -> Self {
        PushOpError::Internal(e.to_string())
    }
}

/// Error from a batch push — identifies the failing item index.
#[derive(Debug)]
pub struct BatchError {
    pub index: usize,
    pub error: PushOpError,
}

/// Generic storage error.
#[derive(Debug)]
pub enum StorageError {
    /// Entity not found.
    NotFound,
    /// Entity was already deleted.
    AlreadyDeleted,
    /// Root hash mismatch during sync — client state is corrupted.
    RootHashMismatch {
        sequence: blazelist_protocol::NonNegativeI64,
        expected_hash: blake3::Hash,
    },
    /// The on-disk schema was created by an incompatible major version.
    ///
    /// Set `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION=true`
    /// to allow (currently unimplemented) automatic migration.
    IncompatibleVersion {
        stored: blazelist_protocol::Version,
        current: blazelist_protocol::Version,
    },
    /// Automatic migration was allowed but is not yet implemented.
    MigrationNotImplemented {
        stored: blazelist_protocol::Version,
        current: blazelist_protocol::Version,
    },
    Internal(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::NotFound => write!(f, "entity not found"),
            StorageError::AlreadyDeleted => write!(f, "entity already deleted"),
            StorageError::RootHashMismatch {
                sequence,
                expected_hash,
            } => write!(
                f,
                "root hash mismatch at sequence {}: expected {}",
                sequence, expected_hash
            ),
            StorageError::IncompatibleVersion { stored, current } => write!(
                f,
                "incompatible database: created by protocol v{stored}, \
                 current protocol is v{current} (different major version). \
                 Set BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION=true \
                 to allow automatic migration (destructive, irreversible)"
            ),
            StorageError::MigrationNotImplemented { stored, current } => write!(
                f,
                "automatic migration from v{stored} to v{current} is not yet implemented"
            ),
            StorageError::Internal(msg) => write!(f, "storage error: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::Internal(e.to_string())
    }
}

impl From<StorageError> for PushOpError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::AlreadyDeleted => PushOpError::Domain(PushError::AlreadyDeleted),
            other => PushOpError::Internal(other.to_string()),
        }
    }
}

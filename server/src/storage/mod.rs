//! Storage trait and SQLite implementation.

pub mod error;
pub mod sqlite;
pub mod traits;

#[cfg(test)]
mod tests;

pub use error::{BatchError, PushError, PushOpError, StorageError};
pub use sqlite::SqliteStorage;
pub use traits::Storage;

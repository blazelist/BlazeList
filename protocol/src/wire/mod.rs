//! Length-prefixed postcard wire helpers.
//!
//! Provides generic `read_message` and `write_message` functions that work
//! with any `tokio::io::AsyncRead` / `AsyncWrite` stream. Both the server
//! and all clients should use these instead of rolling their own framing.

mod constants;
mod error;
mod wire;

#[cfg(test)]
mod tests;

pub use constants::MAX_MSG_SIZE;
pub use error::WireError;
pub use wire::{read_message, write_message};

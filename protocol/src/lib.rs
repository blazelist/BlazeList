//! Wire protocol and shared types for client-server communication.
//!
//! All messages are serialized with postcard. On the wire, each message
//! is length-prefixed: a 4-byte big-endian u32 length followed by the
//! postcard payload.
//!
//! The [`wire`] module provides generic `read_message` / `write_message`
//! helpers that work with any `tokio::io::AsyncRead` / `AsyncWrite` stream.
//!
//! The [`handshake`] module provides shared client/server version
//! negotiation helpers so that new clients don't need to reimplement
//! the handshake logic.

// -- Type modules (formerly blazelist-types) --
mod card;
mod deleted_entity;
mod hash;
mod non_negative_i64;
mod priority;
mod root;
mod tag;
mod version;

pub use card::Card;
pub use chrono::{DateTime, Utc};
pub use deleted_entity::DeletedEntity;
pub use hash::{Entity, HashVerificationError, ZERO_HASH, canonical_card_hash, canonical_tag_hash};
pub use non_negative_i64::{NegativeValueError, NonNegativeI64, OutOfRangeError};
pub use priority::{compute_priority, priority_percentage};
pub use root::RootState;
pub use tag::Tag;
pub use version::{PROTOCOL_VERSION, Version, is_compatible};

// -- Protocol modules (wire format, handshake, request/response types) --
#[cfg(feature = "io")]
pub mod handshake;
mod types;
#[cfg(feature = "io")]
pub mod wire;

#[cfg(test)]
mod tests;

pub use types::*;

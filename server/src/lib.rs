//! BlazeList server — SQLite storage, QUIC + WebTransport communication.
//!
//! The public API is the [`Storage`] trait and its [`SqliteStorage`]
//! implementation. The QUIC layer is in the [`quic`] module, the
//! WebTransport layer is in the [`webtransport`] module, and the
//! [`protocol`](blazelist_protocol) crate defines the wire format.
//!
//! The [`handler`] module contains the transport-agnostic request
//! dispatcher shared by both transport layers.

pub mod handler;
pub mod https;
pub mod quic;
pub mod storage;
pub mod webtransport;

pub use storage::{SqliteStorage, Storage};

/// The protocol version this server speaks — shared across transport layers.
pub const SERVER_VERSION: blazelist_protocol::Version = blazelist_protocol::PROTOCOL_VERSION;

//! Version handshake helpers for client-server connections.
//!
//! Both the client and server sides of the handshake are provided so that
//! new clients don't need to reimplement the protocol negotiation logic.

mod error;
mod handshake;

#[cfg(test)]
mod tests;

pub use error::HandshakeError;
pub use handshake::{client_handshake, server_handshake};

//! QUIC server and client helpers.
//!
//! The QUIC layer uses `quinn` with self-signed certificates for
//! development. Each bidirectional stream carries one request/response
//! exchange, length-prefixed with a 4-byte big-endian u32.

pub mod server;
pub mod tls;

#[cfg(test)]
mod tests;

pub use crate::handler::handle_request;
pub use blazelist_protocol::wire::{WireError, read_message, write_message};
pub use server::{perform_version_handshake, run_server, send_request};
pub use tls::{SelfSignedCert, client_config_for_cert, self_signed_server_config};

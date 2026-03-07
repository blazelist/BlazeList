//! WebTransport server layer.
//!
//! Mirrors the QUIC server layer but uses `wtransport` for browser-compatible
//! WebTransport (HTTP/3) connections. Shares the same request handler and
//! storage backend as the QUIC layer.

pub mod server;

pub use server::{WtServerConfig, run_webtransport_server, webtransport_server_config};

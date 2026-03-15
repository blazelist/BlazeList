//! Shared client library for BlazeList clients.
//!
//! This crate provides platform-agnostic logic shared between the native
//! CLI client and the WASM web client. It includes:
//!
//! - **Display utilities** — Markdown-to-plain-text rendering and card preview
//!   generation ([`display`]).
//! - **Filtering** — Card filtering by blaze status, search query, and tags
//!   ([`filter`]).
//! - **Sync helpers** — Incremental changeset application ([`sync`]).
//! - **Color utilities** — Tag color formatting and styling ([`color`]).
//! - **Due date utilities** — Status computation and presets ([`due_date`]).

/// Crate version, derived from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod client;
pub mod color;
pub mod display;
pub mod due_date;
pub mod error;
pub mod filter;
pub mod priority;
pub mod sync;

#[cfg(test)]
pub(crate) mod test_helpers {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    pub fn fixed_time() -> DateTime<Utc> {
        DateTime::from_timestamp_millis(1_700_000_000_000).unwrap()
    }

    pub fn priority(v: i64) -> i64 {
        v
    }

    pub fn fixed_uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    }
}

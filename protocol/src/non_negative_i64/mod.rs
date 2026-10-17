//! # NonNegativeI64
//!
//! Newtype around `i64` that rejects negative values at construction and
//! deserialization. Used for priority, count, and sequence fields.
//!
//! Signed rather than `u64` or `u128` because SQLite `INTEGER` and PostgreSQL
//! `BIGINT` are both signed 64-bit: `i64` crosses the storage boundary without
//! a conversion, and the `0..=i64::MAX` range is never exhausted in practice.

mod error;
mod non_negative_i64;

#[cfg(test)]
mod tests;

pub use error::{NegativeValueError, OutOfRangeError};
pub use non_negative_i64::NonNegativeI64;

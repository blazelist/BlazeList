//! # NonNegativeI64
//!
//! Newtype around `i64` that rejects negative values at construction and
//! deserialization. Used for priority, count, and sequence fields.
//! See SPEC.md for the design rationale (why `i64` over `u64` or `u128`).

mod error;
mod non_negative_i64;

#[cfg(test)]
mod tests;

pub use error::{NegativeValueError, OutOfRangeError};
pub use non_negative_i64::NonNegativeI64;

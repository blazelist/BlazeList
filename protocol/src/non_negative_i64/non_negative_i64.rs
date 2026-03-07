use serde::{Deserialize, Serialize};

use super::error::{NegativeValueError, OutOfRangeError};

/// A non-negative `i64` value.
///
/// Wraps an `i64` to guarantee the value is always >= 0. Using `i64`
/// (rather than `u64`) ensures direct compatibility with SQLite `INTEGER`
/// and PostgreSQL `BIGINT`, both of which are signed 64-bit.
///
/// The usable range is 0 to `i64::MAX`. This is half the range of `u64`
/// since only positive values are used, but still far beyond any
/// realistic usage.
///
/// Conversions rely on standard traits:
/// - `u64::from(value)` to get the value as `u64` (lossless, since the
///   inner value is guaranteed non-negative)
/// - `i64::from(value)` when crossing the storage boundary
/// - `NonNegativeI64::try_from(v: u64)` to construct from `u64` (rejects
///   values above `i64::MAX`)
/// - `NonNegativeI64::try_from(v: i64)` to construct from `i64` (rejects
///   negative values)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct NonNegativeI64(i64);

impl NonNegativeI64 {
    /// The maximum value (`i64::MAX`).
    pub const MAX: Self = Self(i64::MAX);

    /// The minimum value (`0`).
    pub const MIN: Self = Self(0);

    /// Returns this value as a percentage of the `NonNegativeI64` range.
    ///
    /// Maps `MIN` (0) to `0.0` and `MAX` (`i64::MAX`) to `100.0`.
    pub fn percentage(self) -> f64 {
        (self.0 as f64 / i64::MAX as f64) * 100.0
    }
}

impl std::fmt::Display for NonNegativeI64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<i64> for NonNegativeI64 {
    type Error = NegativeValueError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value >= 0 {
            Ok(Self(value))
        } else {
            Err(NegativeValueError)
        }
    }
}

impl From<NonNegativeI64> for i64 {
    fn from(v: NonNegativeI64) -> Self {
        v.0
    }
}

impl From<NonNegativeI64> for u64 {
    fn from(v: NonNegativeI64) -> Self {
        v.0 as u64
    }
}

impl TryFrom<u64> for NonNegativeI64 {
    type Error = OutOfRangeError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value <= i64::MAX as u64 {
            Ok(Self(value as i64))
        } else {
            Err(OutOfRangeError)
        }
    }
}

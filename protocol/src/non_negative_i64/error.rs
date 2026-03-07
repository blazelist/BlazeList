/// Error returned when attempting to create a `NonNegativeI64` from a negative value.
#[derive(Debug, Clone)]
pub struct NegativeValueError;

impl std::fmt::Display for NegativeValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "value must be non-negative")
    }
}

impl std::error::Error for NegativeValueError {}

/// Error returned when a `u64` value exceeds `i64::MAX`.
#[derive(Debug, Clone)]
pub struct OutOfRangeError;

impl std::fmt::Display for OutOfRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "value exceeds i64::MAX")
    }
}

impl std::error::Error for OutOfRangeError {}

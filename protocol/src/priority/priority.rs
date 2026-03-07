//! Priority computation for card placement.
//!
//! Implements the midpoint + jitter algorithm described in SPEC.md: a placed
//! card takes the midpoint (floored) between its upper and lower neighbors,
//! plus a small random jitter to avoid collisions when two clients independently
//! place cards into the same gap.

use rand::RngExt;

use crate::NonNegativeI64;

/// Compute a priority between `upper` and `lower` using midpoint + jitter.
///
/// Uses the midpoint + jitter algorithm from SPEC.md: midpoint (floored)
/// between the two bounds, plus a small random jitter to avoid collisions.
pub fn compute_priority(upper: NonNegativeI64, lower: NonNegativeI64) -> NonNegativeI64 {
    let upper_val = i64::from(upper);
    let lower_val = i64::from(lower);
    // Overflow-safe midpoint: low + (high - low) / 2.
    // Both values are non-negative, so the subtraction cannot underflow.
    let gap = upper_val - lower_val;
    let midpoint = lower_val + gap / 2;

    if gap <= 2 {
        // No room for jitter — just return the midpoint.
        return NonNegativeI64::try_from(midpoint).unwrap();
    }

    let jitter_range = (gap / 16).max(1);
    let mut rng = rand::rng();
    let jitter_val = rng.random_range(0..jitter_range * 2 + 1) - jitter_range;
    let result = (midpoint + jitter_val).clamp(lower_val + 1, upper_val - 1);

    NonNegativeI64::try_from(result).unwrap()
}

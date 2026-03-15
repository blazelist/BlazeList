//! Priority computation for card placement.
//!
//! Implements the midpoint + jitter algorithm described in SPEC.md: a placed
//! card takes the midpoint (floored) between its upper and lower neighbors,
//! plus a small random jitter to avoid collisions when two clients independently
//! place cards into the same gap.

use rand::RngExt;

/// Compute a priority between `upper` and `lower` using midpoint + jitter.
///
/// Uses the midpoint + jitter algorithm from SPEC.md: midpoint (floored)
/// between the two bounds, plus a small random jitter to avoid collisions.
pub fn compute_priority(upper: i64, lower: i64) -> i64 {
    let upper_val = upper as i128;
    let lower_val = lower as i128;
    // Overflow-safe midpoint using i128 arithmetic.
    let gap = upper_val - lower_val;
    let midpoint = lower_val + gap / 2;

    if gap <= 2 {
        // No room for jitter — just return the midpoint.
        return midpoint as i64;
    }

    let jitter_range = (gap / 16).max(1);
    let mut rng = rand::rng();
    let jitter_val = rng.random_range(0..jitter_range * 2 + 1) - jitter_range;
    let result = (midpoint + jitter_val).clamp(lower_val + 1, upper_val - 1);

    result as i64
}

/// Returns a priority value as a percentage of the full `i64` range.
///
/// Maps `i64::MIN` to `0.0` and `i64::MAX` to `100.0`.
pub fn priority_percentage(priority: i64) -> f64 {
    ((priority as f64 - i64::MIN as f64) / (i64::MAX as f64 - i64::MIN as f64)) * 100.0
}

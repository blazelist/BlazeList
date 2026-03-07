use super::compute_priority;
use crate::NonNegativeI64;

fn p(v: i64) -> NonNegativeI64 {
    NonNegativeI64::try_from(v).unwrap()
}

#[test]
fn compute_priority_midpoint_between_extremes() {
    // Between MAX and MIN, result should be roughly in the middle.
    let result = compute_priority(NonNegativeI64::MAX, NonNegativeI64::MIN);
    let val = i64::from(result);
    // With jitter, it should be near the midpoint of i64::MAX / 2.
    assert!(val > 0);
    assert!(val < i64::MAX);
}

#[test]
fn compute_priority_always_between_bounds() {
    for _ in 0..100 {
        let result = compute_priority(p(1000), p(100));
        let val = i64::from(result);
        assert!(val > 100, "expected > 100, got {val}");
        assert!(val < 1000, "expected < 1000, got {val}");
    }
}

#[test]
fn compute_priority_narrow_gap() {
    // Gap of 2: should return midpoint.
    let result = compute_priority(p(101), p(99));
    assert_eq!(i64::from(result), 100);
}

#[test]
fn compute_priority_gap_of_one() {
    // Gap of 1: midpoint floored.
    let result = compute_priority(p(101), p(100));
    assert_eq!(i64::from(result), 100);
}

#[test]
fn compute_priority_same_values() {
    let result = compute_priority(p(50), p(50));
    assert_eq!(i64::from(result), 50);
}

#[test]
fn compute_priority_at_zero_lower() {
    let result = compute_priority(p(100), NonNegativeI64::MIN);
    let val = i64::from(result);
    assert!(val > 0);
    assert!(val < 100);
}

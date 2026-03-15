use super::{compute_priority, priority_percentage};

#[test]
fn compute_priority_midpoint_between_extremes() {
    // Between MAX and MIN, result should be roughly in the middle.
    let result = compute_priority(i64::MAX, i64::MIN);
    // With jitter, it should be near 0 (midpoint of the full range).
    assert!(result > i64::MIN);
    assert!(result < i64::MAX);
}

#[test]
fn compute_priority_always_between_bounds() {
    for _ in 0..100 {
        let result = compute_priority(1000, 100);
        assert!(result > 100, "expected > 100, got {result}");
        assert!(result < 1000, "expected < 1000, got {result}");
    }
}

#[test]
fn compute_priority_narrow_gap() {
    // Gap of 2: should return midpoint.
    let result = compute_priority(101, 99);
    assert_eq!(result, 100);
}

#[test]
fn compute_priority_gap_of_one() {
    // Gap of 1: midpoint floored.
    let result = compute_priority(101, 100);
    assert_eq!(result, 100);
}

#[test]
fn compute_priority_same_values() {
    let result = compute_priority(50, 50);
    assert_eq!(result, 50);
}

#[test]
fn compute_priority_at_min_lower() {
    let result = compute_priority(100, i64::MIN);
    assert!(result > i64::MIN);
    assert!(result < 100);
}

#[test]
fn compute_priority_negative_bounds() {
    for _ in 0..100 {
        let result = compute_priority(-100, -1000);
        assert!(result > -1000, "expected > -1000, got {result}");
        assert!(result < -100, "expected < -100, got {result}");
    }
}

#[test]
fn compute_priority_across_zero() {
    for _ in 0..100 {
        let result = compute_priority(1000, -1000);
        assert!(result > -1000, "expected > -1000, got {result}");
        assert!(result < 1000, "expected < 1000, got {result}");
    }
}

#[test]
fn priority_percentage_extremes() {
    assert_eq!(priority_percentage(i64::MIN), 0.0);
    assert_eq!(priority_percentage(i64::MAX), 100.0);
}

#[test]
fn priority_percentage_midrange() {
    // The midpoint of the i64 range is approximately -0.5 (since i64::MIN is
    // one farther from zero than i64::MAX). The percentage should be ~50%.
    let pct = priority_percentage(0);
    assert!(pct > 49.9 && pct < 50.1, "expected ~50%, got {pct}");
}

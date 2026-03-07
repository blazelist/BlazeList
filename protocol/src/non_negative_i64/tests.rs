use super::*;

#[test]
fn try_from_i64_rejects_negative() {
    assert!(NonNegativeI64::try_from(-1i64).is_err());
    assert!(NonNegativeI64::try_from(i64::MIN).is_err());
}

#[test]
fn try_from_i64_accepts_zero() {
    let v = NonNegativeI64::try_from(0i64).unwrap();
    assert_eq!(u64::from(v), 0u64);
}

#[test]
fn try_from_i64_accepts_positive() {
    let v = NonNegativeI64::try_from(42i64).unwrap();
    assert_eq!(u64::from(v), 42u64);
    let v = NonNegativeI64::try_from(i64::MAX).unwrap();
    assert_eq!(u64::from(v), i64::MAX as u64);
}

#[test]
fn try_from_u64_accepts_valid() {
    let v = NonNegativeI64::try_from(0u64).unwrap();
    assert_eq!(u64::from(v), 0u64);
    let v = NonNegativeI64::try_from(42u64).unwrap();
    assert_eq!(u64::from(v), 42u64);
    let v = NonNegativeI64::try_from(i64::MAX as u64).unwrap();
    assert_eq!(u64::from(v), i64::MAX as u64);
}

#[test]
fn try_from_u64_rejects_overflow() {
    assert!(NonNegativeI64::try_from(i64::MAX as u64 + 1).is_err());
    assert!(NonNegativeI64::try_from(u64::MAX).is_err());
}

#[test]
fn max_and_min_constants() {
    assert_eq!(u64::from(NonNegativeI64::MAX), i64::MAX as u64);
    assert_eq!(u64::from(NonNegativeI64::MIN), 0u64);
}

#[test]
fn percentage_at_boundaries() {
    assert_eq!(NonNegativeI64::MIN.percentage(), 0.0);
    assert_eq!(NonNegativeI64::MAX.percentage(), 100.0);
}

#[test]
fn percentage_at_midpoint() {
    let mid = NonNegativeI64::try_from(i64::MAX / 2).unwrap();
    let pct = mid.percentage();
    assert!((pct - 50.0).abs() < 0.01, "expected ~50.0, got {pct}");
}

#[test]
fn ordering_works() {
    let a = NonNegativeI64::try_from(10i64).unwrap();
    let b = NonNegativeI64::try_from(20i64).unwrap();
    assert!(a < b);
    assert!(b > a);
    assert_eq!(a, NonNegativeI64::try_from(10i64).unwrap());
}

#[test]
fn display_shows_inner_value() {
    let v = NonNegativeI64::try_from(42i64).unwrap();
    assert_eq!(v.to_string(), "42");
    assert_eq!(NonNegativeI64::MIN.to_string(), "0");
}

#[test]
fn serde_round_trip() {
    let v = NonNegativeI64::try_from(12345i64).unwrap();
    let bytes = postcard::to_allocvec(&v).unwrap();
    let v2: NonNegativeI64 = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn serde_rejects_negative() {
    let bytes = postcard::to_allocvec(&(-1i64)).unwrap();
    let result: Result<NonNegativeI64, _> = postcard::from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn into_i64_conversion() {
    let v = NonNegativeI64::try_from(42i64).unwrap();
    let raw: i64 = v.into();
    assert_eq!(raw, 42i64);
}

#[test]
fn into_u64_conversion() {
    let v = NonNegativeI64::try_from(42i64).unwrap();
    let raw: u64 = v.into();
    assert_eq!(raw, 42u64);
}

// -- Boundary tests -------------------------------------------------------

#[test]
fn try_from_i64_boundary_exactly_zero() {
    let v = NonNegativeI64::try_from(0i64).unwrap();
    assert_eq!(u64::from(v), 0u64);
    assert_eq!(i64::from(v), 0i64);
}

#[test]
fn try_from_i64_boundary_exactly_one() {
    let v = NonNegativeI64::try_from(1i64).unwrap();
    assert_eq!(u64::from(v), 1u64);
    assert_eq!(i64::from(v), 1i64);
}

#[test]
fn try_from_i64_boundary_minus_one_rejected() {
    assert!(NonNegativeI64::try_from(-1i64).is_err());
}

#[test]
fn try_from_i64_boundary_i64_max() {
    let v = NonNegativeI64::try_from(i64::MAX).unwrap();
    assert_eq!(u64::from(v), i64::MAX as u64);
    assert_eq!(i64::from(v), i64::MAX);
}

#[test]
fn try_from_i64_boundary_i64_min_rejected() {
    assert!(NonNegativeI64::try_from(i64::MIN).is_err());
}

#[test]
fn try_from_u64_boundary_exactly_zero() {
    let v = NonNegativeI64::try_from(0u64).unwrap();
    assert_eq!(u64::from(v), 0u64);
    assert_eq!(i64::from(v), 0i64);
}

#[test]
fn try_from_u64_boundary_exactly_one() {
    let v = NonNegativeI64::try_from(1u64).unwrap();
    assert_eq!(u64::from(v), 1u64);
}

#[test]
fn try_from_u64_boundary_i64_max() {
    let v = NonNegativeI64::try_from(i64::MAX as u64).unwrap();
    assert_eq!(u64::from(v), i64::MAX as u64);
    assert_eq!(i64::from(v), i64::MAX);
}

#[test]
fn try_from_u64_boundary_i64_max_plus_one_rejected() {
    assert!(NonNegativeI64::try_from(i64::MAX as u64 + 1).is_err());
}

#[test]
fn try_from_u64_boundary_u64_max_rejected() {
    assert!(NonNegativeI64::try_from(u64::MAX).is_err());
}

#[test]
fn serde_round_trip_boundary_zero() {
    let v = NonNegativeI64::try_from(0i64).unwrap();
    let bytes = postcard::to_allocvec(&v).unwrap();
    let v2: NonNegativeI64 = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(v, v2);
    assert_eq!(u64::from(v2), 0u64);
}

#[test]
fn serde_round_trip_boundary_i64_max() {
    let v = NonNegativeI64::try_from(i64::MAX).unwrap();
    let bytes = postcard::to_allocvec(&v).unwrap();
    let v2: NonNegativeI64 = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(v, v2);
    assert_eq!(u64::from(v2), i64::MAX as u64);
}

#[test]
fn serde_rejects_i64_min() {
    let bytes = postcard::to_allocvec(&i64::MIN).unwrap();
    let result: Result<NonNegativeI64, _> = postcard::from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn serde_rejects_minus_one() {
    let bytes = postcard::to_allocvec(&(-1i64)).unwrap();
    let result: Result<NonNegativeI64, _> = postcard::from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn into_u64_at_boundaries() {
    assert_eq!(u64::from(NonNegativeI64::MIN), 0u64);
    assert_eq!(u64::from(NonNegativeI64::MAX), i64::MAX as u64);
    assert_eq!(u64::from(NonNegativeI64::try_from(0i64).unwrap()), 0u64);
    assert_eq!(
        u64::from(NonNegativeI64::try_from(i64::MAX).unwrap()),
        i64::MAX as u64
    );
}

#[test]
fn into_i64_at_boundaries() {
    assert_eq!(i64::from(NonNegativeI64::MIN), 0i64);
    assert_eq!(i64::from(NonNegativeI64::MAX), i64::MAX);
}

use super::{Version, is_compatible};

#[test]
fn pkg_version_macro() {
    // Workspace version is "2.1.0".
    let v: Version = crate::pkg_version!();
    assert_eq!(v, Version::new(2, 1, 0));
}

#[test]
fn const_construction() {
    const V: Version = Version::new(1, 2, 3);
    assert_eq!(V.major, 1);
    assert_eq!(V.minor, 2);
    assert_eq!(V.patch, 3);
}

#[test]
fn display_formatting() {
    assert_eq!(Version::new(0, 0, 0).to_string(), "0.0.0");
    assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
    assert_eq!(Version::new(10, 20, 300).to_string(), "10.20.300");
}

#[test]
fn compatible_same_major() {
    // Post-1.0: same major is compatible.
    assert!(is_compatible(
        &Version::new(2, 0, 0),
        &Version::new(2, 5, 3),
    ));
}

#[test]
fn compatible_same_major_zero() {
    // Major-version matching only — no special pre-1.0 handling because
    // all crates start at 1.0.0.
    assert!(is_compatible(
        &Version::new(0, 1, 0),
        &Version::new(0, 2, 0),
    ));
}

#[test]
fn compatible_different_minor() {
    // Post-1.0: different minor versions with same major are compatible.
    assert!(is_compatible(
        &Version::new(1, 1, 0),
        &Version::new(1, 2, 0),
    ));
}

#[test]
fn incompatible_different_major() {
    assert!(!is_compatible(
        &Version::new(0, 0, 0),
        &Version::new(1, 0, 0),
    ));
}

#[test]
fn compatible_reflexive() {
    let v = Version::new(3, 1, 4);
    assert!(is_compatible(&v, &v));
}

#[test]
fn equality() {
    assert_eq!(Version::new(1, 2, 3), Version::new(1, 2, 3));
    assert_ne!(Version::new(1, 2, 3), Version::new(1, 2, 4));
    assert_ne!(Version::new(1, 2, 3), Version::new(1, 3, 3));
    assert_ne!(Version::new(1, 2, 3), Version::new(2, 2, 3));
}

#[test]
fn serde_round_trip() {
    let v = Version::new(1, 2, 3);
    let bytes = postcard::to_allocvec(&v).unwrap();
    let decoded: Version = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn parse_from_string() {
    let v: Version = "1.2.3".parse().unwrap();
    assert_eq!(v, Version::new(1, 2, 3));
}

#[test]
fn prerelease_support() {
    let v: Version = "1.0.0-alpha.1".parse().unwrap();
    assert_eq!(v.major, 1);
    assert!(!v.pre.is_empty());
}

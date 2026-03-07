//! Version utilities for BlazeList protocol negotiation.
//!
//! Re-exports [`semver::Version`] and provides the [`pkg_version!`] macro
//! for compile-time version extraction from Cargo metadata.

pub use semver::Version;

/// Expands to a [`semver::Version`] constant holding the calling crate's
/// version, parsed from `CARGO_PKG_VERSION_MAJOR`, `CARGO_PKG_VERSION_MINOR`,
/// and `CARGO_PKG_VERSION_PATCH` at compile time.
///
/// Because the macro is expanded at the call site, each crate gets its own
/// version even if this macro lives in `blazelist-protocol`.
#[macro_export]
macro_rules! pkg_version {
    () => {{
        const fn parse_u64(bytes: &[u8]) -> u64 {
            let mut result: u64 = 0;
            let mut i = 0;
            while i < bytes.len() {
                let digit = bytes[i];
                assert!(
                    digit >= b'0' && digit <= b'9',
                    "non-digit in version component"
                );
                result = result * 10 + (digit - b'0') as u64;
                i += 1;
            }
            result
        }
        const VERSION: $crate::Version = $crate::Version::new(
            parse_u64(env!("CARGO_PKG_VERSION_MAJOR").as_bytes()),
            parse_u64(env!("CARGO_PKG_VERSION_MINOR").as_bytes()),
            parse_u64(env!("CARGO_PKG_VERSION_PATCH").as_bytes()),
        );
        VERSION
    }};
}

/// The protocol crate's own version, derived at compile time from
/// `blazelist-protocol`'s `Cargo.toml`.
///
/// Both client and server should use this constant for the version
/// handshake so that compatibility is always checked against the
/// **protocol** version, not the application crate's version.
pub const PROTOCOL_VERSION: Version = crate::pkg_version!();

/// Returns `true` if two versions are compatible.
///
/// Compatibility requires the same major version (standard semver rules).
pub const fn is_compatible(a: &Version, b: &Version) -> bool {
    a.major == b.major
}

#[cfg(test)]
mod tests;
mod version;

pub use version::{PROTOCOL_VERSION, PROTOCOL_VERSION_STR, Version, is_compatible};

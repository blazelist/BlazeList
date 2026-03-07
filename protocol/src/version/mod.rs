#[cfg(test)]
mod tests;
mod version;

pub use version::{PROTOCOL_VERSION, Version, is_compatible};

/// Error returned when a hash does not match the expected value.
#[derive(Debug, Clone)]
pub struct HashVerificationError;

impl std::fmt::Display for HashVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hash verification failed: stored hash does not match computed hash"
        )
    }
}

impl std::error::Error for HashVerificationError {}

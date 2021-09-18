use sha2::{Digest, Sha256};
use std::fmt;

/// A checksum of some data.
#[derive(Debug, Clone)]
pub enum Checksum {
    Sha256(String),
}

impl Checksum {
    pub fn sha256(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        Self::Sha256(format!("{:x}", hash))
    }

    pub fn sha256_from_str_unchecked(hash: &str) -> Self {
        Self::Sha256(hash.to_string())
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash = match self {
            Self::Sha256(hash) => hash,
        };
        write!(f, "{}", hash)
    }
}

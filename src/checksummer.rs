//! Compute checksums of data.
//!
//! De-duplication of backed up data in Obnam relies on cryptographic
//! checksums. They are implemented in this module. Note that Obnam
//! does not aim to make these algorithms configurable, so only a very
//! small number of carefully chosen algorithms are supported here.

use sha2::{Digest, Sha256};
use std::fmt;

/// A checksum of some data.
#[derive(Debug, Clone)]
pub enum Checksum {
    /// An arbitrary, literal string.
    Literal(String),

    /// A SHA256 checksum.
    Sha256(String),
}

impl Checksum {
    /// Construct a literal string.
    pub fn literal(s: &str) -> Self {
        Self::Literal(s.to_string())
    }

    /// Compute a SHA256 checksum for a block of data.
    pub fn sha256(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        Self::Sha256(format!("{:x}", hash))
    }

    /// Create a `Checksum` from a known, previously computed hash.
    pub fn sha256_from_str_unchecked(hash: &str) -> Self {
        Self::Sha256(hash.to_string())
    }
}

impl fmt::Display for Checksum {
    /// Format a checksum for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{}", s),
            Self::Sha256(hash) => write!(f, "{}", hash),
        }
    }
}

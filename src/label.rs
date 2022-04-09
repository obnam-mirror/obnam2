//! A chunk label.
//!
//! De-duplication of backed up data in Obnam relies on cryptographic
//! checksums. They are implemented in this module. Note that Obnam
//! does not aim to make these algorithms configurable, so only a very
//! small number of carefully chosen algorithms are supported here.

use sha2::{Digest, Sha256};

const LITERAL: char = '0';
const SHA256: char = '1';

/// A checksum of some data.
#[derive(Debug, Clone)]
pub enum Label {
    /// An arbitrary, literal string.
    Literal(String),

    /// A SHA256 checksum.
    Sha256(String),
}

impl Label {
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

    /// Serialize a label into a string representation.
    pub fn serialize(&self) -> String {
        match self {
            Self::Literal(s) => format!("{}{}", LITERAL, s),
            Self::Sha256(hash) => format!("{}{}", SHA256, hash),
        }
    }

    /// De-serialize a label from its string representation.
    pub fn deserialize(s: &str) -> Result<Self, LabelError> {
        if s.starts_with(LITERAL) {
            Ok(Self::Literal(s[1..].to_string()))
        } else if s.starts_with(SHA256) {
            Ok(Self::Sha256(s[1..].to_string()))
        } else {
            Err(LabelError::UnknownType(s.to_string()))
        }
    }
}

/// Possible errors from dealing with chunk labels.
#[derive(Debug, thiserror::Error)]
pub enum LabelError {
    /// Serialized label didn't start with a known type prefix.
    #[error("Unknown label: {0:?}")]
    UnknownType(String),
}

#[cfg(test)]
mod test {
    use super::Label;

    #[test]
    fn roundtrip_literal() {
        let label = Label::literal("dummy data");
        let serialized = label.serialize();
        let de = Label::deserialize(&serialized).unwrap();
        let seri2 = de.serialize();
        assert_eq!(serialized, seri2);
    }

    #[test]
    fn roundtrip_sha256() {
        let label = Label::sha256(b"dummy data");
        let serialized = label.serialize();
        let de = Label::deserialize(&serialized).unwrap();
        let seri2 = de.serialize();
        assert_eq!(serialized, seri2);
    }
}

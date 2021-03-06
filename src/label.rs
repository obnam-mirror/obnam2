//! A chunk label.
//!
//! De-duplication of backed up data in Obnam relies on cryptographic
//! checksums. They are implemented in this module. Note that Obnam
//! does not aim to make these algorithms configurable, so only a very
//! small number of carefully chosen algorithms are supported here.

use blake2::Blake2s256;
use sha2::{Digest, Sha256};

const LITERAL: char = '0';
const SHA256: char = '1';
const BLAKE2: char = '2';

/// A checksum of some data.
#[derive(Debug, Clone)]
pub enum Label {
    /// An arbitrary, literal string.
    Literal(String),

    /// A SHA256 checksum.
    Sha256(String),

    /// A BLAKE2s checksum.
    Blake2(String),
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

    /// Compute a BLAKE2s checksum for a block of data.
    pub fn blake2(data: &[u8]) -> Self {
        let mut hasher = Blake2s256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        Self::Sha256(format!("{:x}", hash))
    }

    /// Serialize a label into a string representation.
    pub fn serialize(&self) -> String {
        match self {
            Self::Literal(s) => format!("{}{}", LITERAL, s),
            Self::Sha256(hash) => format!("{}{}", SHA256, hash),
            Self::Blake2(hash) => format!("{}{}", BLAKE2, hash),
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

/// Kinds of checksum labels.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LabelChecksumKind {
    /// Use a Blake2 checksum.
    Blake2,

    /// Use a SHA256 checksum.
    Sha256,
}

impl LabelChecksumKind {
    /// Parse a string into a label checksum kind.
    pub fn from(s: &str) -> Result<Self, LabelError> {
        if s == "sha256" {
            Ok(Self::Sha256)
        } else if s == "blake2" {
            Ok(Self::Blake2)
        } else {
            Err(LabelError::UnknownType(s.to_string()))
        }
    }

    /// Serialize a checksum kind into a string.
    pub fn serialize(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Blake2 => "blake2",
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
    use super::{Label, LabelChecksumKind};

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

    #[test]
    fn roundtrip_checksum_kind() {
        for kind in [LabelChecksumKind::Sha256, LabelChecksumKind::Blake2] {
            assert_eq!(LabelChecksumKind::from(kind.serialize()).unwrap(), kind);
        }
    }
}

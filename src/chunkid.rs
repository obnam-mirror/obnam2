use crate::checksummer::Checksum;
use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use uuid::Uuid;

/// An identifier for a chunk.
///
/// An identifier is chosen randomly in such a way that even in
/// extremely large numbers of identifiers the likelihood of duplicate
/// identifiers is so small it can be ignored. The current
/// implementation uses UUID4 and provides a 122-bit random number.
/// For a discussion on collision likelihood, see
/// <https://en.wikipedia.org/wiki/Universally_unique_identifier#Collisions>.
///
/// We also need to be able to re-create identifiers from stored
/// values. When an identifier is formatted as a string and parsed
/// back, the result is the same value.
///
/// Because every identifier is meant to be different, there is no
/// default value, since default values should be identical.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub struct ChunkId {
    id: String,
}

#[allow(clippy::new_without_default)]
impl ChunkId {
    /// Construct a new, random identifier.
    pub fn new() -> Self {
        ChunkId {
            id: Uuid::new_v4().to_string(),
        }
    }

    /// Re-construct an identifier from a previous values.
    pub fn recreate(s: &str) -> Self {
        ChunkId { id: s.to_string() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.id.as_bytes()
    }

    pub fn sha256(&self) -> Checksum {
        Checksum::sha256(self.id.as_bytes())
    }
}

impl ToSql for ChunkId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Text(
            self.id.clone(),
        )))
    }
}

impl fmt::Display for ChunkId {
    /// Format an identifier for display.
    ///
    /// The output can be parsed to re-created an identical identifier.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl From<&String> for ChunkId {
    fn from(s: &String) -> Self {
        ChunkId { id: s.to_string() }
    }
}

impl From<&OsStr> for ChunkId {
    fn from(s: &OsStr) -> Self {
        ChunkId {
            id: s.to_string_lossy().to_string(),
        }
    }
}

impl FromStr for ChunkId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ChunkId::recreate(s))
    }
}

#[cfg(test)]
mod test {
    use super::ChunkId;

    #[test]
    fn to_string() {
        let id = ChunkId::new();
        assert_ne!(id.to_string(), "")
    }

    #[test]
    fn never_the_same() {
        let id1 = ChunkId::new();
        let id2 = ChunkId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn recreatable() {
        let id_str = "xyzzy"; // it doesn't matter what the string representation is
        let id: ChunkId = id_str.parse().unwrap();
        assert_eq!(id.to_string(), id_str);
    }

    #[test]
    fn survives_round_trip() {
        let id = ChunkId::new();
        let id_str = id.to_string();
        assert_eq!(id, ChunkId::recreate(&id_str))
    }
}

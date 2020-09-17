use serde::{Deserialize, Serialize};
use std::default::Default;
use std::str::FromStr;

/// Metadata about chunks.
///
/// We manage three bits of metadata about chunks, in addition to its
/// identifier:
///
/// * for all chunks, a [SHA256][] checksum of the chunk content
///
/// * for generation chunks, an indication that it is a generation
///   chunk, and a timestamp for when making the generation snapshot
///   ended
///
/// There is no syntax or semantics imposed on the timestamp, but a
/// client should probably use [ISO 8601][] representation.
///
/// For HTTP, the metadata will be serialised as a JSON object, like this:
///
/// ~~~json
/// {
///     "sha256": "09ca7e4eaa6e8ae9c7d261167129184883644d07dfba7cbfbc4c8a2e08360d5b",
///     "generation": true,
///     "ended": "2020-09-17T08:17:13+03:00"
/// }
/// ~~~
///
/// This module provides functions for serializing to and from JSON.
/// The JSON doesn't have to include the fields for generations if
/// they're not needed, although when serialized, they will always be
/// there.
///
/// After chunk metadata is created, it is immutable.
///
/// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
/// [SHA256]: https://en.wikipedia.org/wiki/SHA-2
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkMeta {
    sha256: String,
    // The remaining fields are Options so that JSON parsing doesn't
    // insist on them being there in the textual representation.
    generation: Option<bool>,
    ended: Option<String>,
}

impl ChunkMeta {
    /// Create a new data chunk.
    ///
    /// Data chunks are not for generations.
    pub fn new(sha256: &str) -> Self {
        ChunkMeta {
            sha256: sha256.to_string(),
            generation: None,
            ended: None,
        }
    }

    /// Create a new generation chunk.
    pub fn new_generation(sha256: &str, ended: &str) -> Self {
        ChunkMeta {
            sha256: sha256.to_string(),
            generation: Some(true),
            ended: Some(ended.to_string()),
        }
    }

    /// Is this a generation chunk?
    pub fn is_generation(&self) -> bool {
        match self.generation {
            Some(true) => true,
            _ => false,
        }
    }

    /// When did this generation end?
    pub fn ended(&self) -> Option<&str> {
        self.ended.as_deref().map(|s| s)
    }

    /// SHA256 checksum of the content of the chunk.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Serialize as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl FromStr for ChunkMeta {
    type Err = serde_json::error::Error;

    /// Parse a JSON representation metdata.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod test {
    use super::ChunkMeta;

    #[test]
    fn new_creates_data_chunk() {
        let meta = ChunkMeta::new("abcdef");
        assert!(!meta.is_generation());
        assert_eq!(meta.ended(), None);
        assert_eq!(meta.sha256(), "abcdef");
    }

    #[test]
    fn new_generation_creates_generation_chunk() {
        let meta = ChunkMeta::new_generation("abcdef", "2020-09-17T08:17:13+03:00");
        assert!(meta.is_generation());
        assert_eq!(meta.ended(), Some("2020-09-17T08:17:13+03:00"));
        assert_eq!(meta.sha256(), "abcdef");
    }

    #[test]
    fn data_chunk_from_json() {
        let meta: ChunkMeta = r#"{"sha256": "abcdef"}"#.parse().unwrap();
        assert!(!meta.is_generation());
        assert_eq!(meta.ended(), None);
        assert_eq!(meta.sha256(), "abcdef");
    }

    #[test]
    fn generation_chunk_from_json() {
        let meta: ChunkMeta =
            r#"{"sha256": "abcdef", "generation": true, "ended": "2020-09-17T08:17:13+03:00"}"#
                .parse()
                .unwrap();
        assert!(meta.is_generation());
        assert_eq!(meta.ended(), Some("2020-09-17T08:17:13+03:00"));
        assert_eq!(meta.sha256(), "abcdef");
    }

    #[test]
    fn json_roundtrip() {
        let meta = ChunkMeta::new_generation("abcdef", "2020-09-17T08:17:13+03:00");
        let json = serde_json::to_string(&meta).unwrap();
        let meta2 = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, meta2);
    }
}

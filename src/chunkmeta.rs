//! Metadata about a chunk.

use crate::label::Label;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::str::FromStr;

/// Metadata about chunks.
///
/// We a single piece of metadata about chunks, in addition to its
/// identifier: a label assigned by the client. Currently, this is a
/// [SHA256][] checksum of the chunk content.
///
/// For HTTP, the metadata will be serialised as a JSON object, like this:
///
/// ~~~json
/// {
///     "label": "09ca7e4eaa6e8ae9c7d261167129184883644d07dfba7cbfbc4c8a2e08360d5b",
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkMeta {
    label: String,
}

impl ChunkMeta {
    /// Create a new data chunk.
    ///
    /// Data chunks are not for generations.
    pub fn new(label: &Label) -> Self {
        ChunkMeta {
            label: label.to_string(),
        }
    }

    /// The label of the content of the chunk.
    ///
    /// The caller should not interpret the label in any way. It
    /// happens to be a SHA256 of the cleartext contents of the
    /// checksum for now, but that _will_ change in the future.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Serialize from a textual JSON representation.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    /// Serialize as JSON, as a byte vector.
    pub fn to_json_vec(&self) -> Vec<u8> {
        self.to_json().as_bytes().to_vec()
    }
}

impl FromStr for ChunkMeta {
    type Err = serde_json::error::Error;

    /// Parse a JSON representation metadata.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod test {
    use super::{ChunkMeta, Label};

    #[test]
    fn new_creates_data_chunk() {
        let sum = Label::sha256(b"abcdef");
        let meta = ChunkMeta::new(&sum);
        assert_eq!(meta.label(), &format!("{}", sum));
    }

    #[test]
    fn new_generation_creates_generation_chunk() {
        let sum = Label::sha256(b"abcdef");
        let meta = ChunkMeta::new(&sum);
        assert_eq!(meta.label(), &format!("{}", sum));
    }

    #[test]
    fn data_chunk_from_json() {
        let meta: ChunkMeta = r#"{"label": "abcdef"}"#.parse().unwrap();
        assert_eq!(meta.label(), "abcdef");
    }

    #[test]
    fn generation_chunk_from_json() {
        let meta: ChunkMeta =
            r#"{"label": "abcdef", "generation": true, "ended": "2020-09-17T08:17:13+03:00"}"#
                .parse()
                .unwrap();

        assert_eq!(meta.label(), "abcdef");
    }

    #[test]
    fn generation_json_roundtrip() {
        let sum = Label::sha256(b"abcdef");
        let meta = ChunkMeta::new(&sum);
        let json = serde_json::to_string(&meta).unwrap();
        let meta2 = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, meta2);
    }

    #[test]
    fn data_json_roundtrip() {
        let sum = Label::sha256(b"abcdef");
        let meta = ChunkMeta::new(&sum);
        let json = meta.to_json_vec();
        let meta2 = serde_json::from_slice(&json).unwrap();
        assert_eq!(meta, meta2);
        assert_eq!(meta.to_json_vec(), meta2.to_json_vec());
    }
}

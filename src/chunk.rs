//! Chunks of data.

use crate::checksummer::Checksum;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use serde::{Deserialize, Serialize};
use std::default::Default;

/// An arbitrary chunk of arbitrary binary data.
///
/// A chunk also contains its associated metadata, except its
/// identifier, so that it's easy to keep the data and metadata
/// together. The identifier is used to find the chunk, and it's
/// assigned by the server when the chunk is uploaded, so it's not
/// stored in the chunk itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataChunk {
    data: Vec<u8>,
    meta: ChunkMeta,
}

impl DataChunk {
    /// Create a new chunk.
    pub fn new(data: Vec<u8>, meta: ChunkMeta) -> Self {
        Self { data, meta }
    }

    /// Return a chunk's data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Return a chunk's metadata.
    pub fn meta(&self) -> &ChunkMeta {
        &self.meta
    }
}

/// A chunk representing a backup generation.
///
/// A generation chunk lists all the data chunks for the SQLite file
/// with the backup's metadata. It's different from a normal data
/// chunk so that we can do things that make no sense to a data chunk.
/// Generation chunks can be converted into or created from data
/// chunks, for uploading to or downloading from the server.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GenerationChunk {
    chunk_ids: Vec<ChunkId>,
}

/// All the errors that may be returned for `GenerationChunk` operations.
#[derive(Debug, thiserror::Error)]
pub enum GenerationChunkError {
    /// Error converting text from UTF8.
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Error parsing JSON as chunk metadata.
    #[error("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),

    /// Error generating JSON from chunk metadata.
    #[error("failed to serialize to JSON: {0}")]
    JsonGenerate(serde_json::Error),
}

impl GenerationChunk {
    /// Create a new backup generation chunk from metadata chunk ids.
    pub fn new(chunk_ids: Vec<ChunkId>) -> Self {
        Self { chunk_ids }
    }

    /// Create a new backup generation chunk from a data chunk.
    pub fn from_data_chunk(chunk: &DataChunk) -> Result<Self, GenerationChunkError> {
        let data = chunk.data();
        let data = std::str::from_utf8(data)?;
        serde_json::from_str(data).map_err(GenerationChunkError::JsonParse)
    }

    /// Does the generation chunk contain any metadata chunks?
    pub fn is_empty(&self) -> bool {
        self.chunk_ids.is_empty()
    }

    /// How many metadata chunks does generation chunk contain?
    pub fn len(&self) -> usize {
        self.chunk_ids.len()
    }

    /// Return iterator over the metadata chunk identifiers.
    pub fn chunk_ids(&self) -> impl Iterator<Item = &ChunkId> {
        self.chunk_ids.iter()
    }

    /// Convert generation chunk to a data chunk.
    pub fn to_data_chunk(&self, ended: &str) -> Result<DataChunk, GenerationChunkError> {
        let json: String =
            serde_json::to_string(self).map_err(GenerationChunkError::JsonGenerate)?;
        let bytes = json.as_bytes().to_vec();
        let sha = Checksum::sha256(&bytes);
        let meta = ChunkMeta::new_generation(&sha, ended);
        Ok(DataChunk::new(bytes, meta))
    }
}

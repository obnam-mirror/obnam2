use crate::checksummer::sha256;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use serde::{Deserialize, Serialize};
use std::default::Default;

/// Store an arbitrary chunk of data.
///
/// The data is just arbitrary binary data.
///
/// A chunk also contains its associated metadata, except its
/// identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataChunk {
    data: Vec<u8>,
    meta: ChunkMeta,
}

impl DataChunk {
    /// Construct a new chunk.
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

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GenerationChunk {
    chunk_ids: Vec<ChunkId>,
}

/// All the errors that may be returned for `GenerationChunk` operations.
#[derive(Debug, thiserror::Error)]
pub enum GenerationChunkError {
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),

    #[error("failed to serialize to JSON: {0}")]
    JsonGenerate(serde_json::Error),
}

impl GenerationChunk {
    pub fn new(chunk_ids: Vec<ChunkId>) -> Self {
        Self { chunk_ids }
    }

    pub fn from_data_chunk(chunk: &DataChunk) -> Result<Self, GenerationChunkError> {
        let data = chunk.data();
        let data = std::str::from_utf8(data)?;
        serde_json::from_str(data).map_err(GenerationChunkError::JsonParse)
    }

    pub fn is_empty(&self) -> bool {
        self.chunk_ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.chunk_ids.len()
    }

    pub fn chunk_ids(&self) -> impl Iterator<Item = &ChunkId> {
        self.chunk_ids.iter()
    }

    pub fn to_data_chunk(&self, ended: &str) -> Result<DataChunk, GenerationChunkError> {
        let json: String =
            serde_json::to_string(self).map_err(GenerationChunkError::JsonGenerate)?;
        let bytes = json.as_bytes().to_vec();
        let sha = sha256(&bytes);
        let meta = ChunkMeta::new_generation(&sha, ended);
        Ok(DataChunk::new(bytes, meta))
    }
}

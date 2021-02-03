use crate::chunkid::ChunkId;
use serde::{Deserialize, Serialize};
use std::default::Default;

/// Store an arbitrary chunk of data.
///
/// The data is just arbitrary binary data.
///
/// A chunk also contains its associated metadata, except its
/// identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChunk {
    data: Vec<u8>,
}

impl DataChunk {
    /// Construct a new chunk.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Return a chunk's data.
    pub fn data(&self) -> &[u8] {
        &self.data
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

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

/// A result from a chunk operation.
pub type GenerationChunkResult<T> = Result<T, GenerationChunkError>;

impl GenerationChunk {
    pub fn new(chunk_ids: Vec<ChunkId>) -> Self {
        Self { chunk_ids }
    }

    pub fn from_data_chunk(chunk: &DataChunk) -> GenerationChunkResult<Self> {
        let data = chunk.data();
        let data = std::str::from_utf8(data)?;
        Ok(serde_json::from_str(data)?)
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

    pub fn to_data_chunk(&self) -> GenerationChunkResult<DataChunk> {
        let json = serde_json::to_string(self)?;
        Ok(DataChunk::new(json.as_bytes().to_vec()))
    }
}

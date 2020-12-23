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

impl GenerationChunk {
    pub fn new(chunk_ids: Vec<ChunkId>) -> Self {
        Self { chunk_ids }
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

    pub fn to_data_chunk(&self) -> anyhow::Result<DataChunk> {
        let json = serde_json::to_string(self)?;
        Ok(DataChunk::new(json.as_bytes().to_vec()))
    }
}

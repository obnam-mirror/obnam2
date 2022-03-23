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
    pub fn to_data_chunk(&self) -> Result<DataChunk, GenerationChunkError> {
        let json: String =
            serde_json::to_string(self).map_err(GenerationChunkError::JsonGenerate)?;
        let bytes = json.as_bytes().to_vec();
        let checksum = Checksum::sha256(&bytes);
        let meta = ChunkMeta::new(&checksum);
        Ok(DataChunk::new(bytes, meta))
    }
}

/// A client trust root chunk.
///
/// This chunk contains all per-client backup information. As long as
/// this chunk can be trusted, everything it links to can also be
/// trusted, thanks to cryptographic signatures.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientTrust {
    client_name: String,
    previous_version: Option<ChunkId>,
    timestamp: String,
    backups: Vec<ChunkId>,
}

/// All the errors that may be returned for `ClientTrust` operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientTrustError {
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

impl ClientTrust {
    /// Create a new ClientTrust object.
    pub fn new(
        name: &str,
        previous_version: Option<ChunkId>,
        timestamp: String,
        backups: Vec<ChunkId>,
    ) -> Self {
        Self {
            client_name: name.to_string(),
            previous_version,
            timestamp,
            backups,
        }
    }

    /// Return client name.
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// Return id of previous version, if any.
    pub fn previous_version(&self) -> Option<ChunkId> {
        self.previous_version.clone()
    }

    /// Return timestamp.
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    /// Return list of all backup generations known.
    pub fn backups(&self) -> &[ChunkId] {
        &self.backups
    }

    /// Append a backup generation to the list.
    pub fn append_backup(&mut self, id: &ChunkId) {
        self.backups.push(id.clone());
    }

    /// Update for new upload.
    ///
    /// This needs to happen every time the chunk is updated so that
    /// the timestamp gets updated.
    pub fn finalize(&mut self, timestamp: String) {
        self.timestamp = timestamp;
    }

    /// Convert generation chunk to a data chunk.
    pub fn to_data_chunk(&self) -> Result<DataChunk, ClientTrustError> {
        let json: String = serde_json::to_string(self).map_err(ClientTrustError::JsonGenerate)?;
        let bytes = json.as_bytes().to_vec();
        let checksum = Checksum::sha256_from_str_unchecked("client-trust");
        let meta = ChunkMeta::new(&checksum);
        Ok(DataChunk::new(bytes, meta))
    }

    /// Create a new ClientTrust from a data chunk.
    pub fn from_data_chunk(chunk: &DataChunk) -> Result<Self, ClientTrustError> {
        let data = chunk.data();
        let data = std::str::from_utf8(data)?;
        serde_json::from_str(data).map_err(ClientTrustError::JsonParse)
    }
}

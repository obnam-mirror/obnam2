use crate::chunkid::ChunkId;
use std::path::PathBuf;
use thiserror::Error;

/// Define all the kinds of errors any part of this crate can return.
#[derive(Debug, Error)]
pub enum ObnamError {
    #[error("Can't find backup '{0}'")]
    UnknownGeneration(String),

    #[error("Generation has more than one file with the name {0}")]
    TooManyFiles(PathBuf),

    #[error("Server response did not have a 'chunk-meta' header for chunk {0}")]
    NoChunkMeta(ChunkId),

    #[error("Wrong checksum for chunk {0}, got {1}, expected {2}")]
    WrongChecksum(ChunkId, String, String),

    #[error("Chunk is missing: {0}")]
    MissingChunk(ChunkId),

    #[error("Chunk is in store too many times: {0}")]
    DuplicateChunk(ChunkId),
}

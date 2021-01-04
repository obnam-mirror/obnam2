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
    NoChunkMeta(String),

    #[error("Wrong checksum for chunk {0}")]
    WrongChecksum(String),
}

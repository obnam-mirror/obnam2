use std::path::PathBuf;
use thiserror::Error;

/// Define all the kinds of errors any part of this crate can return.
#[derive(Debug, Error)]
pub enum ObnamError {
    #[error("Can't find backup '{0}'")]
    UnknownGeneration(String),

    #[error("Generation has more than one file with the name {0}")]
    TooManyFiles(PathBuf),
}

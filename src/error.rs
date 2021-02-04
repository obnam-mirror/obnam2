use crate::backup_run::BackupError;
use crate::client::{ClientConfigError, ClientError};
use crate::cmd::restore::RestoreError;
use crate::generation::{LocalGenerationError, NascentError};
use crate::genlist::GenerationListError;
use std::time::SystemTimeError;
use tempfile::PersistError;
use thiserror::Error;

/// Define all the kinds of errors that functions corresponding to
/// subcommands of the main program can return.
#[derive(Debug, Error)]
pub enum ObnamError {
    #[error(transparent)]
    GenerationListError(#[from] GenerationListError),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    ClientConfigError(#[from] ClientConfigError),

    #[error(transparent)]
    BackupError(#[from] BackupError),

    #[error(transparent)]
    NascentError(#[from] NascentError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    #[error(transparent)]
    RestoreError(#[from] RestoreError),

    #[error(transparent)]
    PersistError(#[from] PersistError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SystemTimeError(#[from] SystemTimeError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

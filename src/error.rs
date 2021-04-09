use crate::backup_run::BackupError;
use crate::client::{ClientConfigError, ClientError};
use crate::cmd::restore::RestoreError;
use crate::generation::{LocalGenerationError, NascentError};
use crate::genlist::GenerationListError;
use crate::passwords::PasswordError;
use std::path::PathBuf;
use std::time::SystemTimeError;
use tempfile::PersistError;

/// Define all the kinds of errors that functions corresponding to
/// subcommands of the main program can return.
#[derive(Debug, thiserror::Error)]
pub enum ObnamError {
    #[error(transparent)]
    GenerationListError(#[from] GenerationListError),

    #[error("couldn't save passwords to {0}: {1}")]
    PasswordSave(PathBuf, PasswordError),

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

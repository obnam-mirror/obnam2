//! Errors from Obnam client.

use crate::backup_run::BackupError;
use crate::cipher::CipherError;
use crate::client::ClientError;
use crate::cmd::restore::RestoreError;
use crate::config::ClientConfigError;
use crate::db::DatabaseError;
use crate::dbgen::GenerationDbError;
use crate::generation::{LocalGenerationError, NascentError};
use crate::genlist::GenerationListError;
use crate::passwords::PasswordError;
use std::path::PathBuf;
use std::time::SystemTimeError;
use tempfile::PersistError;

/// Define all the kinds of errors that functions corresponding to
/// subcommands of the main program can return.
///
/// This collects all kinds of errors the Obnam client may get, for
/// convenience.
#[derive(Debug, thiserror::Error)]
pub enum ObnamError {
    /// Error listing generations on server.
    #[error(transparent)]
    GenerationListError(#[from] GenerationListError),

    /// Error saving passwords.
    #[error("couldn't save passwords to {0}: {1}")]
    PasswordSave(PathBuf, PasswordError),

    /// Error using server HTTP API.
    #[error(transparent)]
    ClientError(#[from] ClientError),

    /// Error in client configuration.
    #[error(transparent)]
    ClientConfigError(#[from] ClientConfigError),

    /// Error making a backup.
    #[error(transparent)]
    BackupError(#[from] BackupError),

    /// Error making a new backup generation.
    #[error(transparent)]
    NascentError(#[from] NascentError),

    /// Error encrypting or decrypting.
    #[error(transparent)]
    CipherError(#[from] CipherError),

    /// Error using local copy of existing backup generation.
    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    /// Error from generation database.
    #[error(transparent)]
    GenerationDb(#[from] GenerationDbError),

    /// Error using a Database.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Error restoring a backup.
    #[error(transparent)]
    RestoreError(#[from] RestoreError),

    /// Error making temporary file persistent.
    #[error(transparent)]
    PersistError(#[from] PersistError),

    /// Error doing I/O.
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// Error reading system clock.
    #[error(transparent)]
    SystemTimeError(#[from] SystemTimeError),

    /// Error regarding JSON.
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    /// Unexpected cache directories found.
    #[error(
        "found CACHEDIR.TAG files that aren't present in the previous backup, might be an attack"
    )]
    NewCachedirTagsFound,
}

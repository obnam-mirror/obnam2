use crate::backup_progress::BackupProgress;
use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::client::{BackupClient, ClientConfig, ClientError};
use crate::fsentry::FilesystemEntry;
use crate::fsiter::{FsIterError, FsIterResult};
use crate::generation::{LocalGeneration, LocalGenerationError};
use crate::policy::BackupPolicy;
use log::{info, warn};

pub struct BackupRun {
    client: BackupClient,
    policy: BackupPolicy,
    buffer_size: usize,
    progress: BackupProgress,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    FsIterError(#[from] FsIterError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),
}

pub type BackupResult<T> = Result<T, BackupError>;

impl BackupRun {
    pub fn new(config: &ClientConfig) -> BackupResult<Self> {
        let client = BackupClient::new(config)?;
        let policy = BackupPolicy::new();
        let progress = BackupProgress::new();
        Ok(Self {
            client,
            policy,
            buffer_size: config.chunk_size,
            progress,
        })
    }

    pub fn client(&self) -> &BackupClient {
        &self.client
    }

    pub fn progress(&self) -> &BackupProgress {
        &self.progress
    }

    pub fn backup_file_initially(
        &self,
        entry: FsIterResult<FilesystemEntry>,
    ) -> BackupResult<(FilesystemEntry, Vec<ChunkId>, Reason)> {
        match entry {
            Err(err) => Err(err.into()),
            Ok(entry) => {
                let path = &entry.pathbuf();
                info!("backup: {}", path.display());
                self.progress.found_live_file(path);
                let ids = self
                    .client
                    .upload_filesystem_entry(&entry, self.buffer_size)?;
                Ok((entry.clone(), ids, Reason::IsNew))
            }
        }
    }

    pub fn backup_file_incrementally(
        &self,
        entry: FsIterResult<FilesystemEntry>,
        old: &LocalGeneration,
    ) -> BackupResult<(FilesystemEntry, Vec<ChunkId>, Reason)> {
        match entry {
            Err(err) => {
                warn!("backup: {}", err);
                self.progress.found_problem();
                Err(BackupError::FsIterError(err))
            }
            Ok(entry) => {
                let path = &entry.pathbuf();
                info!("backup: {}", path.display());
                self.progress.found_live_file(path);
                let reason = self.policy.needs_backup(&old, &entry);
                match reason {
                    Reason::IsNew | Reason::Changed | Reason::Error => {
                        let ids = self
                            .client
                            .upload_filesystem_entry(&entry, self.buffer_size)?;
                        Ok((entry.clone(), ids, reason))
                    }
                    Reason::Unchanged | Reason::Skipped => {
                        let fileno = old.get_fileno(&entry.pathbuf())?;
                        let ids = if let Some(fileno) = fileno {
                            old.chunkids(fileno)?
                        } else {
                            vec![]
                        };
                        Ok((entry.clone(), ids, reason))
                    }
                }
            }
        }
    }
}

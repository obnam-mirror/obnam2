use crate::backup_progress::BackupProgress;
use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::client::{BackupClient, ClientError};
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemEntry;
use crate::fsiter::{FsIterError, FsIterResult};
use crate::generation::{LocalGeneration, LocalGenerationError};
use crate::policy::BackupPolicy;
use log::{info, warn};
use std::path::Path;

pub struct InitialBackup<'a> {
    client: &'a BackupClient,
    buffer_size: usize,
    progress: BackupProgress,
}

pub struct IncrementalBackup<'a> {
    client: &'a BackupClient,
    policy: BackupPolicy,
    buffer_size: usize,
    progress: Option<BackupProgress>,
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

impl<'a> InitialBackup<'a> {
    pub fn new(config: &ClientConfig, client: &'a BackupClient) -> BackupResult<Self> {
        let progress = BackupProgress::initial();
        Ok(Self {
            client,
            buffer_size: config.chunk_size,
            progress,
        })
    }

    pub fn drop(&self) {
        self.progress.finish();
    }

    pub fn backup(
        &self,
        entry: FsIterResult<FilesystemEntry>,
    ) -> BackupResult<(FilesystemEntry, Vec<ChunkId>, Reason)> {
        match entry {
            Err(err) => {
                warn!("backup: there was a problem: {:?}", err);
                self.progress.found_problem();
                Err(err.into())
            }
            Ok(entry) => {
                let path = &entry.pathbuf();
                info!("backup: {}", path.display());
                self.progress.found_live_file(path);
                Ok(backup_file(
                    &self.client,
                    &entry,
                    &path,
                    self.buffer_size,
                    Reason::IsNew,
                ))
            }
        }
    }
}

impl<'a> IncrementalBackup<'a> {
    pub fn new(config: &ClientConfig, client: &'a BackupClient) -> BackupResult<Self> {
        let policy = BackupPolicy::default();
        Ok(Self {
            client,
            policy,
            buffer_size: config.chunk_size,
            progress: None,
        })
    }

    pub fn start_backup(&mut self, old: &LocalGeneration) -> Result<(), ObnamError> {
        let progress = BackupProgress::incremental();
        progress.files_in_previous_generation(old.file_count()? as u64);
        self.progress = Some(progress);
        Ok(())
    }

    pub fn client(&self) -> &BackupClient {
        self.client
    }

    pub fn drop(&self) {
        if let Some(progress) = &self.progress {
            progress.finish();
        }
    }

    pub fn fetch_previous_generation(
        &self,
        genid: &str,
        oldname: &Path,
    ) -> Result<LocalGeneration, ObnamError> {
        let progress = BackupProgress::download_generation(genid);
        let old = self.client().fetch_generation(genid, &oldname)?;
        progress.finish();
        Ok(old)
    }

    pub fn backup(
        &self,
        entry: FsIterResult<FilesystemEntry>,
        old: &LocalGeneration,
    ) -> BackupResult<(FilesystemEntry, Vec<ChunkId>, Reason)> {
        match entry {
            Err(err) => {
                warn!("backup: {}", err);
                self.found_problem();
                Err(BackupError::FsIterError(err))
            }
            Ok(entry) => {
                let path = &entry.pathbuf();
                info!("backup: {}", path.display());
                self.found_live_file(path);
                let reason = self.policy.needs_backup(&old, &entry);
                match reason {
                    Reason::IsNew
                    | Reason::Changed
                    | Reason::GenerationLookupError
                    | Reason::Unknown => Ok(backup_file(
                        &self.client,
                        &entry,
                        &path,
                        self.buffer_size,
                        reason,
                    )),
                    Reason::Unchanged | Reason::Skipped | Reason::FileError => {
                        let fileno = old.get_fileno(&entry.pathbuf())?;
                        let ids = if let Some(fileno) = fileno {
                            let mut ids = vec![];
                            for id in old.chunkids(fileno)?.iter()? {
                                ids.push(id?);
                            }
                            ids
                        } else {
                            vec![]
                        };
                        Ok((entry, ids, reason))
                    }
                }
            }
        }
    }

    fn found_live_file(&self, path: &Path) {
        if let Some(progress) = &self.progress {
            progress.found_live_file(path);
        }
    }

    fn found_problem(&self) {
        if let Some(progress) = &self.progress {
            progress.found_problem();
        }
    }
}

fn backup_file(
    client: &BackupClient,
    entry: &FilesystemEntry,
    path: &Path,
    chunk_size: usize,
    reason: Reason,
) -> (FilesystemEntry, Vec<ChunkId>, Reason) {
    let ids = client.upload_filesystem_entry(&entry, chunk_size);
    match ids {
        Err(err) => {
            warn!("error backing up {}, skipping it: {}", path.display(), err);
            (entry.clone(), vec![], Reason::FileError)
        }
        Ok(ids) => (entry.clone(), ids, reason),
    }
}

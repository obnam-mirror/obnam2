use crate::backup_progress::BackupProgress;
use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::client::{BackupClient, ClientError};
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemEntry;
use crate::fsiter::{FsIterError, FsIterResult, FsIterator};
use crate::generation::{LocalGeneration, LocalGenerationError, NascentError, NascentGeneration};
use crate::policy::BackupPolicy;
use log::{info, warn};
use std::path::Path;

pub struct BackupRun<'a> {
    client: &'a BackupClient,
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

#[derive(Debug)]
pub struct FsEntryBackupOutcome {
    pub entry: FilesystemEntry,
    pub ids: Vec<ChunkId>,
    pub reason: Reason,
}

impl<'a> BackupRun<'a> {
    pub fn initial(config: &ClientConfig, client: &'a BackupClient) -> Result<Self, BackupError> {
        Ok(Self {
            client,
            policy: BackupPolicy::default(),
            buffer_size: config.chunk_size,
            progress: BackupProgress::initial(),
        })
    }

    pub fn incremental(
        config: &ClientConfig,
        client: &'a BackupClient,
    ) -> Result<Self, BackupError> {
        Ok(Self {
            client,
            policy: BackupPolicy::default(),
            buffer_size: config.chunk_size,
            progress: BackupProgress::incremental(),
        })
    }

    pub fn start(
        &mut self,
        genid: Option<&str>,
        oldname: &Path,
    ) -> Result<LocalGeneration, ObnamError> {
        match genid {
            None => {
                // Create a new, empty generation.
                NascentGeneration::create(oldname)?;

                // Open the newly created empty generation.
                Ok(LocalGeneration::open(oldname)?)
            }
            Some(genid) => {
                let old = self.fetch_previous_generation(genid, oldname)?;
                self.progress
                    .files_in_previous_generation(old.file_count()? as u64);
                Ok(old)
            }
        }
    }

    fn fetch_previous_generation(
        &self,
        genid: &str,
        oldname: &Path,
    ) -> Result<LocalGeneration, ObnamError> {
        let progress = BackupProgress::download_generation(genid);
        let old = self.client.fetch_generation(genid, &oldname)?;
        progress.finish();
        Ok(old)
    }

    pub fn finish(&self) {
        self.progress.finish();
    }

    pub fn backup_roots(
        &self,
        config: &ClientConfig,
        old: &LocalGeneration,
        newpath: &Path,
    ) -> Result<(i64, Vec<BackupError>), NascentError> {
        let mut all_warnings = vec![];
        let count = {
            let mut new = NascentGeneration::create(newpath)?;
            for root in &config.roots {
                let iter = FsIterator::new(root, config.exclude_cache_tag_directories);
                let mut warnings = new.insert_iter(iter.map(|entry| self.backup(entry, &old)))?;
                all_warnings.append(&mut warnings);
            }
            new.file_count()
        };
        self.finish();
        Ok((count, all_warnings))
    }

    pub fn backup(
        &self,
        entry: FsIterResult<FilesystemEntry>,
        old: &LocalGeneration,
    ) -> Result<FsEntryBackupOutcome, BackupError> {
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
                        Ok(FsEntryBackupOutcome { entry, ids, reason })
                    }
                }
            }
        }
    }

    fn found_live_file(&self, path: &Path) {
        self.progress.found_live_file(path);
    }

    fn found_problem(&self) {
        self.progress.found_problem();
    }
}

fn backup_file(
    client: &BackupClient,
    entry: &FilesystemEntry,
    path: &Path,
    chunk_size: usize,
    reason: Reason,
) -> FsEntryBackupOutcome {
    let ids = client.upload_filesystem_entry(&entry, chunk_size);
    match ids {
        Err(err) => {
            warn!("error backing up {}, skipping it: {}", path.display(), err);
            FsEntryBackupOutcome {
                entry: entry.clone(),
                ids: vec![],
                reason: Reason::FileError,
            }
        }
        Ok(ids) => FsEntryBackupOutcome {
            entry: entry.clone(),
            ids,
            reason,
        },
    }
}

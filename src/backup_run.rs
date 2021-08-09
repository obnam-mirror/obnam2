use crate::backup_progress::BackupProgress;
use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::client::{AsyncBackupClient, ClientError};
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemEntry;
use crate::fsiter::{AnnotatedFsEntry, FsIterError, FsIterator};
use crate::generation::{
    GenId, LocalGeneration, LocalGenerationError, NascentError, NascentGeneration,
};
use crate::policy::BackupPolicy;
use log::{debug, info, warn};
use std::path::{Path, PathBuf};

pub struct BackupRun<'a> {
    client: &'a AsyncBackupClient,
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
    NascentError(#[from] NascentError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),
}

#[derive(Debug)]
pub struct FsEntryBackupOutcome {
    pub entry: FilesystemEntry,
    pub ids: Vec<ChunkId>,
    pub reason: Reason,
    pub is_cachedir_tag: bool,
}

#[derive(Debug)]
pub struct RootsBackupOutcome {
    /// The number of backed up files.
    pub files_count: i64,
    /// The errors encountered while backing up files.
    pub warnings: Vec<BackupError>,
    /// CACHEDIR.TAG files that aren't present in in a previous generation.
    pub new_cachedir_tags: Vec<PathBuf>,
}

impl<'a> BackupRun<'a> {
    pub fn initial(
        config: &ClientConfig,
        client: &'a AsyncBackupClient,
    ) -> Result<Self, BackupError> {
        Ok(Self {
            client,
            policy: BackupPolicy::default(),
            buffer_size: config.chunk_size,
            progress: Some(BackupProgress::initial()),
        })
    }

    pub fn incremental(
        config: &ClientConfig,
        client: &'a AsyncBackupClient,
    ) -> Result<Self, BackupError> {
        Ok(Self {
            client,
            policy: BackupPolicy::default(),
            buffer_size: config.chunk_size,
            progress: None,
        })
    }

    pub async fn start(
        &mut self,
        genid: Option<&GenId>,
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
                let old = self.fetch_previous_generation(genid, oldname).await?;

                let progress = BackupProgress::incremental();
                progress.files_in_previous_generation(old.file_count()? as u64);
                self.progress = Some(progress);

                Ok(old)
            }
        }
    }

    async fn fetch_previous_generation(
        &self,
        genid: &GenId,
        oldname: &Path,
    ) -> Result<LocalGeneration, ObnamError> {
        let progress = BackupProgress::download_generation(genid);
        let old = self.client.fetch_generation(genid, oldname).await?;
        progress.finish();
        Ok(old)
    }

    pub fn finish(&self) {
        if let Some(progress) = &self.progress {
            progress.finish();
        }
    }

    pub async fn backup_roots(
        &self,
        config: &ClientConfig,
        old: &LocalGeneration,
        newpath: &Path,
    ) -> Result<RootsBackupOutcome, NascentError> {
        let mut warnings: Vec<BackupError> = vec![];
        let mut new_cachedir_tags = vec![];
        let files_count = {
            let mut new = NascentGeneration::create(newpath)?;
            for root in &config.roots {
                match self.backup_one_root(config, old, &mut new, root).await {
                    Ok(mut o) => {
                        new_cachedir_tags.append(&mut o.new_cachedir_tags);
                        if !o.warnings.is_empty() {
                            for err in o.warnings.iter() {
                                debug!("ignoring backup error {}", err);
                                self.found_problem();
                            }
                            warnings.append(&mut o.warnings);
                        }
                    }
                    Err(err) => {
                        debug!("ignoring backup error {}", err);
                        warnings.push(err.into());
                        self.found_problem();
                    }
                }
            }
            new.file_count()
        };
        self.finish();
        Ok(RootsBackupOutcome {
            files_count,
            warnings,
            new_cachedir_tags,
        })
    }

    async fn backup_one_root(
        &self,
        config: &ClientConfig,
        old: &LocalGeneration,
        new: &mut NascentGeneration,
        root: &Path,
    ) -> Result<RootsBackupOutcome, NascentError> {
        let mut warnings: Vec<BackupError> = vec![];
        let mut new_cachedir_tags = vec![];
        let iter = FsIterator::new(root, config.exclude_cache_tag_directories);
        for entry in iter {
            match entry {
                Err(err) => {
                    warnings.push(err.into());
                }
                Ok(entry) => {
                    let path = entry.inner.pathbuf();
                    if entry.is_cachedir_tag && !old.is_cachedir_tag(&path)? {
                        new_cachedir_tags.push(path);
                    }
                    match self.backup_if_changed(entry, old).await {
                        Err(err) => {
                            warnings.push(err);
                        }
                        Ok(o) => {
                            if let Err(err) =
                                new.insert(o.entry, &o.ids, o.reason, o.is_cachedir_tag)
                            {
                                warnings.push(err.into());
                            }
                        }
                    }
                }
            }
        }

        Ok(RootsBackupOutcome {
            files_count: 0, // Caller will get file count from new.
            warnings,
            new_cachedir_tags,
        })
    }

    async fn backup_if_changed(
        &self,
        entry: AnnotatedFsEntry,
        old: &LocalGeneration,
    ) -> Result<FsEntryBackupOutcome, BackupError> {
        let path = &entry.inner.pathbuf();
        info!("backup: {}", path.display());
        self.found_live_file(path);
        let reason = self.policy.needs_backup(old, &entry.inner);
        match reason {
            Reason::IsNew | Reason::Changed | Reason::GenerationLookupError | Reason::Unknown => {
                Ok(backup_one_entry(self.client, &entry, path, self.buffer_size, reason).await)
            }
            Reason::Unchanged | Reason::Skipped | Reason::FileError => {
                let fileno = old.get_fileno(&entry.inner.pathbuf())?;
                let ids = if let Some(fileno) = fileno {
                    let mut ids = vec![];
                    for id in old.chunkids(fileno)?.iter()? {
                        ids.push(id?);
                    }
                    ids
                } else {
                    vec![]
                };
                Ok(FsEntryBackupOutcome {
                    entry: entry.inner,
                    ids,
                    reason,
                    is_cachedir_tag: entry.is_cachedir_tag,
                })
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

async fn backup_one_entry(
    client: &AsyncBackupClient,
    entry: &AnnotatedFsEntry,
    path: &Path,
    chunk_size: usize,
    reason: Reason,
) -> FsEntryBackupOutcome {
    let ids = client
        .upload_filesystem_entry(&entry.inner, chunk_size)
        .await;
    match ids {
        Err(err) => {
            warn!("error backing up {}, skipping it: {}", path.display(), err);
            FsEntryBackupOutcome {
                entry: entry.inner.clone(),
                ids: vec![],
                reason: Reason::FileError,
                is_cachedir_tag: entry.is_cachedir_tag,
            }
        }
        Ok(ids) => FsEntryBackupOutcome {
            entry: entry.inner.clone(),
            ids,
            reason,
            is_cachedir_tag: entry.is_cachedir_tag,
        },
    }
}

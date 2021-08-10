use crate::backup_progress::BackupProgress;
use crate::backup_reason::Reason;
use crate::chunk::{GenerationChunk, GenerationChunkError};
use crate::chunker::{Chunker, ChunkerError};
use crate::chunkid::ChunkId;
use crate::client::{AsyncBackupClient, ClientError};
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::fsiter::{AnnotatedFsEntry, FsIterError, FsIterator};
use crate::generation::{
    GenId, LocalGeneration, LocalGenerationError, NascentError, NascentGeneration,
};
use crate::policy::BackupPolicy;

use bytesize::MIB;
use chrono::{DateTime, Local};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};

const SQLITE_CHUNK_SIZE: usize = MIB as usize;

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

    #[error(transparent)]
    ChunkerError(#[from] ChunkerError),

    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),
}

#[derive(Debug)]
pub struct FsEntryBackupOutcome {
    pub entry: FilesystemEntry,
    pub ids: Vec<ChunkId>,
    pub reason: Reason,
    pub is_cachedir_tag: bool,
}

#[derive(Debug)]
struct OneRootBackupOutcome {
    pub files_count: i64,
    pub warnings: Vec<BackupError>,
    pub new_cachedir_tags: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct RootsBackupOutcome {
    /// The number of backed up files.
    pub files_count: i64,
    /// The errors encountered while backing up files.
    pub warnings: Vec<BackupError>,
    /// CACHEDIR.TAG files that aren't present in in a previous generation.
    pub new_cachedir_tags: Vec<PathBuf>,
    /// Id of new generation.
    pub gen_id: GenId,
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
    ) -> Result<RootsBackupOutcome, ObnamError> {
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
        let gen_id = self.upload_nascent_generation(newpath).await?;
        let gen_id = GenId::from_chunk_id(gen_id);
        Ok(RootsBackupOutcome {
            files_count,
            warnings,
            new_cachedir_tags,
            gen_id,
        })
    }

    async fn backup_one_root(
        &self,
        config: &ClientConfig,
        old: &LocalGeneration,
        new: &mut NascentGeneration,
        root: &Path,
    ) -> Result<OneRootBackupOutcome, NascentError> {
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
                    match self.backup_if_needed(entry, old).await {
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

        Ok(OneRootBackupOutcome {
            files_count: 0, // Caller will get file count from new.
            warnings,
            new_cachedir_tags,
        })
    }

    async fn backup_if_needed(
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
                Ok(self.backup_one_entry(&entry, path, reason).await)
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

    async fn backup_one_entry(
        &self,
        entry: &AnnotatedFsEntry,
        path: &Path,
        reason: Reason,
    ) -> FsEntryBackupOutcome {
        let ids = self
            .upload_filesystem_entry(&entry.inner, self.buffer_size)
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

    pub async fn upload_filesystem_entry(
        &self,
        e: &FilesystemEntry,
        size: usize,
    ) -> Result<Vec<ChunkId>, BackupError> {
        let path = e.pathbuf();
        info!("uploading {:?}", path);
        let ids = match e.kind() {
            FilesystemKind::Regular => self.upload_regular_file(&path, size).await?,
            FilesystemKind::Directory => vec![],
            FilesystemKind::Symlink => vec![],
            FilesystemKind::Socket => vec![],
            FilesystemKind::Fifo => vec![],
        };
        info!("upload OK for {:?}", path);
        Ok(ids)
    }

    pub async fn upload_generation(
        &self,
        filename: &Path,
        size: usize,
    ) -> Result<ChunkId, BackupError> {
        info!("upload SQLite {}", filename.display());
        let ids = self.upload_regular_file(filename, size).await?;
        let gen = GenerationChunk::new(ids);
        let data = gen.to_data_chunk(&current_timestamp())?;
        let gen_id = self.client.upload_chunk(data).await?;
        info!("uploaded generation {}", gen_id);
        Ok(gen_id)
    }

    async fn upload_regular_file(
        &self,
        filename: &Path,
        size: usize,
    ) -> Result<Vec<ChunkId>, BackupError> {
        info!("upload file {}", filename.display());
        let mut chunk_ids = vec![];
        let file = std::fs::File::open(filename)
            .map_err(|err| ClientError::FileOpen(filename.to_path_buf(), err))?;
        let chunker = Chunker::new(size, file, filename);
        for item in chunker {
            let chunk = item?;
            if let Some(chunk_id) = self.client.has_chunk(chunk.meta()).await? {
                chunk_ids.push(chunk_id.clone());
                info!("reusing existing chunk {}", chunk_id);
            } else {
                let chunk_id = self.client.upload_chunk(chunk).await?;
                chunk_ids.push(chunk_id.clone());
                info!("created new chunk {}", chunk_id);
            }
        }
        Ok(chunk_ids)
    }

    async fn upload_nascent_generation(&self, filename: &Path) -> Result<ChunkId, ObnamError> {
        let progress = BackupProgress::upload_generation();
        let gen_id = self.upload_generation(filename, SQLITE_CHUNK_SIZE).await?;
        progress.finish();
        Ok(gen_id)
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

fn current_timestamp() -> String {
    let now: DateTime<Local> = Local::now();
    format!("{}", now.format("%Y-%m-%d %H:%M:%S.%f %z"))
}

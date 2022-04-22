//! Backup generations of various kinds.

use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::db::{DatabaseError, SqlResults};
use crate::dbgen::{FileId, GenerationDb, GenerationDbError};
use crate::fsentry::FilesystemEntry;
use crate::genmeta::{GenerationMeta, GenerationMetaError};
use crate::label::LabelChecksumKind;
use crate::schema::{SchemaVersion, VersionComponent};
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// An identifier for a generation.
#[derive(Debug, Clone, Serialize)]
pub struct GenId {
    id: ChunkId,
}

impl GenId {
    /// Create a generation identifier from a chunk identifier.
    pub fn from_chunk_id(id: ChunkId) -> Self {
        Self { id }
    }

    /// Convert a generation identifier into a chunk identifier.
    pub fn as_chunk_id(&self) -> &ChunkId {
        &self.id
    }
}

impl fmt::Display for GenId {
    /// Format an identifier for display.
    ///
    /// The output can be parsed to re-created an identical identifier.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// A nascent backup generation.
///
/// A nascent generation is one that is being prepared. It isn't
/// finished yet, and it's not actually on the server until the upload
/// of its generation chunk.
pub struct NascentGeneration {
    db: GenerationDb,
    fileno: FileId,
}

/// Possible errors from nascent backup generations.
#[derive(Debug, thiserror::Error)]
pub enum NascentError {
    /// Error backing up a backup root.
    #[error("Could not back up a backup root directory: {0}: {1}")]
    BackupRootFailed(PathBuf, crate::fsiter::FsIterError),

    /// Error using a local generation.
    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    /// Error from a GenerationDb.
    #[error(transparent)]
    GenerationDb(#[from] GenerationDbError),

    /// Error from an SQL transaction.
    #[error("SQL transaction error: {0}")]
    Transaction(rusqlite::Error),

    /// Error from committing an SQL transaction.
    #[error("SQL commit error: {0}")]
    Commit(rusqlite::Error),

    /// Error creating a temporary file.
    #[error("Failed to create temporary file: {0}")]
    TempFile(#[from] std::io::Error),
}

impl NascentGeneration {
    /// Create a new nascent generation.
    pub fn create<P>(
        filename: P,
        schema: SchemaVersion,
        checksum_kind: LabelChecksumKind,
    ) -> Result<Self, NascentError>
    where
        P: AsRef<Path>,
    {
        let db = GenerationDb::create(filename.as_ref(), schema, checksum_kind)?;
        Ok(Self { db, fileno: 0 })
    }

    /// Commit any changes, and close the database.
    pub fn close(self) -> Result<(), NascentError> {
        self.db.close().map_err(NascentError::GenerationDb)
    }

    /// How many files are there now in the nascent generation?
    pub fn file_count(&self) -> FileId {
        self.fileno
    }

    /// Insert a new file system entry into a nascent generation.
    pub fn insert(
        &mut self,
        e: FilesystemEntry,
        ids: &[ChunkId],
        reason: Reason,
        is_cachedir_tag: bool,
    ) -> Result<(), NascentError> {
        self.fileno += 1;
        self.db
            .insert(e, self.fileno, ids, reason, is_cachedir_tag)?;
        Ok(())
    }
}

/// A finished generation on the server.
///
/// A generation is finished when it's on the server. It can be
/// fetched so it can be used as a [`LocalGeneration`].
#[derive(Debug, Clone)]
pub struct FinishedGeneration {
    id: GenId,
    ended: String,
}

impl FinishedGeneration {
    /// Create a new finished generation.
    pub fn new(id: &str, ended: &str) -> Self {
        let id = GenId::from_chunk_id(id.parse().unwrap()); // this never fails
        Self {
            id,
            ended: ended.to_string(),
        }
    }

    /// Get the generation's identifier.
    pub fn id(&self) -> &GenId {
        &self.id
    }

    /// When was generation finished?
    pub fn ended(&self) -> &str {
        &self.ended
    }
}

/// A local representation of a finished generation.
///
/// This is for querying an existing generation, and other read-only
/// operations.
pub struct LocalGeneration {
    db: GenerationDb,
}

/// Possible errors from using local generations.
#[derive(Debug, thiserror::Error)]
pub enum LocalGenerationError {
    /// Duplicate file names.
    #[error("Generation has more than one file with the name {0}")]
    TooManyFiles(PathBuf),

    /// No 'meta' table in generation.
    #[error("Generation does not have a 'meta' table")]
    NoMeta,

    /// Local generation uses a schema version that this version of
    /// Obnam isn't compatible with.
    #[error("Backup is not compatible with this version of Obnam: {0}.{1}")]
    Incompatible(VersionComponent, VersionComponent),

    /// Error from generation metadata.
    #[error(transparent)]
    GenerationMeta(#[from] GenerationMetaError),

    /// Error from SQL.
    #[error(transparent)]
    RusqliteError(#[from] rusqlite::Error),

    /// Error from a GenerationDb.
    #[error(transparent)]
    GenerationDb(#[from] GenerationDbError),

    /// Error from a Database.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Error from JSON.
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    /// Error from I/O.
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

/// A backed up file in a local generation.
pub struct BackedUpFile {
    fileno: FileId,
    entry: FilesystemEntry,
    reason: Reason,
}

impl BackedUpFile {
    /// Create a new `BackedUpFile`.
    pub fn new(fileno: FileId, entry: FilesystemEntry, reason: Reason) -> Self {
        Self {
            fileno,
            entry,
            reason,
        }
    }

    /// Return id for file in its local generation.
    pub fn fileno(&self) -> FileId {
        self.fileno
    }

    /// Return file system entry for file.
    pub fn entry(&self) -> &FilesystemEntry {
        &self.entry
    }

    /// Return reason why file is in its local generation.
    pub fn reason(&self) -> Reason {
        self.reason
    }
}

impl LocalGeneration {
    fn new(db: GenerationDb) -> Self {
        Self { db }
    }

    /// Open a local file as a local generation.
    pub fn open<P>(filename: P) -> Result<Self, LocalGenerationError>
    where
        P: AsRef<Path>,
    {
        let db = GenerationDb::open(filename.as_ref())?;
        let gen = Self::new(db);
        Ok(gen)
    }

    /// Return generation metadata for local generation.
    pub fn meta(&self) -> Result<GenerationMeta, LocalGenerationError> {
        let map = self.db.meta()?;
        GenerationMeta::from(map).map_err(LocalGenerationError::GenerationMeta)
    }

    /// How many files are there in the local generation?
    pub fn file_count(&self) -> Result<FileId, LocalGenerationError> {
        Ok(self.db.file_count()?)
    }

    /// Return all files in the local generation.
    pub fn files(
        &self,
    ) -> Result<SqlResults<(FileId, FilesystemEntry, Reason, bool)>, LocalGenerationError> {
        self.db.files().map_err(LocalGenerationError::GenerationDb)
    }

    /// Return ids for all chunks in local generation.
    pub fn chunkids(&self, fileid: FileId) -> Result<SqlResults<ChunkId>, LocalGenerationError> {
        self.db
            .chunkids(fileid)
            .map_err(LocalGenerationError::GenerationDb)
    }

    /// Return entry for a file, given its pathname.
    pub fn get_file(
        &self,
        filename: &Path,
    ) -> Result<Option<FilesystemEntry>, LocalGenerationError> {
        self.db
            .get_file(filename)
            .map_err(LocalGenerationError::GenerationDb)
    }

    /// Get the id in the local generation of a file, given its pathname.
    pub fn get_fileno(&self, filename: &Path) -> Result<Option<FileId>, LocalGenerationError> {
        self.db
            .get_fileno(filename)
            .map_err(LocalGenerationError::GenerationDb)
    }

    /// Does a pathname refer to a cache directory?
    pub fn is_cachedir_tag(&self, filename: &Path) -> Result<bool, LocalGenerationError> {
        self.db
            .is_cachedir_tag(filename)
            .map_err(LocalGenerationError::GenerationDb)
    }
}

#[cfg(test)]
mod test {
    use super::{LabelChecksumKind, LocalGeneration, NascentGeneration, SchemaVersion};
    use tempfile::NamedTempFile;

    #[test]
    fn empty() {
        let filename = NamedTempFile::new().unwrap().path().to_path_buf();
        let schema = SchemaVersion::new(0, 0);
        {
            let mut _gen =
                NascentGeneration::create(&filename, schema, LabelChecksumKind::Sha256).unwrap();
            // _gen is dropped here; the connection is close; the file
            // should not be removed.
        }
        assert!(filename.exists());
    }

    // FIXME: This is way too complicated a test function. It should
    // be simplified, possibly by re-thinking the abstractions of the
    // code it calls.
    #[test]
    fn remembers_cachedir_tags() {
        use crate::{
            backup_reason::Reason, backup_run::FsEntryBackupOutcome, fsentry::FilesystemEntry,
        };
        use std::{fs::metadata, path::Path};

        // Create a `Metadata` structure to pass to other functions (we don't care about the
        // contents)
        let src_file = NamedTempFile::new().unwrap();
        let metadata = metadata(src_file.path()).unwrap();

        let dbfile = NamedTempFile::new().unwrap().path().to_path_buf();

        let nontag_path1 = Path::new("/nontag1");
        let nontag_path2 = Path::new("/dir/nontag2");
        let tag_path1 = Path::new("/a_tag");
        let tag_path2 = Path::new("/another_dir/a_tag");

        let schema = SchemaVersion::new(0, 0);
        let mut gen =
            NascentGeneration::create(&dbfile, schema, LabelChecksumKind::Sha256).unwrap();
        let mut cache = users::UsersCache::new();

        gen.insert(
            FilesystemEntry::from_metadata(nontag_path1, &metadata, &mut cache).unwrap(),
            &[],
            Reason::IsNew,
            false,
        )
        .unwrap();
        gen.insert(
            FilesystemEntry::from_metadata(tag_path1, &metadata, &mut cache).unwrap(),
            &[],
            Reason::IsNew,
            true,
        )
        .unwrap();

        let entries = vec![
            FsEntryBackupOutcome {
                entry: FilesystemEntry::from_metadata(nontag_path2, &metadata, &mut cache).unwrap(),
                ids: vec![],
                reason: Reason::IsNew,
                is_cachedir_tag: false,
            },
            FsEntryBackupOutcome {
                entry: FilesystemEntry::from_metadata(tag_path2, &metadata, &mut cache).unwrap(),
                ids: vec![],
                reason: Reason::IsNew,
                is_cachedir_tag: true,
            },
        ];

        for o in entries {
            gen.insert(o.entry, &o.ids, o.reason, o.is_cachedir_tag)
                .unwrap();
        }

        gen.close().unwrap();

        let gen = LocalGeneration::open(dbfile).unwrap();
        assert!(!gen.is_cachedir_tag(nontag_path1).unwrap());
        assert!(!gen.is_cachedir_tag(nontag_path2).unwrap());
        assert!(gen.is_cachedir_tag(tag_path1).unwrap());
        assert!(gen.is_cachedir_tag(tag_path2).unwrap());

        // Nonexistent files are not cachedir tags
        assert!(!gen.is_cachedir_tag(Path::new("/hello/world")).unwrap());
        assert!(!gen
            .is_cachedir_tag(Path::new("/different path/to/another file.txt"))
            .unwrap());
    }
}

//! Backup generations of various kinds.

use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::db::{DatabaseError, SqlResults};
use crate::dbgen::{FileId, GenerationDb, GenerationDbError, SCHEMA_MAJOR, SCHEMA_MINOR};
use crate::fsentry::FilesystemEntry;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// An identifier for a generation.
#[derive(Debug, Clone)]
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
    pub fn create<P>(filename: P) -> Result<Self, NascentError>
    where
        P: AsRef<Path>,
    {
        let db = GenerationDb::create(filename.as_ref())?;
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

    /// Missing from from 'meta' table.
    #[error("Generation 'meta' table does not have a row {0}")]
    NoMetaKey(String),

    /// Bad data in 'meta' table.
    #[error("Generation 'meta' row {0} has badly formed integer: {1}")]
    BadMetaInteger(String, std::num::ParseIntError),

    /// Local generation uses a schema version that this version of
    /// Obnam isn't compatible with.
    #[error("Backup is not compatible with this version of Obnam: {0}.{1}")]
    Incompatible(u32, u32),

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
        let schema = gen.meta()?.schema_version();
        let our_schema = SchemaVersion::new(SCHEMA_MAJOR, SCHEMA_MINOR);
        if !our_schema.is_compatible_with(&schema) {
            return Err(LocalGenerationError::Incompatible(
                schema.major,
                schema.minor,
            ));
        }
        Ok(gen)
    }

    /// Return generation metadata for local generation.
    pub fn meta(&self) -> Result<GenMeta, LocalGenerationError> {
        let map = self.db.meta()?;
        GenMeta::from(map)
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

/// Metadata about the local generation.
#[derive(Debug, Serialize)]
pub struct GenMeta {
    schema_version: SchemaVersion,
    extras: HashMap<String, String>,
}

impl GenMeta {
    /// Create from a hash map.
    fn from(mut map: HashMap<String, String>) -> Result<Self, LocalGenerationError> {
        let major: u32 = metaint(&mut map, "schema_version_major")?;
        let minor: u32 = metaint(&mut map, "schema_version_minor")?;
        Ok(Self {
            schema_version: SchemaVersion::new(major, minor),
            extras: map,
        })
    }

    /// Return schema version of local generation.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }
}

fn metastr(map: &mut HashMap<String, String>, key: &str) -> Result<String, LocalGenerationError> {
    if let Some(v) = map.remove(key) {
        Ok(v)
    } else {
        Err(LocalGenerationError::NoMetaKey(key.to_string()))
    }
}

fn metaint(map: &mut HashMap<String, String>, key: &str) -> Result<u32, LocalGenerationError> {
    let v = metastr(map, key)?;
    let v = v
        .parse()
        .map_err(|err| LocalGenerationError::BadMetaInteger(key.to_string(), err))?;
    Ok(v)
}

/// Schema version of the database storing the generation.
///
/// An Obnam client can restore a generation using schema version
/// (x,y), if the client supports a schema version (x,z). If z < y,
/// the client knows it may not be able to the generation faithfully,
/// and should warn the user about this. If z >= y, the client knows
/// it can restore the generation faithfully. If the client does not
/// support any schema version x, it knows it can't restore the backup
/// at all.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SchemaVersion {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
}

impl SchemaVersion {
    fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Is this schema version compatible with another schema version?
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}

#[cfg(test)]
mod test_schema {
    use super::*;

    #[test]
    fn compatible_with_self() {
        let v = SchemaVersion::new(1, 2);
        assert!(v.is_compatible_with(&v));
    }

    #[test]
    fn compatible_with_older_minor_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(1, 3);
        assert!(new.is_compatible_with(&old));
    }

    #[test]
    fn not_compatible_with_newer_minor_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(1, 3);
        assert!(!old.is_compatible_with(&new));
    }

    #[test]
    fn not_compatible_with_older_major_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(2, 0);
        assert!(!new.is_compatible_with(&old));
    }

    #[test]
    fn not_compatible_with_newer_major_version() {
        let old = SchemaVersion::new(1, 2);
        let new = SchemaVersion::new(2, 0);
        assert!(!old.is_compatible_with(&new));
    }
}

#[cfg(test)]
mod test {
    use super::{LocalGeneration, NascentGeneration};
    use tempfile::NamedTempFile;

    #[test]
    fn empty() {
        let filename = NamedTempFile::new().unwrap().path().to_path_buf();
        {
            let mut _gen = NascentGeneration::create(&filename).unwrap();
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

        let mut gen = NascentGeneration::create(&dbfile).unwrap();
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

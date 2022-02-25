//! Backup generations of various kinds.

use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::dbgen::{FileId, SCHEMA_MAJOR, SCHEMA_MINOR};
use crate::fsentry::FilesystemEntry;
use rusqlite::Connection;
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
    conn: Connection,
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
        let conn = sql::create_db(filename.as_ref())?;
        Ok(Self { conn, fileno: 0 })
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
        let t = self.conn.transaction().map_err(NascentError::Transaction)?;
        self.fileno += 1;
        sql::insert_one(&t, e, self.fileno, ids, reason, is_cachedir_tag)?;
        t.commit().map_err(NascentError::Commit)?;
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
    conn: Connection,
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
    pub fn new(fileno: FileId, entry: FilesystemEntry, reason: &str) -> Self {
        let reason = Reason::from(reason);
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
    fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Open a local file as a local generation.
    pub fn open<P>(filename: P) -> Result<Self, LocalGenerationError>
    where
        P: AsRef<Path>,
    {
        let conn = sql::open_db(filename.as_ref())?;
        let gen = Self::new(conn);
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
        let map = sql::meta(&self.conn)?;
        GenMeta::from(map)
    }

    /// How many files are there in the local generation?
    pub fn file_count(&self) -> Result<FileId, LocalGenerationError> {
        sql::file_count(&self.conn)
    }

    /// Return all files in the local generation.
    pub fn files(&self) -> Result<sql::SqlResults<BackedUpFile>, LocalGenerationError> {
        sql::files(&self.conn)
    }

    /// Return ids for all chunks in local generation.
    pub fn chunkids(
        &self,
        fileno: FileId,
    ) -> Result<sql::SqlResults<ChunkId>, LocalGenerationError> {
        sql::chunkids(&self.conn, fileno)
    }

    /// Return entry for a file, given its pathname.
    pub fn get_file(
        &self,
        filename: &Path,
    ) -> Result<Option<FilesystemEntry>, LocalGenerationError> {
        sql::get_file(&self.conn, filename)
    }

    /// Get the id in the local generation of a file, given its pathname.
    pub fn get_fileno(&self, filename: &Path) -> Result<Option<FileId>, LocalGenerationError> {
        sql::get_fileno(&self.conn, filename)
    }

    /// Does a pathname refer to a cache directory?
    pub fn is_cachedir_tag(&self, filename: &Path) -> Result<bool, LocalGenerationError> {
        sql::is_cachedir_tag(&self.conn, filename)
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

mod sql {
    use super::BackedUpFile;
    use super::FileId;
    use super::LocalGenerationError;
    use crate::backup_reason::Reason;
    use crate::chunkid::ChunkId;
    use crate::fsentry::FilesystemEntry;
    use crate::generation::SCHEMA_MAJOR;
    use crate::generation::SCHEMA_MINOR;
    use log::debug;
    use rusqlite::{params, Connection, OpenFlags, Row, Statement, Transaction};
    use std::collections::HashMap;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// Create a new database in a file.
    pub fn create_db(filename: &Path) -> Result<Connection, LocalGenerationError> {
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute("CREATE TABLE meta (key TEXT, value TEXT)", params![])?;
        init_meta(&conn)?;
        conn.execute(
            "CREATE TABLE files (fileno INTEGER PRIMARY KEY, filename BLOB, json TEXT, reason TEXT, is_cachedir_tag BOOLEAN)",
            params![],
        )?;
        conn.execute(
            "CREATE TABLE chunks (fileno INTEGER, chunkid TEXT)",
            params![],
        )?;
        conn.execute("CREATE INDEX filenames ON files (filename)", params![])?;
        conn.execute("CREATE INDEX filenos ON chunks (fileno)", params![])?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(conn)
    }

    fn init_meta(conn: &Connection) -> Result<(), LocalGenerationError> {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            params!["schema_version_major", SCHEMA_MAJOR],
        )?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            params!["schema_version_minor", SCHEMA_MINOR],
        )?;
        Ok(())
    }

    /// Open an existing database in a file.
    pub fn open_db(filename: &Path) -> Result<Connection, LocalGenerationError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(conn)
    }

    /// Return generation metadata from a database.
    pub fn meta(conn: &Connection) -> Result<HashMap<String, String>, LocalGenerationError> {
        let mut stmt = conn.prepare("SELECT key, value FROM meta")?;
        let iter = stmt.query_map(params![], row_to_key_value)?;
        let mut map = HashMap::new();
        for r in iter {
            let (key, value) = r?;
            map.insert(key, value);
        }
        Ok(map)
    }

    fn row_to_key_value(row: &Row) -> rusqlite::Result<(String, String)> {
        let key: String = row.get("key")?;
        let value: String = row.get("value")?;
        Ok((key, value))
    }

    /// Insert one file system entry into the database.
    pub fn insert_one(
        t: &Transaction,
        e: FilesystemEntry,
        fileno: FileId,
        ids: &[ChunkId],
        reason: Reason,
        is_cachedir_tag: bool,
    ) -> Result<(), LocalGenerationError> {
        let json = serde_json::to_string(&e)?;
        t.execute(
            "INSERT INTO files (fileno, filename, json, reason, is_cachedir_tag) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![fileno, path_into_blob(&e.pathbuf()), &json, reason, is_cachedir_tag,],
        )?;
        for id in ids {
            t.execute(
                "INSERT INTO chunks (fileno, chunkid) VALUES (?1, ?2)",
                params![fileno, id],
            )?;
        }
        Ok(())
    }

    fn path_into_blob(path: &Path) -> Vec<u8> {
        path.as_os_str().as_bytes().to_vec()
    }

    /// Parse an SQL query result row.
    pub fn row_to_entry(row: &Row) -> rusqlite::Result<(FileId, String, String)> {
        let fileno: FileId = row.get("fileno")?;
        let json: String = row.get("json")?;
        let reason: String = row.get("reason")?;
        Ok((fileno, json, reason))
    }

    /// Count number of file system entries.
    pub fn file_count(conn: &Connection) -> Result<FileId, LocalGenerationError> {
        let mut stmt = conn.prepare("SELECT count(*) FROM files")?;
        let mut iter = stmt.query_map(params![], |row| row.get(0))?;
        let count = iter.next().expect("SQL count result (1)");
        let count = count?;
        Ok(count)
    }

    // A pointer to a "fallible iterator" over values of type `T`, which is to say it's an iterator
    // over values of type `Result<T, LocalGenerationError>`. The iterator is only valid for the
    // lifetime 'stmt.
    //
    // The fact that it's a pointer (`Box<dyn ...>`) means we don't care what the actual type of
    // the iterator is, and who produces it.
    type SqlResultsIterator<'stmt, T> =
        Box<dyn Iterator<Item = Result<T, LocalGenerationError>> + 'stmt>;

    // A pointer to a function which, when called on a prepared SQLite statement, would create
    // a "fallible iterator" over values of type `ItemT`. (See above for an explanation of what
    // a "fallible iterator" is.)
    //
    // The iterator is only valid for the lifetime of the associated SQLite statement; we
    // call this lifetime 'stmt, and use it both both on the reference and the returned Now.
    //
    // we iterator're in a pickle: all named lifetimes have to be declared _somewhere_, but we can't add
    // 'stmt to the signature of `CreateIterFn` because then we'll have to specify it when we
    // define the function. Obviously, at that point we won't yet have a `Statement`, and thus we
    // would have no idea what its lifetime is going to be. So we can't put the 'stmt lifetime into
    // the signature of `CreateIterFn`.
    //
    // That's what `for<'stmt>` is for. This is a so-called ["higher-rank trait bound"][hrtb], and
    // it enables us to say that a function is valid for *some* lifetime 'stmt that we pass into it
    // at the call site. It lets Rust continue to track lifetimes even though `CreateIterFn`
    // interferes by "hiding" the 'stmt lifetime from its signature.
    //
    // [hrtb]: https://doc.rust-lang.org/nomicon/hrtb.html
    type CreateIterFn<'conn, ItemT> = Box<
        dyn for<'stmt> Fn(
            &'stmt mut Statement<'conn>,
        )
            -> Result<SqlResultsIterator<'stmt, ItemT>, LocalGenerationError>,
    >;

    /// Iterator of SQL results.
    pub struct SqlResults<'conn, ItemT> {
        stmt: Statement<'conn>,
        create_iter: CreateIterFn<'conn, ItemT>,
    }

    impl<'conn, ItemT> SqlResults<'conn, ItemT> {
        fn new(
            conn: &'conn Connection,
            statement: &str,
            create_iter: CreateIterFn<'conn, ItemT>,
        ) -> Result<Self, LocalGenerationError> {
            let stmt = conn.prepare(statement)?;
            Ok(Self { stmt, create_iter })
        }

        /// Create an iterator over results.
        pub fn iter(&'_ mut self) -> Result<SqlResultsIterator<'_, ItemT>, LocalGenerationError> {
            (self.create_iter)(&mut self.stmt)
        }
    }

    /// Return all file system entries in database.
    pub fn files(conn: &Connection) -> Result<SqlResults<BackedUpFile>, LocalGenerationError> {
        SqlResults::new(
            conn,
            "SELECT * FROM files",
            Box::new(|stmt| {
                let iter = stmt.query_map(params![], row_to_entry)?;
                let iter = iter.map(|x| match x {
                    Ok((fileno, json, reason)) => serde_json::from_str(&json)
                        .map(|entry| BackedUpFile::new(fileno, entry, &reason))
                        .map_err(|e| e.into()),
                    Err(e) => Err(e.into()),
                });
                Ok(Box::new(iter))
            }),
        )
    }

    /// Return all chunk ids in database.
    pub fn chunkids(
        conn: &Connection,
        fileno: FileId,
    ) -> Result<SqlResults<ChunkId>, LocalGenerationError> {
        SqlResults::new(
            conn,
            "SELECT chunkid FROM chunks WHERE fileno = ?1",
            Box::new(move |stmt| {
                let iter = stmt.query_map(params![fileno], |row| row.get(0))?;
                let iter = iter.map(|x| {
                    let fileno: String = x?;
                    Ok(ChunkId::from(&fileno))
                });
                Ok(Box::new(iter))
            }),
        )
    }

    /// Get a file's information given its path.
    pub fn get_file(
        conn: &Connection,
        filename: &Path,
    ) -> Result<Option<FilesystemEntry>, LocalGenerationError> {
        match get_file_and_fileno(conn, filename)? {
            None => Ok(None),
            Some((_, e, _)) => Ok(Some(e)),
        }
    }

    /// Get a file's information given it's id in the database.
    pub fn get_fileno(
        conn: &Connection,
        filename: &Path,
    ) -> Result<Option<FileId>, LocalGenerationError> {
        match get_file_and_fileno(conn, filename)? {
            None => Ok(None),
            Some((id, _, _)) => Ok(Some(id)),
        }
    }

    fn get_file_and_fileno(
        conn: &Connection,
        filename: &Path,
    ) -> Result<Option<(FileId, FilesystemEntry, String)>, LocalGenerationError> {
        let mut stmt = conn.prepare("SELECT * FROM files WHERE filename = ?1")?;
        let mut iter = stmt.query_map(params![path_into_blob(filename)], row_to_entry)?;
        match iter.next() {
            None => Ok(None),
            Some(Err(e)) => {
                debug!("database lookup error: {}", e);
                Err(e.into())
            }
            Some(Ok((fileno, json, reason))) => {
                let entry = serde_json::from_str(&json)?;
                if iter.next() == None {
                    Ok(Some((fileno, entry, reason)))
                } else {
                    debug!("too many files in file lookup");
                    Err(LocalGenerationError::TooManyFiles(filename.to_path_buf()))
                }
            }
        }
    }

    /// Does a path refer to a cache directory?
    pub fn is_cachedir_tag(
        conn: &Connection,
        filename: &Path,
    ) -> Result<bool, LocalGenerationError> {
        let mut stmt = conn.prepare("SELECT is_cachedir_tag FROM files WHERE filename = ?1")?;
        let mut iter = stmt.query_map(params![path_into_blob(filename)], |row| row.get(0))?;
        match iter.next() {
            // File is missing, so it's definitely not a CACHEDIR.TAG
            None => Ok(false),
            Some(result) => Ok(result?),
        }
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
        use std::{fs::metadata, mem::drop, path::Path};

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

        gen.insert(
            FilesystemEntry::from_metadata(nontag_path1, &metadata).unwrap(),
            &[],
            Reason::IsNew,
            false,
        )
        .unwrap();
        gen.insert(
            FilesystemEntry::from_metadata(tag_path1, &metadata).unwrap(),
            &[],
            Reason::IsNew,
            true,
        )
        .unwrap();

        let entries = vec![
            FsEntryBackupOutcome {
                entry: FilesystemEntry::from_metadata(nontag_path2, &metadata).unwrap(),
                ids: vec![],
                reason: Reason::IsNew,
                is_cachedir_tag: false,
            },
            FsEntryBackupOutcome {
                entry: FilesystemEntry::from_metadata(tag_path2, &metadata).unwrap(),
                ids: vec![],
                reason: Reason::IsNew,
                is_cachedir_tag: true,
            },
        ];

        for o in entries {
            gen.insert(o.entry, &o.ids, o.reason, o.is_cachedir_tag)
                .unwrap();
        }

        drop(gen);

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

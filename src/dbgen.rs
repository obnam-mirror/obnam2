//! Database abstraction for generations.

use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::db::{Column, Database, DatabaseError, SqlResults, Table, Value};
use crate::fsentry::FilesystemEntry;
use log::error;
use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Current generation database schema major version.
pub const SCHEMA_MAJOR: u32 = 0;

/// Current generation database schema minor version.
pub const SCHEMA_MINOR: u32 = 0;

/// An identifier for a file in a generation.
pub type FileId = u64;

/// Possible errors from using generation databases.
#[derive(Debug, thiserror::Error)]
pub enum GenerationDbError {
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

    /// Error from a database
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Error from JSON.
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    /// Error from I/O.
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

/// A database representing a backup generation.
pub struct GenerationDb {
    created: bool,
    db: Database,
    meta: Table,
    files: Table,
    chunks: Table,
}

impl GenerationDb {
    /// Create a new generation database in read/write mode.
    pub fn create<P: AsRef<Path>>(filename: P) -> Result<Self, GenerationDbError> {
        let db = Database::create(filename.as_ref())?;
        let mut moi = Self::new(db);
        moi.created = true;
        moi.create_tables()?;
        Ok(moi)
    }

    /// Open an existing generation database in read-only mode.
    pub fn open<P: AsRef<Path>>(filename: P) -> Result<Self, GenerationDbError> {
        let db = Database::open(filename.as_ref())?;
        Ok(Self::new(db))
    }

    fn new(db: Database) -> Self {
        let meta = Table::new("meta")
            .column(Column::text("key"))
            .column(Column::text("value"))
            .build();
        let files = Table::new("files")
            .column(Column::primary_key("fileno")) // FIXME: rename to fileid
            .column(Column::blob("filename"))
            .column(Column::text("json"))
            .column(Column::text("reason"))
            .column(Column::bool("is_cachedir_tag"))
            .build();
        let chunks = Table::new("chunks")
            .column(Column::int("fileno")) // FIXME: rename to fileid
            .column(Column::text("chunkid"))
            .build();

        Self {
            created: false,
            db,
            meta,
            files,
            chunks,
        }
    }

    fn create_tables(&mut self) -> Result<(), GenerationDbError> {
        self.db.create_table(&self.meta)?;
        self.db.create_table(&self.files)?;
        self.db.create_table(&self.chunks)?;

        self.db.insert(
            &self.meta,
            &[
                Value::text("key", "schema_version_major"),
                Value::text("value", &format!("{}", SCHEMA_MAJOR)),
            ],
        )?;
        self.db.insert(
            &self.meta,
            &[
                Value::text("key", "schema_version_minor"),
                Value::text("value", &format!("{}", SCHEMA_MINOR)),
            ],
        )?;

        Ok(())
    }

    /// Close a database, commit any changes.
    pub fn close(self) -> Result<(), GenerationDbError> {
        if self.created {
            self.db
                .create_index("filenames_idx", &self.files, "filename")?;
            self.db.create_index("fileid_idx", &self.chunks, "fileno")?;
        }
        self.db.close().map_err(GenerationDbError::Database)
    }

    /// Return contents of "meta" table as a HashMap.
    pub fn meta(&self) -> Result<HashMap<String, String>, GenerationDbError> {
        let mut map = HashMap::new();
        let mut iter = self.db.all_rows(&self.meta, &row_to_kv)?;
        for kv in iter.iter()? {
            let (key, value) = kv?;
            map.insert(key, value);
        }
        Ok(map)
    }

    /// Insert a file system entry into the database.
    pub fn insert(
        &mut self,
        e: FilesystemEntry,
        fileid: FileId,
        ids: &[ChunkId],
        reason: Reason,
        is_cachedir_tag: bool,
    ) -> Result<(), GenerationDbError> {
        let json = serde_json::to_string(&e)?;
        self.db.insert(
            &self.files,
            &[
                Value::primary_key("fileno", fileid),
                Value::blob("filename", &path_into_blob(&e.pathbuf())),
                Value::text("json", &json),
                Value::text("reason", &format!("{}", reason)),
                Value::bool("is_cachedir_tag", is_cachedir_tag),
            ],
        )?;
        for id in ids {
            self.db.insert(
                &self.chunks,
                &[
                    Value::int("fileno", fileid),
                    Value::text("chunkid", &format!("{}", id)),
                ],
            )?;
        }
        Ok(())
    }

    /// Count number of file system entries.
    pub fn file_count(&self) -> Result<FileId, GenerationDbError> {
        // FIXME: this needs to be done use "SELECT count(*) FROM
        // files", but the Database abstraction doesn't support that
        // yet.
        let mut iter = self.db.all_rows(&self.files, &row_to_entry)?;
        let mut count = 0;
        for _ in iter.iter()? {
            count += 1;
        }
        Ok(count)
    }

    /// Does a path refer to a cache directory?
    pub fn is_cachedir_tag(&self, filename: &Path) -> Result<bool, GenerationDbError> {
        let filename = path_into_blob(filename);
        let value = Value::blob("filename", &filename);
        let mut rows = self.db.some_rows(&self.files, &value, &row_to_entry)?;
        let mut iter = rows.iter()?;

        if let Some(row) = iter.next() {
            if iter.next().is_some() {
                Ok(false)
            } else {
                let (_, _, _, is_cachedir_tag) = row?;
                Ok(is_cachedir_tag)
            }
        } else {
            Ok(false)
        }
    }

    /// Return all chunk ids in database.
    pub fn chunkids(&self, fileid: FileId) -> Result<SqlResults<ChunkId>, GenerationDbError> {
        let fileid = Value::int("fileno", fileid);
        Ok(self.db.some_rows(&self.chunks, &fileid, &row_to_chunkid)?)
    }

    /// Return all chunk ids in database.
    pub fn files(
        &self,
    ) -> Result<SqlResults<(FileId, FilesystemEntry, Reason, bool)>, GenerationDbError> {
        Ok(self.db.all_rows(&self.files, &row_to_fsentry)?)
    }

    /// Get a file's information given its path.
    pub fn get_file(&self, filename: &Path) -> Result<Option<FilesystemEntry>, GenerationDbError> {
        match self.get_file_and_fileno(filename)? {
            None => Ok(None),
            Some((_, e, _)) => Ok(Some(e)),
        }
    }

    /// Get a file's information given it's id in the database.
    pub fn get_fileno(&self, filename: &Path) -> Result<Option<FileId>, GenerationDbError> {
        match self.get_file_and_fileno(filename)? {
            None => Ok(None),
            Some((id, _, _)) => Ok(Some(id)),
        }
    }

    fn get_file_and_fileno(
        &self,
        filename: &Path,
    ) -> Result<Option<(FileId, FilesystemEntry, String)>, GenerationDbError> {
        let filename_bytes = path_into_blob(filename);
        let value = Value::blob("filename", &filename_bytes);
        let mut rows = self.db.some_rows(&self.files, &value, &row_to_entry)?;
        let mut iter = rows.iter()?;

        if let Some(row) = iter.next() {
            if iter.next().is_some() {
                error!("too many files in file lookup");
                Err(GenerationDbError::TooManyFiles(filename.to_path_buf()))
            } else {
                let (fileid, ref json, ref reason, _) = row?;
                let entry = serde_json::from_str(json)?;
                Ok(Some((fileid, entry, reason.to_string())))
            }
        } else {
            Ok(None)
        }
    }
}

fn row_to_kv(row: &rusqlite::Row) -> rusqlite::Result<(String, String)> {
    let k = row.get("key")?;
    let v = row.get("value")?;
    Ok((k, v))
}

fn path_into_blob(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<(FileId, String, String, bool)> {
    let fileno: FileId = row.get("fileno")?;
    let json: String = row.get("json")?;
    let reason: String = row.get("reason")?;
    let is_cachedir_tag: bool = row.get("is_cachedir_tag")?;
    Ok((fileno, json, reason, is_cachedir_tag))
}

fn row_to_fsentry(
    row: &rusqlite::Row,
) -> rusqlite::Result<(FileId, FilesystemEntry, Reason, bool)> {
    let fileno: FileId = row.get("fileno")?;
    let json: String = row.get("json")?;
    let entry = serde_json::from_str(&json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let reason: String = row.get("reason")?;
    let reason = Reason::from(&reason);
    let is_cachedir_tag: bool = row.get("is_cachedir_tag")?;
    Ok((fileno, entry, reason, is_cachedir_tag))
}

fn row_to_chunkid(row: &rusqlite::Row) -> rusqlite::Result<ChunkId> {
    let chunkid: String = row.get("chunkid")?;
    let chunkid = ChunkId::recreate(&chunkid);
    Ok(chunkid)
}

#[cfg(test)]
mod test {
    use super::Database;
    use tempfile::tempdir;

    #[test]
    fn opens_previously_created_db() {
        let dir = tempdir().unwrap();
        let filename = dir.path().join("test.db");
        Database::create(&filename).unwrap();
        assert!(Database::open(&filename).is_ok());
    }
}

use crate::backup_reason::Reason;
use crate::chunkid::ChunkId;
use crate::fsentry::FilesystemEntry;
use rusqlite::Connection;
use std::path::Path;

/// An identifier for a file in a generation.
type FileId = i64;

/// A nascent backup generation.
///
/// A nascent generation is one that is being prepared. It isn't
/// finished yet, and it's not actually on the server until the upload
/// of its generation chunk.
pub struct NascentGeneration {
    conn: Connection,
    fileno: FileId,
}

impl NascentGeneration {
    pub fn create<P>(filename: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let conn = sql::create_db(filename.as_ref())?;
        Ok(Self { conn, fileno: 0 })
    }

    pub fn file_count(&self) -> FileId {
        self.fileno
    }

    pub fn insert(
        &mut self,
        e: FilesystemEntry,
        ids: &[ChunkId],
        reason: Reason,
    ) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        self.fileno += 1;
        sql::insert_one(&t, e, self.fileno, ids, reason)?;
        t.commit()?;
        Ok(())
    }

    pub fn insert_iter<'a>(
        &mut self,
        entries: impl Iterator<Item = anyhow::Result<(FilesystemEntry, Vec<ChunkId>, Reason)>>,
    ) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        for r in entries {
            let (e, ids, reason) = r?;
            self.fileno += 1;
            sql::insert_one(&t, e, self.fileno, &ids[..], reason)?;
        }
        t.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::NascentGeneration;
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
}

/// A finished generation.
///
/// A generation is finished when it's on the server. It can be restored.
#[derive(Debug, Clone)]
pub struct FinishedGeneration {
    id: ChunkId,
    ended: String,
}

impl FinishedGeneration {
    pub fn new(id: &str, ended: &str) -> Self {
        let id = id.parse().unwrap(); // this never fails
        Self {
            id,
            ended: ended.to_string(),
        }
    }

    pub fn id(&self) -> ChunkId {
        self.id.clone()
    }

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

impl LocalGeneration {
    pub fn open<P>(filename: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let conn = sql::open_db(filename.as_ref())?;
        Ok(Self { conn })
    }

    pub fn file_count(&self) -> anyhow::Result<i64> {
        Ok(sql::file_count(&self.conn)?)
    }

    pub fn files(&self) -> anyhow::Result<Vec<(FileId, FilesystemEntry, String)>> {
        Ok(sql::files(&self.conn)?)
    }

    pub fn chunkids(&self, fileno: FileId) -> anyhow::Result<Vec<ChunkId>> {
        Ok(sql::chunkids(&self.conn, fileno)?)
    }

    pub fn get_file(&self, filename: &Path) -> anyhow::Result<Option<FilesystemEntry>> {
        Ok(sql::get_file(&self.conn, filename)?)
    }

    pub fn get_fileno(&self, filename: &Path) -> anyhow::Result<Option<FileId>> {
        Ok(sql::get_fileno(&self.conn, filename)?)
    }
}

mod sql {
    use super::FileId;
    use crate::backup_reason::Reason;
    use crate::chunkid::ChunkId;
    use crate::error::ObnamError;
    use crate::fsentry::FilesystemEntry;
    use rusqlite::{params, Connection, OpenFlags, Row, Transaction};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    pub fn create_db(filename: &Path) -> anyhow::Result<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute(
            "CREATE TABLE files (fileno INTEGER PRIMARY KEY, filename BLOB, json TEXT, reason TEXT)",
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

    pub fn open_db(filename: &Path) -> anyhow::Result<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(conn)
    }

    pub fn insert_one(
        t: &Transaction,
        e: FilesystemEntry,
        fileno: FileId,
        ids: &[ChunkId],
        reason: Reason,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_string(&e)?;
        t.execute(
            "INSERT INTO files (fileno, filename, json, reason) VALUES (?1, ?2, ?3, ?4)",
            params![fileno, path_into_blob(&e.pathbuf()), &json, reason,],
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

    pub fn row_to_entry(row: &Row) -> rusqlite::Result<(FileId, String, String)> {
        let fileno: FileId = row.get(row.column_index("fileno")?)?;
        let json: String = row.get(row.column_index("json")?)?;
        let reason: String = row.get(row.column_index("reason")?)?;
        Ok((fileno, json, reason))
    }

    pub fn file_count(conn: &Connection) -> anyhow::Result<FileId> {
        let mut stmt = conn.prepare("SELECT count(*) FROM files")?;
        let mut iter = stmt.query_map(params![], |row| row.get(0))?;
        let count = iter.next().expect("SQL count result (1)");
        let count = count?;
        Ok(count)
    }

    pub fn files(conn: &Connection) -> anyhow::Result<Vec<(FileId, FilesystemEntry, String)>> {
        let mut stmt = conn.prepare("SELECT * FROM files")?;
        let iter = stmt.query_map(params![], |row| row_to_entry(row))?;
        let mut files: Vec<(FileId, FilesystemEntry, String)> = vec![];
        for x in iter {
            let (fileno, json, reason) = x?;
            let entry = serde_json::from_str(&json)?;
            files.push((fileno, entry, reason));
        }
        Ok(files)
    }

    pub fn chunkids(conn: &Connection, fileno: FileId) -> anyhow::Result<Vec<ChunkId>> {
        let mut stmt = conn.prepare("SELECT chunkid FROM chunks WHERE fileno = ?1")?;
        let iter = stmt.query_map(params![fileno], |row| Ok(row.get(0)?))?;
        let mut ids: Vec<ChunkId> = vec![];
        for x in iter {
            let fileno: String = x?;
            ids.push(ChunkId::from(&fileno));
        }
        Ok(ids)
    }

    pub fn get_file(conn: &Connection, filename: &Path) -> anyhow::Result<Option<FilesystemEntry>> {
        match get_file_and_fileno(conn, filename)? {
            None => Ok(None),
            Some((_, e, _)) => Ok(Some(e)),
        }
    }

    pub fn get_fileno(conn: &Connection, filename: &Path) -> anyhow::Result<Option<FileId>> {
        match get_file_and_fileno(conn, filename)? {
            None => Ok(None),
            Some((id, _, _)) => Ok(Some(id)),
        }
    }

    fn get_file_and_fileno(
        conn: &Connection,
        filename: &Path,
    ) -> anyhow::Result<Option<(FileId, FilesystemEntry, String)>> {
        let mut stmt = conn.prepare("SELECT * FROM files WHERE filename = ?1")?;
        let mut iter =
            stmt.query_map(params![path_into_blob(filename)], |row| row_to_entry(row))?;
        match iter.next() {
            None => Ok(None),
            Some(Err(e)) => Err(e.into()),
            Some(Ok((fileno, json, reason))) => {
                let entry = serde_json::from_str(&json)?;
                if iter.next() == None {
                    Ok(Some((fileno, entry, reason)))
                } else {
                    Err(ObnamError::TooManyFiles(filename.to_path_buf()).into())
                }
            }
        }
    }
}

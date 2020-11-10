use crate::fsentry::{FilesystemEntry, FilesystemKind};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
//use crate::fsiter::FsIterator;
use crate::chunkid::ChunkId;
use rusqlite::{params, Connection, OpenFlags, Row, Transaction};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// A backup generation.
pub struct Generation {
    conn: Connection,
    fileno: u64,
}

impl Generation {
    pub fn create<P>(filename: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute(
            "CREATE TABLE files (fileid INTEGER PRIMARY KEY, path BLOB, kind INTEGER, len INTEGER)",
            params![],
        )?;
        conn.execute(
            "CREATE TABLE chunks (fileid INTEGER, chunkid TEXT)",
            params![],
        )?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(Self { conn, fileno: 0 })
    }

    pub fn open<P>(filename: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(Self { conn, fileno: 0 })
    }

    pub fn insert(&mut self, e: FilesystemEntry, ids: &[ChunkId]) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        insert_one(&t, e, self.fileno, ids)?;
        self.fileno += 1;
        t.commit()?;
        Ok(())
    }

    pub fn insert_iter(
        &mut self,
        entries: impl Iterator<Item = anyhow::Result<(FilesystemEntry, Vec<ChunkId>)>>,
    ) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        for r in entries {
            let (e, ids) = r?;
            insert_one(&t, e, self.fileno, &ids[..])?;
            self.fileno += 1;
        }
        t.commit()?;
        Ok(())
    }

    pub fn files(&self) -> anyhow::Result<Vec<(u64, FilesystemEntry)>> {
        let mut stmt = self.conn.prepare("SELECT * FROM files")?;
        let iter = stmt.query_map(params![], |row| row_to_entry(row))?;
        let mut files: Vec<(u64, FilesystemEntry)> = vec![];
        for x in iter {
            let (fileid, entry) = x?;
            files.push((fileid, entry));
        }
        Ok(files)
    }

    pub fn chunkids(&self, fileid: u64) -> anyhow::Result<Vec<ChunkId>> {
        let fileid = fileid as i64;
        let mut stmt = self
            .conn
            .prepare("SELECT chunkid FROM chunks WHERE fileid = ?1")?;
        let iter = stmt.query_map(params![fileid], |row| Ok(row.get(0)?))?;
        let mut ids: Vec<ChunkId> = vec![];
        for x in iter {
            let fileid: String = x?;
            ids.push(ChunkId::from(&fileid));
        }
        Ok(ids)
    }
}

fn row_to_entry(row: &Row) -> rusqlite::Result<(u64, FilesystemEntry)> {
    let fileid: i64 = row.get(row.column_index("fileid")?)?;
    let fileid = fileid as u64;
    let path: Vec<u8> = row.get(row.column_index("path")?)?;
    let path: &OsStr = OsStrExt::from_bytes(&path);
    let path: PathBuf = PathBuf::from(path);
    let kind = row.get(row.column_index("kind")?)?;
    let kind = FilesystemKind::from_code(kind).unwrap();
    let entry = match kind {
        FilesystemKind::Regular => FilesystemEntry::regular(path, 0),
        FilesystemKind::Directory => FilesystemEntry::directory(path),
    };
    Ok((fileid, entry))
}

fn insert_one(
    t: &Transaction,
    e: FilesystemEntry,
    fileno: u64,
    ids: &[ChunkId],
) -> anyhow::Result<()> {
    let path = e.path().as_os_str().to_os_string().into_vec();
    let kind = e.kind().as_code();
    let len = e.len() as i64;
    let fileno = fileno as i64;
    t.execute(
        "INSERT INTO files (fileid, path, kind, len) VALUES (?1, ?2, ?3, ?4)",
        params![fileno, path, kind, len],
    )?;
    for id in ids {
        t.execute(
            "INSERT INTO chunks (fileid, chunkid) VALUES (?1, ?2)",
            params![fileno, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::Generation;
    use tempfile::NamedTempFile;

    #[test]
    fn empty() {
        let filename = NamedTempFile::new().unwrap().path().to_path_buf();
        {
            let mut _gen = Generation::create(&filename).unwrap();
            // _gen is dropped here; the connection is close; the file
            // should not be removed.
        }
        assert!(filename.exists());
    }
}

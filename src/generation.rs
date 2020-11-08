use crate::fsentry::FilesystemEntry;
//use crate::fsiter::FsIterator;
use crate::chunkid::ChunkId;
use rusqlite::{params, Connection, OpenFlags, Transaction};
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

/// A backup generation.
pub struct Generation {
    conn: Connection,
    fileno: u64,
}

impl Generation {
    pub fn new<P>(filename: P) -> anyhow::Result<Self>
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

    pub fn insert(&mut self, e: FilesystemEntry, ids: &[ChunkId]) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        insert_one(&t, e, self.fileno, ids)?;
        self.fileno += 1;
        t.commit()?;
        Ok(())
    }

    pub fn insert_iter<'a>(
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
            let mut _gen = Generation::new(&filename).unwrap();
            // _gen is dropped here; the connection is close; the file
            // should not be removed.
        }
        assert!(filename.exists());
    }
}

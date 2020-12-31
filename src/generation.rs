use crate::chunkid::ChunkId;
use crate::error::ObnamError;
use crate::fsentry::FilesystemEntry;
use rusqlite::{params, Connection, OpenFlags, Row, Transaction};
use std::path::Path;

/// A nascent backup generation.
///
/// A nascent generation is one that is being prepared. It isn't
/// finished yet, and it's not actually on the server until the upload
/// of its generation chunk.
pub struct NascentGeneration {
    conn: Connection,
    fileno: u64,
}

impl NascentGeneration {
    pub fn create<P>(filename: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute(
            "CREATE TABLE files (fileno INTEGER PRIMARY KEY, json TEXT)",
            params![],
        )?;
        conn.execute(
            "CREATE TABLE chunks (fileno INTEGER, chunkid TEXT)",
            params![],
        )?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(Self { conn, fileno: 0 })
    }

    pub fn file_count(&self) -> u64 {
        self.fileno
    }

    pub fn insert(&mut self, e: FilesystemEntry, ids: &[ChunkId]) -> anyhow::Result<()> {
        let t = self.conn.transaction()?;
        self.fileno += 1;
        insert_one(&t, e, self.fileno, ids)?;
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
            self.fileno += 1;
            insert_one(&t, e, self.fileno, &ids[..])?;
        }
        t.commit()?;
        Ok(())
    }
}

fn row_to_entry(row: &Row) -> rusqlite::Result<(u64, String)> {
    let fileno: i64 = row.get(row.column_index("fileno")?)?;
    let fileno = fileno as u64;
    let json: String = row.get(row.column_index("json")?)?;
    Ok((fileno, json))
}

fn insert_one(
    t: &Transaction,
    e: FilesystemEntry,
    fileno: u64,
    ids: &[ChunkId],
) -> anyhow::Result<()> {
    let fileno = fileno as i64;
    let json = serde_json::to_string(&e)?;
    t.execute(
        "INSERT INTO files (fileno, json) VALUES (?1, ?2)",
        params![fileno, &json],
    )?;
    for id in ids {
        t.execute(
            "INSERT INTO chunks (fileno, chunkid) VALUES (?1, ?2)",
            params![fileno, id],
        )?;
    }
    Ok(())
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
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;

        Ok(Self { conn })
    }

    pub fn file_count(&self) -> anyhow::Result<u32> {
        let mut stmt = self.conn.prepare("SELECT count(*) FROM files")?;
        let mut iter = stmt.query_map(params![], |row| row.get(0))?;
        let count = iter.next().expect("SQL count result");
        let count = count?;
        Ok(count)
    }

    pub fn files(&self) -> anyhow::Result<Vec<(u64, FilesystemEntry)>> {
        let mut stmt = self.conn.prepare("SELECT * FROM files")?;
        let iter = stmt.query_map(params![], |row| row_to_entry(row))?;
        let mut files: Vec<(u64, FilesystemEntry)> = vec![];
        for x in iter {
            let (fileno, json) = x?;
            let entry = serde_json::from_str(&json)?;
            files.push((fileno, entry));
        }
        Ok(files)
    }

    pub fn chunkids(&self, fileno: u64) -> anyhow::Result<Vec<ChunkId>> {
        let fileno = fileno as i64;
        let mut stmt = self
            .conn
            .prepare("SELECT chunkid FROM chunks WHERE fileno = ?1")?;
        let iter = stmt.query_map(params![fileno], |row| Ok(row.get(0)?))?;
        let mut ids: Vec<ChunkId> = vec![];
        for x in iter {
            let fileno: String = x?;
            ids.push(ChunkId::from(&fileno));
        }
        Ok(ids)
    }

    pub fn get_file(&self, filename: &Path) -> anyhow::Result<Option<FilesystemEntry>> {
        match self.get_file_and_fileno(filename)? {
            None => Ok(None),
            Some((_, e)) => Ok(Some(e)),
        }
    }

    pub fn get_fileno(&self, filename: &Path) -> anyhow::Result<Option<u64>> {
        match self.get_file_and_fileno(filename)? {
            None => Ok(None),
            Some((id, _)) => Ok(Some(id)),
        }
    }

    fn get_file_and_fileno(
        &self,
        filename: &Path,
    ) -> anyhow::Result<Option<(u64, FilesystemEntry)>> {
        let files = self.files()?;
        let files: Vec<(u64, FilesystemEntry)> = files
            .iter()
            .filter(|(_, e)| e.pathbuf() == filename)
            .map(|(id, e)| (*id, e.clone()))
            .collect();
        match files.len() {
            0 => Ok(None),
            1 => Ok(Some((files[0].0, files[0].1.clone()))),
            _ => return Err(ObnamError::TooManyFiles(filename.to_path_buf()).into()),
        }
    }
}

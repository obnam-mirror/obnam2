//! An on-disk index of chunks for the server.

use crate::checksummer::Checksum;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use rusqlite::Connection;
use std::path::Path;

/// A chunk index stored on the disk.
///
/// A chunk index lets the server quickly find chunks based on a
/// string key/value pair, or whether they are generations.
#[derive(Debug)]
pub struct Index {
    conn: Connection,
}

/// All the errors that may be returned for `Index`.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Index does not have a chunk.
    #[error("The repository index does not have chunk {0}")]
    MissingChunk(ChunkId),

    /// Index has chunk more than once.
    #[error("The repository index duplicates chunk {0}")]
    DuplicateChunk(ChunkId),

    /// An error from SQLite.
    #[error(transparent)]
    SqlError(#[from] rusqlite::Error),
}

impl Index {
    /// Create a new index.
    pub fn new<P: AsRef<Path>>(dirname: P) -> Result<Self, IndexError> {
        let filename = dirname.as_ref().join("meta.db");
        let conn = if filename.exists() {
            sql::open_db(&filename)?
        } else {
            sql::create_db(&filename)?
        };
        Ok(Self { conn })
    }

    /// Insert metadata for a new chunk into index.
    pub fn insert_meta(&mut self, id: ChunkId, meta: ChunkMeta) -> Result<(), IndexError> {
        let t = self.conn.transaction()?;
        sql::insert(&t, &id, &meta)?;
        t.commit()?;
        Ok(())
    }

    /// Look up metadata for a chunk, given its id.
    pub fn get_meta(&self, id: &ChunkId) -> Result<ChunkMeta, IndexError> {
        sql::lookup(&self.conn, id)
    }

    /// Remove a chunk's metadata.
    pub fn remove_meta(&mut self, id: &ChunkId) -> Result<(), IndexError> {
        sql::remove(&self.conn, id)
    }

    /// Find chunks with a client-assigned label.
    pub fn find_by_label(&self, label: &str) -> Result<Vec<ChunkId>, IndexError> {
        sql::find_by_label(&self.conn, label)
    }

    /// Find all backup generations.
    pub fn find_generations(&self) -> Result<Vec<ChunkId>, IndexError> {
        sql::find_generations(&self.conn)
    }

    /// Find all chunks.
    pub fn all_chunks(&self) -> Result<Vec<ChunkId>, IndexError> {
        sql::find_chunk_ids(&self.conn)
    }
}

#[cfg(test)]
mod test {
    use crate::checksummer::Checksum;

    use super::{ChunkId, ChunkMeta, Index};
    use std::path::Path;
    use tempfile::tempdir;

    fn new_index(dirname: &Path) -> Index {
        Index::new(dirname).unwrap()
    }

    #[test]
    fn remembers_inserted() {
        let id: ChunkId = "id001".parse().unwrap();
        let sum = Checksum::sha256_from_str_unchecked("abc");
        let meta = ChunkMeta::new(&sum);
        let dir = tempdir().unwrap();
        let mut idx = new_index(dir.path());
        idx.insert_meta(id.clone(), meta.clone()).unwrap();
        assert_eq!(idx.get_meta(&id).unwrap(), meta);
        let ids = idx.find_by_label("abc").unwrap();
        assert_eq!(ids, vec![id]);
    }

    #[test]
    fn does_not_find_uninserted() {
        let id: ChunkId = "id001".parse().unwrap();
        let sum = Checksum::sha256_from_str_unchecked("abc");
        let meta = ChunkMeta::new(&sum);
        let dir = tempdir().unwrap();
        let mut idx = new_index(dir.path());
        idx.insert_meta(id, meta).unwrap();
        assert_eq!(idx.find_by_label("def").unwrap().len(), 0)
    }

    #[test]
    fn removes_inserted() {
        let id: ChunkId = "id001".parse().unwrap();
        let sum = Checksum::sha256_from_str_unchecked("abc");
        let meta = ChunkMeta::new(&sum);
        let dir = tempdir().unwrap();
        let mut idx = new_index(dir.path());
        idx.insert_meta(id.clone(), meta).unwrap();
        idx.remove_meta(&id).unwrap();
        let ids: Vec<ChunkId> = idx.find_by_label("abc").unwrap();
        assert_eq!(ids, vec![]);
    }

    #[test]
    fn has_no_generations_initially() {
        let dir = tempdir().unwrap();
        let idx = new_index(dir.path());
        assert_eq!(idx.find_generations().unwrap(), vec![]);
    }

    #[test]
    fn remembers_generation() {
        let id: ChunkId = "id001".parse().unwrap();
        let sum = Checksum::sha256_from_str_unchecked("abc");
        let meta = ChunkMeta::new_generation(&sum, "timestamp");
        let dir = tempdir().unwrap();
        let mut idx = new_index(dir.path());
        idx.insert_meta(id.clone(), meta).unwrap();
        assert_eq!(idx.find_generations().unwrap(), vec![id]);
    }

    #[test]
    fn removes_generation() {
        let id: ChunkId = "id001".parse().unwrap();
        let sum = Checksum::sha256_from_str_unchecked("abc");
        let meta = ChunkMeta::new_generation(&sum, "timestamp");
        let dir = tempdir().unwrap();
        let mut idx = new_index(dir.path());
        idx.insert_meta(id.clone(), meta).unwrap();
        idx.remove_meta(&id).unwrap();
        assert_eq!(idx.find_generations().unwrap(), vec![]);
    }
}

mod sql {
    use super::{Checksum, IndexError};
    use crate::chunkid::ChunkId;
    use crate::chunkmeta::ChunkMeta;
    use log::error;
    use rusqlite::{params, Connection, OpenFlags, Row, Transaction};
    use std::path::Path;

    /// Create a database in a file.
    pub fn create_db(filename: &Path) -> Result<Connection, IndexError> {
        let flags = OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.execute(
            "CREATE TABLE chunks (id TEXT PRIMARY KEY, label TEXT, generation INT, ended TEXT)",
            params![],
        )?;
        conn.execute("CREATE INDEX label_idx ON chunks (label)", params![])?;
        conn.execute(
            "CREATE INDEX generation_idx ON chunks (generation)",
            params![],
        )?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(conn)
    }

    /// Open an existing database in a file.
    pub fn open_db(filename: &Path) -> Result<Connection, IndexError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let conn = Connection::open_with_flags(filename, flags)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(conn)
    }

    /// Insert a new chunk's metadata into database.
    pub fn insert(t: &Transaction, chunkid: &ChunkId, meta: &ChunkMeta) -> Result<(), IndexError> {
        let chunkid = format!("{}", chunkid);
        let label = meta.label();
        let generation = if meta.is_generation() { 1 } else { 0 };
        let ended = meta.ended();
        t.execute(
            "INSERT INTO chunks (id, label, generation, ended) VALUES (?1, ?2, ?3, ?4)",
            params![chunkid, label, generation, ended],
        )?;
        Ok(())
    }

    /// Remove a chunk's metadata from the database.
    pub fn remove(conn: &Connection, chunkid: &ChunkId) -> Result<(), IndexError> {
        conn.execute("DELETE FROM chunks WHERE id IS ?1", params![chunkid])?;
        Ok(())
    }

    /// Look up a chunk using its id.
    pub fn lookup(conn: &Connection, id: &ChunkId) -> Result<ChunkMeta, IndexError> {
        let mut stmt = conn.prepare("SELECT * FROM chunks WHERE id IS ?1")?;
        let iter = stmt.query_map(params![id], row_to_meta)?;
        let mut metas: Vec<ChunkMeta> = vec![];
        for meta in iter {
            let meta = meta?;
            if metas.is_empty() {
                metas.push(meta);
            } else {
                let err = IndexError::DuplicateChunk(id.clone());
                error!("{}", err);
                return Err(err);
            }
        }
        if metas.is_empty() {
            return Err(IndexError::MissingChunk(id.clone()));
        }
        let r = metas[0].clone();
        Ok(r)
    }

    /// Find chunks with a given checksum.
    pub fn find_by_label(conn: &Connection, label: &str) -> Result<Vec<ChunkId>, IndexError> {
        let mut stmt = conn.prepare("SELECT id FROM chunks WHERE label IS ?1")?;
        let iter = stmt.query_map(params![label], row_to_id)?;
        let mut ids = vec![];
        for x in iter {
            let x = x?;
            ids.push(x);
        }
        Ok(ids)
    }

    /// Find all generations.
    pub fn find_generations(conn: &Connection) -> Result<Vec<ChunkId>, IndexError> {
        let mut stmt = conn.prepare("SELECT id FROM chunks WHERE generation IS 1")?;
        let iter = stmt.query_map(params![], row_to_id)?;
        let mut ids = vec![];
        for x in iter {
            let x = x?;
            ids.push(x);
        }
        Ok(ids)
    }

    /// Find ids of all chunks.
    pub fn find_chunk_ids(conn: &Connection) -> Result<Vec<ChunkId>, IndexError> {
        let mut stmt = conn.prepare("SELECT id FROM chunks WHERE generation IS 0")?;
        let iter = stmt.query_map(params![], row_to_id)?;
        let mut ids = vec![];
        for x in iter {
            let x = x?;
            ids.push(x);
        }
        Ok(ids)
    }

    fn row_to_meta(row: &Row) -> rusqlite::Result<ChunkMeta> {
        let hash: String = row.get("label")?;
        let sha256 = Checksum::sha256_from_str_unchecked(&hash);
        let generation: i32 = row.get("generation")?;
        let meta = if generation == 0 {
            ChunkMeta::new(&sha256)
        } else {
            let ended: String = row.get("ended")?;
            ChunkMeta::new_generation(&sha256, &ended)
        };
        Ok(meta)
    }

    fn row_to_id(row: &Row) -> rusqlite::Result<ChunkId> {
        let id: String = row.get("id")?;
        Ok(ChunkId::recreate(&id))
    }
}

//! Store chunks on-disk on server.

use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use std::path::{Path, PathBuf};

/// Store chunks, with metadata, persistently.
///
/// The chunks and their metadata are stored persistently on disk
/// under a directory specified as the Store struct is created. To
/// store or retrieve a chunk its identifier must be used.
pub struct Store {
    dir: PathBuf,
}

/// An error from a `Store` operation.
pub type StoreError = std::io::Error;

impl Store {
    /// Create a new Store to represent on-disk storage of chunks.x
    pub fn new(dir: &Path) -> Self {
        Store {
            dir: dir.to_path_buf(),
        }
    }

    // Construct name for a files in the store from chunk id.
    //
    // The name of directory containing the file is returned
    // separately to make it easier to create it if needed.
    fn filenames(&self, id: &ChunkId) -> (PathBuf, PathBuf, PathBuf) {
        let bytes = id.as_bytes();
        assert!(bytes.len() > 3);
        let a = bytes[0];
        let b = bytes[1];
        let c = bytes[2];
        let dir = self.dir.join(format!("{}/{}/{}", a, b, c));
        let meta = dir.join(format!("{}.{}", id, "meta"));
        let data = dir.join(format!("{}.{}", id, "data"));
        (dir, meta, data)
    }

    /// Save a chunk into a store.
    pub fn save(&self, id: &ChunkId, chunk: &DataChunk) -> Result<(), StoreError> {
        let (dir, metaname, dataname) = &self.filenames(id);

        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }

        std::fs::write(&metaname, chunk.meta().to_json())?;
        std::fs::write(&dataname, chunk.data())?;
        Ok(())
    }

    /// Load a chunk from a store.
    pub fn load(&self, id: &ChunkId) -> Result<DataChunk, StoreError> {
        let (_, metaname, dataname) = &self.filenames(id);
        let meta = std::fs::read(&metaname)?;
        let meta = serde_json::from_slice(&meta)?;

        let data = std::fs::read(&dataname)?;
        let data = DataChunk::new(data, meta);
        Ok(data)
    }

    /// Delete a chunk from a store.
    pub fn delete(&self, id: &ChunkId) -> Result<(), StoreError> {
        let (_, metaname, dataname) = &self.filenames(id);
        std::fs::remove_file(&metaname)?;
        std::fs::remove_file(&dataname)?;
        Ok(())
    }
}

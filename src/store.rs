use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use std::path::{Path, PathBuf};

/// Store chunks, with metadata, persistently.
///
/// The chunks and their metadata are stored persistently on disk
/// under a directory specified as the Store struct is created. To
/// store or retrieve a chunk its identifier must be used.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Create a new Store to represent on-disk storage of chunks.x
    pub fn new(dir: &Path) -> Self {
        Store {
            dir: dir.to_path_buf(),
        }
    }

    // Construct name for a file in the store from chunk id and suffix.
    fn filename(&self, id: &ChunkId, suffix: &str) -> PathBuf {
        self.dir.join(format!("{}.{}", id, suffix))
    }

    /// Save a chunk into a store.
    pub fn save(&self, id: &ChunkId, meta: &ChunkMeta, chunk: &DataChunk) -> anyhow::Result<()> {
        std::fs::write(&self.filename(id, "meta"), meta.to_json())?;
        std::fs::write(&self.filename(id, "data"), chunk.data())?;
        Ok(())
    }

    /// Load a chunk's metadata from a store.
    pub fn load_meta(&self, id: &ChunkId) -> anyhow::Result<ChunkMeta> {
        let meta = std::fs::read(&self.filename(id, "meta"))?;
        Ok(serde_json::from_slice(&meta)?)
    }

    /// Load a chunk from a store.
    pub fn load(&self, id: &ChunkId) -> anyhow::Result<(ChunkMeta, DataChunk)> {
        let meta = self.load_meta(id)?;
        let data = std::fs::read(&self.filename(id, "data"))?;
        Ok((meta, DataChunk::new(data)))
    }

    /// Delete a chunk from a store.
    pub fn delete(&self, id: &ChunkId) -> anyhow::Result<()> {
        std::fs::remove_file(&self.filename(id, "meta"))?;
        std::fs::remove_file(&self.filename(id, "data"))?;
        Ok(())
    }
}

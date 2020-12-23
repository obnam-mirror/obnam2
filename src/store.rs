use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use anyhow::Context;
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
    pub fn save(&self, id: &ChunkId, meta: &ChunkMeta, chunk: &DataChunk) -> anyhow::Result<()> {
        let (dir, metaname, dataname) = &self.filenames(id);

        if !dir.exists() {
            let res = std::fs::create_dir_all(dir).into();
            if let Err(_) = res {
                return res.with_context(|| format!("creating directory {}", dir.display()));
            }
        }

        std::fs::write(&metaname, meta.to_json())?;
        std::fs::write(&dataname, chunk.data())?;
        Ok(())
    }

    /// Load a chunk's metadata from a store.
    pub fn load_meta(&self, id: &ChunkId) -> anyhow::Result<ChunkMeta> {
        let (_, metaname, _) = &self.filenames(id);
        let meta = std::fs::read(&metaname)?;
        Ok(serde_json::from_slice(&meta)?)
    }

    /// Load a chunk from a store.
    pub fn load(&self, id: &ChunkId) -> anyhow::Result<LoadedChunk> {
        let (_, _, dataname) = &self.filenames(id);
        let meta = self.load_meta(id)?;
        let data = std::fs::read(&dataname)?;
        let data = DataChunk::new(data);
        Ok(LoadedChunk { meta, data })
    }

    /// Delete a chunk from a store.
    pub fn delete(&self, id: &ChunkId) -> anyhow::Result<()> {
        let (_, metaname, dataname) = &self.filenames(id);
        std::fs::remove_file(&metaname)?;
        std::fs::remove_file(&dataname)?;
        Ok(())
    }
}

pub struct LoadedChunk {
    meta: ChunkMeta,
    data: DataChunk,
}

impl LoadedChunk {
    pub fn new(meta: ChunkMeta, data: DataChunk) -> Self {
        Self { meta, data }
    }

    pub fn meta(&self) -> &ChunkMeta {
        &self.meta
    }

    pub fn data(&self) -> &DataChunk {
        &self.data
    }
}

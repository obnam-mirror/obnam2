use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::index::Index;
use crate::store::{LoadedChunk, Store};
use std::path::Path;

/// A store for chunks and their metadata.
///
/// This combines Store and Index into one interface to make it easier
/// to handle the server side storage of chunks.
pub struct IndexedStore {
    store: Store,
    index: Index,
}

impl IndexedStore {
    pub fn new(dirname: &Path) -> Self {
        let store = Store::new(dirname);
        let index = Index::default();
        Self { store, index }
    }

    pub fn save(&mut self, meta: &ChunkMeta, chunk: &DataChunk) -> anyhow::Result<ChunkId> {
        let id = ChunkId::new();
        self.store.save(&id, meta, chunk)?;
        self.index.insert(id.clone(), "sha256", meta.sha256());
        if meta.is_generation() {
            self.index.insert_generation(id.clone());
        }
        Ok(id)
    }

    pub fn load(&self, id: &ChunkId) -> anyhow::Result<LoadedChunk> {
        self.store.load(id)
    }

    pub fn load_meta(&self, id: &ChunkId) -> anyhow::Result<ChunkMeta> {
        self.store.load_meta(id)
    }

    pub fn find_by_sha256(&self, sha256: &str) -> Vec<ChunkId> {
        self.index.find("sha256", sha256)
    }

    pub fn find_generations(&self) -> Vec<ChunkId> {
        self.index.find_generations()
    }

    pub fn remove(&mut self, id: &ChunkId) -> anyhow::Result<()> {
        let loaded = self.store.load(id)?;
        self.index.remove("sha256", loaded.meta().sha256());
        self.index.remove_generation(id);
        self.store.delete(id)?;
        Ok(())
    }
}

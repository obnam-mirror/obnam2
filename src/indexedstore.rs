use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::index::Index;
use crate::store::Store;
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
    pub fn new(dirname: &Path) -> anyhow::Result<Self> {
        let store = Store::new(dirname);
        let index = Index::new(dirname)?;
        Ok(Self { store, index })
    }

    pub fn save(&mut self, meta: &ChunkMeta, chunk: &DataChunk) -> anyhow::Result<ChunkId> {
        let id = ChunkId::new();
        self.store.save(&id, meta, chunk)?;
        self.insert_meta(&id, meta)?;
        Ok(id)
    }

    fn insert_meta(&mut self, id: &ChunkId, meta: &ChunkMeta) -> anyhow::Result<()> {
        self.index.insert_meta(id.clone(), meta.clone())?;
        Ok(())
    }

    pub fn load(&self, id: &ChunkId) -> anyhow::Result<(DataChunk, ChunkMeta)> {
        Ok((self.store.load(id)?, self.load_meta(id)?))
    }

    pub fn load_meta(&self, id: &ChunkId) -> anyhow::Result<ChunkMeta> {
        self.index.get_meta(id)
    }

    pub fn find_by_sha256(&self, sha256: &str) -> anyhow::Result<Vec<ChunkId>> {
        self.index.find_by_sha256(sha256)
    }

    pub fn find_generations(&self) -> anyhow::Result<Vec<ChunkId>> {
        self.index.find_generations()
    }

    pub fn remove(&mut self, id: &ChunkId) -> anyhow::Result<()> {
        self.index.remove_meta(id).unwrap();
        self.store.delete(id)?;
        Ok(())
    }
}

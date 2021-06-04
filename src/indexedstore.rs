use crate::chunk::{DataChunk, GenerationChunkError};
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::index::{Index, IndexError};
use crate::store::{Store, StoreError};
use std::path::Path;

/// A store for chunks and their metadata.
///
/// This combines Store and Index into one interface to make it easier
/// to handle the server side storage of chunks.
pub struct IndexedStore {
    store: Store,
    index: Index,
}

/// All the errors that may be returned for `IndexStore`.
#[derive(Debug, thiserror::Error)]
pub enum IndexedError {
    /// An error from Index.
    #[error(transparent)]
    IndexError(#[from] IndexError),

    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),

    /// An error from Store.
    #[error(transparent)]
    SqlError(#[from] StoreError),
}

/// A result from an `Index` operation.
pub type IndexedResult<T> = Result<T, IndexedError>;

impl IndexedStore {
    pub fn new(dirname: &Path) -> IndexedResult<Self> {
        let store = Store::new(dirname);
        let index = Index::new(dirname)?;
        Ok(Self { store, index })
    }

    pub fn save(&mut self, chunk: &DataChunk) -> IndexedResult<ChunkId> {
        let id = ChunkId::new();
        self.store.save(&id, chunk)?;
        self.insert_meta(&id, chunk.meta())?;
        Ok(id)
    }

    fn insert_meta(&mut self, id: &ChunkId, meta: &ChunkMeta) -> IndexedResult<()> {
        self.index.insert_meta(id.clone(), meta.clone())?;
        Ok(())
    }

    pub fn load(&self, id: &ChunkId) -> IndexedResult<(DataChunk, ChunkMeta)> {
        Ok((self.store.load(id)?, self.load_meta(id)?))
    }

    pub fn load_meta(&self, id: &ChunkId) -> IndexedResult<ChunkMeta> {
        Ok(self.index.get_meta(id)?)
    }

    pub fn find_by_sha256(&self, sha256: &str) -> IndexedResult<Vec<ChunkId>> {
        Ok(self.index.find_by_sha256(sha256)?)
    }

    pub fn find_generations(&self) -> IndexedResult<Vec<ChunkId>> {
        Ok(self.index.find_generations()?)
    }

    pub fn remove(&mut self, id: &ChunkId) -> IndexedResult<()> {
        self.index.remove_meta(id)?;
        self.store.delete(id)?;
        Ok(())
    }
}

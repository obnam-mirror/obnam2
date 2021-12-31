//! An indexed, on-disk store for chunks on the server.

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

    /// Error regarding generation chunks.
    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),

    /// An error from Store.
    #[error(transparent)]
    SqlError(#[from] StoreError),
}

impl IndexedStore {
    /// Create a new indexed store.
    pub fn new(dirname: &Path) -> Result<Self, IndexedError> {
        let store = Store::new(dirname);
        let index = Index::new(dirname)?;
        Ok(Self { store, index })
    }

    /// Save a chunk in the store.
    pub fn save(&mut self, chunk: &DataChunk) -> Result<ChunkId, IndexedError> {
        let id = ChunkId::new();
        self.store.save(&id, chunk)?;
        self.insert_meta(&id, chunk.meta())?;
        Ok(id)
    }

    fn insert_meta(&mut self, id: &ChunkId, meta: &ChunkMeta) -> Result<(), IndexedError> {
        self.index.insert_meta(id.clone(), meta.clone())?;
        Ok(())
    }

    /// Get a chunk from the store, given its id.
    pub fn load(&self, id: &ChunkId) -> Result<(DataChunk, ChunkMeta), IndexedError> {
        Ok((self.store.load(id)?, self.load_meta(id)?))
    }

    /// Get a chunk's metadata form the store, given its id.
    pub fn load_meta(&self, id: &ChunkId) -> Result<ChunkMeta, IndexedError> {
        Ok(self.index.get_meta(id)?)
    }

    /// Find chunks with a given checksum.
    pub fn find_by_sha256(&self, sha256: &str) -> Result<Vec<ChunkId>, IndexedError> {
        Ok(self.index.find_by_sha256(sha256)?)
    }

    /// Find all generations.
    pub fn find_generations(&self) -> Result<Vec<ChunkId>, IndexedError> {
        Ok(self.index.find_generations()?)
    }

    /// Remove a chunk from the store.
    pub fn remove(&mut self, id: &ChunkId) -> Result<(), IndexedError> {
        self.index.remove_meta(id)?;
        self.store.delete(id)?;
        Ok(())
    }
}

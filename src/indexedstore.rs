use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::index::Index;
use crate::store::{LoadedChunk, Store};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A store for chunks and their metadata.
///
/// This combines Store and Index into one interface to make it easier
/// to handle the server side storage of chunks.
pub struct IndexedStore {
    dirname: PathBuf,
    store: Store,
    index: Index,
}

impl IndexedStore {
    pub fn new(dirname: &Path) -> Self {
        let store = Store::new(dirname);
        let index = Index::default();
        Self {
            dirname: dirname.to_path_buf(),
            store,
            index,
        }
    }

    pub fn fill_index(&mut self) -> anyhow::Result<()> {
        for entry in WalkDir::new(&self.dirname) {
            let entry = entry?;
            let path = entry.path();
            //            println!("found entry: {:?} (ext: {:?})", path, path.extension());
            if let Some(ext) = path.extension() {
                if ext == "meta" {
                    println!("found meta: {:?}", path);
                    let text = std::fs::read(path)?;
                    let meta: ChunkMeta = serde_json::from_slice(&text)?;
                    if let Some(stem) = path.file_stem() {
                        let id: ChunkId = stem.into();
                        println!("id: {:?}", id);
                        self.insert_meta(&id, &meta);
                    }
                }
            }
            println!("");
        }
        Ok(())
    }

    pub fn save(&mut self, meta: &ChunkMeta, chunk: &DataChunk) -> anyhow::Result<ChunkId> {
        let id = ChunkId::new();
        self.store.save(&id, meta, chunk)?;
        self.insert_meta(&id, meta);
        Ok(id)
    }

    fn insert_meta(&mut self, id: &ChunkId, meta: &ChunkMeta) {
        self.index.insert(id.clone(), "sha256", meta.sha256());
        if meta.is_generation() {
            self.index.insert_generation(id.clone());
        }
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

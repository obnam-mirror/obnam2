use crate::chunkmeta::ChunkMeta;

/// Store an arbitrary chunk of data.
///
/// The data is just arbitrary binary data.
///
/// A chunk also contains its associated metadata, except its
/// identifier.
pub struct Chunk {
    meta: ChunkMeta,
    data: Vec<u8>,
}

impl Chunk {
    /// Construct a new chunk.
    pub fn new(meta: ChunkMeta, data: Vec<u8>) -> Self {
        Chunk { meta, data }
    }

    /// Return a chunk's metadata.
    pub fn meta(&self) -> &ChunkMeta {
        &self.meta
    }

    /// Return a chunk's data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

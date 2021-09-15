use crate::checksummer::Checksum;
use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;

// Generate a desired number of empty data chunks with id and metadata.
pub struct ChunkGenerator {
    goal: u32,
    next: u32,
}

impl ChunkGenerator {
    pub fn new(goal: u32) -> Self {
        Self { goal, next: 0 }
    }
}

impl Iterator for ChunkGenerator {
    type Item = (ChunkId, Checksum, DataChunk);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.goal {
            None
        } else {
            let id = ChunkId::recreate(&format!("{}", self.next));
            let checksum = id.sha256();
            let meta = ChunkMeta::new(&checksum);
            let chunk = DataChunk::new(vec![], meta);
            self.next += 1;
            Some((id, checksum, chunk))
        }
    }
}

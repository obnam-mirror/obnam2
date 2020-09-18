use crate::chunk::Chunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;

/// Result of creating a chunk.
#[derive(Debug, Serialize)]
pub struct Created {
    id: ChunkId,
}

impl Created {
    pub fn new(id: ChunkId) -> Self {
        Created { id }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

/// Result of retrieving a chunk.

#[derive(Debug, Serialize)]
pub struct Fetched {
    id: ChunkId,
    chunk: Chunk,
}

impl Fetched {
    pub fn new(id: ChunkId, chunk: Chunk) -> Self {
        Fetched { id, chunk }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

/// Result of a search.
#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct SearchHits {
    map: HashMap<String, ChunkMeta>,
}

impl SearchHits {
    pub fn insert(&mut self, id: ChunkId, meta: ChunkMeta) {
        self.map.insert(format!("{}", id), meta);
    }

    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        let map = serde_json::from_str(s)?;
        Ok(SearchHits { map })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.map).unwrap()
    }
}

#[cfg(test)]
mod test_search_hits {
    use super::{ChunkMeta, SearchHits};

    #[test]
    fn no_search_hits() {
        let hits = SearchHits::default();
        assert_eq!(hits.to_json(), "{}");
    }

    #[test]
    fn one_search_hit() {
        let id = "abc".parse().unwrap();
        let meta = ChunkMeta::new("123");
        let mut hits = SearchHits::default();
        hits.insert(id, meta);
        eprintln!("hits: {:?}", hits);
        let json = hits.to_json();
        let hits2 = SearchHits::from_json(&json).unwrap();
        assert_eq!(hits, hits2);
    }
}

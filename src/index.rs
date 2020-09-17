use crate::chunkid::ChunkId;
use std::collections::HashMap;
use std::default::Default;

/// A chunk index.
///
/// A chunk index lets the server quickly find chunks based on a
/// string key/value pair, or whether they are generations.
#[derive(Debug, Default)]
pub struct Index {
    map: HashMap<(String, String), Vec<ChunkId>>,
    generations: Vec<ChunkId>,
}

impl Index {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn insert(&mut self, id: ChunkId, key: &str, value: &str) {
        let kv = kv(key, value);
        if let Some(v) = self.map.get_mut(&kv) {
            v.push(id)
        } else {
            self.map.insert(kv, vec![id]);
        }
    }

    pub fn find(&self, key: &str, value: &str) -> Vec<ChunkId> {
        let kv = kv(key, value);
        if let Some(v) = self.map.get(&kv) {
            v.clone()
        } else {
            vec![]
        }
    }

    pub fn insert_generation(&mut self, id: ChunkId) {
        self.generations.push(id)
    }

    pub fn find_generations(&self) -> Vec<ChunkId> {
        self.generations.clone()
    }
}

fn kv(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

#[cfg(test)]
mod test {
    use super::{ChunkId, Index};

    #[test]
    fn is_empty_initially() {
        let idx = Index::default();
        assert!(idx.is_empty());
    }

    #[test]
    fn remembers_inserted() {
        let id: ChunkId = "id001".parse().unwrap();
        let mut idx = Index::default();
        idx.insert(id.clone(), "sha256", "abc");
        assert!(!idx.is_empty());
        assert_eq!(idx.len(), 1);
        let ids: Vec<ChunkId> = idx.find("sha256", "abc");
        assert_eq!(ids, vec![id]);
    }

    #[test]
    fn does_not_find_uninserted() {
        let id: ChunkId = "id001".parse().unwrap();
        let mut idx = Index::default();
        idx.insert(id, "sha256", "abc");
        assert_eq!(idx.find("sha256", "def").len(), 0)
    }

    #[test]
    fn has_no_generations_initially() {
        let idx = Index::default();
        assert_eq!(idx.find_generations(), vec![]);
    }

    #[test]
    fn remembers_generation() {
        let id: ChunkId = "id001".parse().unwrap();
        let mut idx = Index::default();
        idx.insert_generation(id.clone());
        assert_eq!(idx.find_generations(), vec![id]);
    }
}

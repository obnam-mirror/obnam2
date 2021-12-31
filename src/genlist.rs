//! A list of generations on the server.

use crate::chunkid::ChunkId;
use crate::generation::{FinishedGeneration, GenId};

/// A list of generations on the server.
pub struct GenerationList {
    list: Vec<FinishedGeneration>,
}

/// Possible errors from listing generations.
#[derive(Debug, thiserror::Error)]
pub enum GenerationListError {
    /// Server doesn't know about a generation.
    #[error("Unknown generation: {0}")]
    UnknownGeneration(ChunkId),
}

impl GenerationList {
    /// Create a new list of generations.
    pub fn new(gens: Vec<FinishedGeneration>) -> Self {
        let mut list = gens;
        list.sort_by_cached_key(|gen| gen.ended().to_string());
        Self { list }
    }

    /// Return an iterator over the generations.
    pub fn iter(&self) -> impl Iterator<Item = &FinishedGeneration> {
        self.list.iter()
    }

    /// Resolve a symbolic name of a generation into its identifier.
    ///
    /// For example, "latest" refers to the latest backup, but needs
    /// to be resolved into an actual, immutable id to actually be
    /// restored.
    pub fn resolve(&self, genref: &str) -> Result<GenId, GenerationListError> {
        let gen = if self.list.is_empty() {
            None
        } else if genref == "latest" {
            let i = self.list.len() - 1;
            Some(self.list[i].clone())
        } else {
            let genref = GenId::from_chunk_id(genref.parse().unwrap());
            let hits: Vec<FinishedGeneration> = self
                .iter()
                .filter(|gen| gen.id().as_chunk_id() == genref.as_chunk_id())
                .cloned()
                .collect();
            if hits.len() == 1 {
                Some(hits[0].clone())
            } else {
                None
            }
        };
        match gen {
            None => Err(GenerationListError::UnknownGeneration(ChunkId::recreate(
                genref,
            ))),
            Some(gen) => Ok(gen.id().clone()),
        }
    }
}

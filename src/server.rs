use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub chunks: PathBuf,
    pub address: String,
    pub tls_key: PathBuf,
    pub tls_cert: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Directory for chunks {0} does not exist")]
    ChunksDirNotFound(PathBuf),

    #[error("TLS certificate {0} does not exist")]
    TlsCertNotFound(PathBuf),

    #[error("TLS key {0} does not exist")]
    TlsKeyNotFound(PathBuf),

    #[error("server address can't be resolved")]
    BadServerAddress,
}

impl Config {
    pub fn read_config(filename: &Path) -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(filename)?;
        let config: Config = serde_yaml::from_str(&config)?;
        config.check()?;
        Ok(config)
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if !self.chunks.exists() {
            return Err(ConfigError::ChunksDirNotFound(self.chunks.clone()).into());
        }
        if !self.tls_cert.exists() {
            return Err(ConfigError::TlsCertNotFound(self.tls_cert.clone()).into());
        }
        if !self.tls_key.exists() {
            return Err(ConfigError::TlsKeyNotFound(self.tls_key.clone()).into());
        }
        Ok(())
    }
}

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
    chunk: DataChunk,
}

impl Fetched {
    pub fn new(id: ChunkId, chunk: DataChunk) -> Self {
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
        self.map.insert(id.to_string(), meta);
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
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

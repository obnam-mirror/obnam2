//! Stuff related to the Obnam chunk server.

use crate::chunk::DataChunk;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::path::{Path, PathBuf};

/// Server configuration.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Path to directory where chunks are stored.
    pub chunks: PathBuf,
    /// Address where server is to listen.
    pub address: String,
    /// Path to TLS key.
    pub tls_key: PathBuf,
    /// Path to TLS certificate.
    pub tls_cert: PathBuf,
}

/// Possible errors wittht server configuration.
#[derive(Debug, thiserror::Error)]
pub enum ServerConfigError {
    /// The chunks directory doesn't exist.
    #[error("Directory for chunks {0} does not exist")]
    ChunksDirNotFound(PathBuf),

    /// The TLS certificate doesn't exist.
    #[error("TLS certificate {0} does not exist")]
    TlsCertNotFound(PathBuf),

    /// The TLS key doesn't exist.
    #[error("TLS key {0} does not exist")]
    TlsKeyNotFound(PathBuf),

    /// Server address is wrong.
    #[error("server address can't be resolved")]
    BadServerAddress,

    /// Failed to read configuration file.
    #[error("failed to read configuration file {0}: {1}")]
    Read(PathBuf, std::io::Error),

    /// Failed to parse configuration file as YAML.
    #[error("failed to parse configuration file as YAML: {0}")]
    YamlParse(serde_yaml::Error),
}

impl ServerConfig {
    /// Read, parse, and check the server configuration file.
    pub fn read_config(filename: &Path) -> Result<Self, ServerConfigError> {
        let config = match std::fs::read_to_string(filename) {
            Ok(config) => config,
            Err(err) => return Err(ServerConfigError::Read(filename.to_path_buf(), err)),
        };
        let config: Self = serde_yaml::from_str(&config).map_err(ServerConfigError::YamlParse)?;
        config.check()?;
        Ok(config)
    }

    /// Check the configuration.
    pub fn check(&self) -> Result<(), ServerConfigError> {
        if !self.chunks.exists() {
            return Err(ServerConfigError::ChunksDirNotFound(self.chunks.clone()));
        }
        if !self.tls_cert.exists() {
            return Err(ServerConfigError::TlsCertNotFound(self.tls_cert.clone()));
        }
        if !self.tls_key.exists() {
            return Err(ServerConfigError::TlsKeyNotFound(self.tls_key.clone()));
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
    /// Create a new created chunk id.
    pub fn new(id: ChunkId) -> Self {
        Created { id }
    }

    /// Convert to JSON.
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
    /// Create a new id for a fetched chunk.
    pub fn new(id: ChunkId, chunk: DataChunk) -> Self {
        Fetched { id, chunk }
    }

    /// Convert to JSON.
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
    /// Insert a new chunk id to search results.
    pub fn insert(&mut self, id: ChunkId, meta: ChunkMeta) {
        self.map.insert(id.to_string(), meta);
    }

    /// Convert from JSON.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        let map = serde_json::from_str(s)?;
        Ok(SearchHits { map })
    }

    /// Convert to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.map).unwrap()
    }
}

#[cfg(test)]
mod test_search_hits {
    use super::{ChunkMeta, SearchHits};
    use crate::label::Label;

    #[test]
    fn no_search_hits() {
        let hits = SearchHits::default();
        assert_eq!(hits.to_json(), "{}");
    }

    #[test]
    fn one_search_hit() {
        let id = "abc".parse().unwrap();
        let sum = Label::sha256(b"123");
        let meta = ChunkMeta::new(&sum);
        let mut hits = SearchHits::default();
        hits.insert(id, meta);
        eprintln!("hits: {:?}", hits);
        let json = hits.to_json();
        let hits2 = SearchHits::from_json(&json).unwrap();
        assert_eq!(hits, hits2);
    }
}

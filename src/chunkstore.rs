//! Access local and remote chunk stores.
//!
//! A chunk store may be local and accessed via the file system, or
//! remote and accessed over HTTP. This module implements both. This
//! module only handles encrypted chunks.

use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::config::{ClientConfig, ClientConfigError};
use crate::index::{Index, IndexError};

use log::{debug, error, info};
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

/// A chunk store.
///
/// The store may be local or remote.
pub enum ChunkStore {
    /// A local chunk store.
    Local(LocalStore),

    /// A remote chunk store.
    Remote(RemoteStore),
}

impl ChunkStore {
    /// Open a local chunk store.
    pub fn local<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let store = LocalStore::new(path.as_ref())?;
        Ok(Self::Local(store))
    }

    /// Open a remote chunk store.
    pub fn remote(config: &ClientConfig) -> Result<Self, StoreError> {
        let store = RemoteStore::new(config)?;
        Ok(Self::Remote(store))
    }

    /// Does the store have a chunk with a given label?
    pub async fn find_by_label(&self, meta: &ChunkMeta) -> Result<Vec<ChunkId>, StoreError> {
        match self {
            Self::Local(store) => store.find_by_label(meta).await,
            Self::Remote(store) => store.find_by_label(meta).await,
        }
    }

    /// Store a chunk in the store.
    ///
    /// The store chooses an id for the chunk.
    pub async fn put(&self, chunk: Vec<u8>, meta: &ChunkMeta) -> Result<ChunkId, StoreError> {
        match self {
            Self::Local(store) => store.put(chunk, meta).await,
            Self::Remote(store) => store.put(chunk, meta).await,
        }
    }

    /// Get a chunk given its id.
    pub async fn get(&self, id: &ChunkId) -> Result<(Vec<u8>, ChunkMeta), StoreError> {
        match self {
            Self::Local(store) => store.get(id).await,
            Self::Remote(store) => store.get(id).await,
        }
    }
}

/// A local chunk store.
pub struct LocalStore {
    path: PathBuf,
    index: Mutex<Index>,
}

impl LocalStore {
    fn new(path: &Path) -> Result<Self, StoreError> {
        Ok(Self {
            path: path.to_path_buf(),
            index: Mutex::new(Index::new(path)?),
        })
    }

    async fn find_by_label(&self, meta: &ChunkMeta) -> Result<Vec<ChunkId>, StoreError> {
        self.index
            .lock()
            .await
            .find_by_label(meta.label())
            .map_err(StoreError::Index)
    }

    async fn put(&self, chunk: Vec<u8>, meta: &ChunkMeta) -> Result<ChunkId, StoreError> {
        let id = ChunkId::new();
        let (dir, filename) = self.filename(&id);

        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|err| StoreError::ChunkMkdir(dir, err))?;
        }

        std::fs::write(&filename, &chunk)
            .map_err(|err| StoreError::WriteChunk(filename.clone(), err))?;
        self.index
            .lock()
            .await
            .insert_meta(id.clone(), meta.clone())
            .map_err(StoreError::Index)?;
        Ok(id)
    }

    async fn get(&self, id: &ChunkId) -> Result<(Vec<u8>, ChunkMeta), StoreError> {
        let meta = self.index.lock().await.get_meta(id)?;

        let (_, filename) = &self.filename(id);

        let raw =
            std::fs::read(&filename).map_err(|err| StoreError::ReadChunk(filename.clone(), err))?;

        Ok((raw, meta))
    }

    fn filename(&self, id: &ChunkId) -> (PathBuf, PathBuf) {
        let bytes = id.as_bytes();
        assert!(bytes.len() > 3);
        let a = bytes[0];
        let b = bytes[1];
        let c = bytes[2];
        let dir = self.path.join(format!("{}/{}/{}", a, b, c));
        let filename = dir.join(format!("{}", id));
        (dir, filename)
    }
}

/// A remote chunk store.
pub struct RemoteStore {
    client: reqwest::Client,
    base_url: String,
}

impl RemoteStore {
    fn new(config: &ClientConfig) -> Result<Self, StoreError> {
        info!("creating remote store with config: {:#?}", config);

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(!config.verify_tls_cert)
            .build()
            .map_err(StoreError::ReqwestError)?;
        Ok(Self {
            client,
            base_url: config.server_url.to_string(),
        })
    }

    async fn find_by_label(&self, meta: &ChunkMeta) -> Result<Vec<ChunkId>, StoreError> {
        let body = match self.get_helper("", &[("label", meta.label())]).await {
            Ok((_, body)) => body,
            Err(err) => return Err(err),
        };

        let hits: HashMap<String, ChunkMeta> =
            serde_json::from_slice(&body).map_err(StoreError::JsonParse)?;
        let ids = hits.iter().map(|(id, _)| ChunkId::recreate(id)).collect();
        Ok(ids)
    }

    async fn put(&self, chunk: Vec<u8>, meta: &ChunkMeta) -> Result<ChunkId, StoreError> {
        let res = self
            .client
            .post(&self.chunks_url())
            .header("chunk-meta", meta.to_json())
            .body(chunk)
            .send()
            .await
            .map_err(StoreError::ReqwestError)?;
        let res: HashMap<String, String> = res.json().await.map_err(StoreError::ReqwestError)?;
        debug!("upload_chunk: res={:?}", res);
        let chunk_id = if let Some(chunk_id) = res.get("chunk_id") {
            debug!("upload_chunk: id={}", chunk_id);
            chunk_id.parse().unwrap()
        } else {
            return Err(StoreError::NoCreatedChunkId);
        };
        info!("uploaded_chunk {}", chunk_id);
        Ok(chunk_id)
    }

    async fn get(&self, id: &ChunkId) -> Result<(Vec<u8>, ChunkMeta), StoreError> {
        let (headers, body) = self.get_helper(&format!("/{}", id), &[]).await?;
        let meta = self.get_chunk_meta_header(id, &headers)?;
        Ok((body, meta))
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn chunks_url(&self) -> String {
        format!("{}/v1/chunks", self.base_url())
    }

    async fn get_helper(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<(HeaderMap, Vec<u8>), StoreError> {
        let url = format!("{}{}", &self.chunks_url(), path);
        info!("GET {}", url);

        // Build HTTP request structure.
        let req = self
            .client
            .get(&url)
            .query(query)
            .build()
            .map_err(StoreError::ReqwestError)?;

        // Make HTTP request.
        let res = self
            .client
            .execute(req)
            .await
            .map_err(StoreError::ReqwestError)?;

        // Did it work?
        if res.status() != 200 {
            return Err(StoreError::NotFound(path.to_string()));
        }

        // Return headers and body.
        let headers = res.headers().clone();
        let body = res.bytes().await.map_err(StoreError::ReqwestError)?;
        let body = body.to_vec();
        Ok((headers, body))
    }

    fn get_chunk_meta_header(
        &self,
        chunk_id: &ChunkId,
        headers: &HeaderMap,
    ) -> Result<ChunkMeta, StoreError> {
        let meta = headers.get("chunk-meta");

        if meta.is_none() {
            let err = StoreError::NoChunkMeta(chunk_id.clone());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err);
        }

        let meta = meta
            .unwrap()
            .to_str()
            .map_err(StoreError::MetaHeaderToString)?;
        let meta: ChunkMeta = serde_json::from_str(meta).map_err(StoreError::JsonParse)?;

        Ok(meta)
    }
}

/// Possible errors from using a ChunkStore.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// FIXME
    #[error("FIXME")]
    FIXME,

    /// Error from a chunk index.
    #[error(transparent)]
    Index(#[from] IndexError),

    /// An error from the HTTP library.
    #[error("error from reqwest library: {0}")]
    ReqwestError(reqwest::Error),

    /// Client configuration is wrong.
    #[error(transparent)]
    ClientConfigError(#[from] ClientConfigError),

    /// Server claims to not have an entity.
    #[error("Server does not have {0}")]
    NotFound(String),

    /// Server didn't give us a chunk's metadata.
    #[error("Server response did not have a 'chunk-meta' header for chunk {0}")]
    NoChunkMeta(ChunkId),

    /// An error with the `chunk-meta` header.
    #[error("couldn't convert response chunk-meta header to string: {0}")]
    MetaHeaderToString(reqwest::header::ToStrError),

    /// Error parsing JSON.
    #[error("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),

    /// An error creating chunk directory.
    #[error("Failed to create chunk directory {0}")]
    ChunkMkdir(PathBuf, #[source] std::io::Error),

    /// An error writing a chunk file.
    #[error("Failed to write chunk {0}")]
    WriteChunk(PathBuf, #[source] std::io::Error),

    /// An error reading a chunk file.
    #[error("Failed to read chunk {0}")]
    ReadChunk(PathBuf, #[source] std::io::Error),

    /// No chunk id for uploaded chunk.
    #[error("Server response claimed it had created a chunk, but lacked chunk id")]
    NoCreatedChunkId,
}

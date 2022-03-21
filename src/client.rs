//! Client to the Obnam server HTTP API.

use crate::chunk::{
    ClientTrust, ClientTrustError, DataChunk, GenerationChunk, GenerationChunkError,
};
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::cipher::{CipherEngine, CipherError};
use crate::config::{ClientConfig, ClientConfigError};
use crate::generation::{FinishedGeneration, GenId, LocalGeneration, LocalGenerationError};
use crate::genlist::GenerationList;

use log::{debug, error, info};
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

/// Possible errors when using the server API.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// No chunk id for uploaded chunk.
    #[error("Server response claimed it had created a chunk, but lacked chunk id")]
    NoCreatedChunkId,

    /// Server claims to not have an entity.
    #[error("Server does not have {0}")]
    NotFound(String),

    /// Server does not have a chunk.
    #[error("Server does not have chunk {0}")]
    ChunkNotFound(ChunkId),

    /// Server does not have generation.
    #[error("Server does not have generation {0}")]
    GenerationNotFound(ChunkId),

    /// Server didn't give us a chunk's metadata.
    #[error("Server response did not have a 'chunk-meta' header for chunk {0}")]
    NoChunkMeta(ChunkId),

    /// Chunk has wrong checksum and may be corrupted.
    #[error("Wrong checksum for chunk {0}, got {1}, expected {2}")]
    WrongChecksum(ChunkId, String, String),

    /// Client configuration is wrong.
    #[error(transparent)]
    ClientConfigError(#[from] ClientConfigError),

    /// An error encrypting or decrypting chunks.
    #[error(transparent)]
    CipherError(#[from] CipherError),

    /// An error regarding generation chunks.
    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),

    /// An error regarding client trust.
    #[error(transparent)]
    ClientTrust(#[from] ClientTrustError),

    /// An error using a backup's local metadata.
    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    /// An error with the `chunk-meta` header.
    #[error("couldn't convert response chunk-meta header to string: {0}")]
    MetaHeaderToString(reqwest::header::ToStrError),

    /// An error from the HTTP library.
    #[error("error from reqwest library: {0}")]
    ReqwestError(reqwest::Error),

    /// Couldn't look up a chunk via checksum.
    #[error("lookup by chunk checksum failed: {0}")]
    ChunkExists(reqwest::Error),

    /// Error parsing JSON.
    #[error("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),

    /// Error generating JSON.
    #[error("failed to generate JSON: {0}")]
    JsonGenerate(serde_json::Error),

    /// Error parsing YAML.
    #[error("failed to parse YAML: {0}")]
    YamlParse(serde_yaml::Error),

    /// Failed to open a file.
    #[error("failed to open file {0}: {1}")]
    FileOpen(PathBuf, std::io::Error),

    /// Failed to create a file.
    #[error("failed to create file {0}: {1}")]
    FileCreate(PathBuf, std::io::Error),

    /// Failed to write a file.
    #[error("failed to write to file {0}: {1}")]
    FileWrite(PathBuf, std::io::Error),
}

/// Client for the Obnam server HTTP API.
pub struct BackupClient {
    client: reqwest::Client,
    base_url: String,
    cipher: CipherEngine,
}

impl BackupClient {
    /// Create a new backup client.
    pub fn new(config: &ClientConfig) -> Result<Self, ClientError> {
        info!("creating backup client with config: {:#?}", config);

        let pass = config.passwords()?;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(!config.verify_tls_cert)
            .build()
            .map_err(ClientError::ReqwestError)?;
        Ok(Self {
            client,
            base_url: config.server_url.to_string(),
            cipher: CipherEngine::new(&pass),
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn chunks_url(&self) -> String {
        format!("{}/chunks", self.base_url())
    }

    /// Does the server have a chunk?
    pub async fn has_chunk(&self, meta: &ChunkMeta) -> Result<Option<ChunkId>, ClientError> {
        let body = match self.get("", &[("label", meta.label())]).await {
            Ok((_, body)) => body,
            Err(err) => return Err(err),
        };

        let hits: HashMap<String, ChunkMeta> =
            serde_json::from_slice(&body).map_err(ClientError::JsonParse)?;
        let mut iter = hits.iter();
        let has = if let Some((chunk_id, _)) = iter.next() {
            Some(chunk_id.into())
        } else {
            None
        };

        Ok(has)
    }

    /// Upload a data chunk to the server.
    pub async fn upload_chunk(&self, chunk: DataChunk) -> Result<ChunkId, ClientError> {
        let enc = self.cipher.encrypt_chunk(&chunk)?;
        let res = self
            .client
            .post(&self.chunks_url())
            .header("chunk-meta", chunk.meta().to_json())
            .body(enc.ciphertext().to_vec())
            .send()
            .await
            .map_err(ClientError::ReqwestError)?;
        debug!("upload_chunk: res={:?}", res);
        let res: HashMap<String, String> = res.json().await.map_err(ClientError::ReqwestError)?;
        let chunk_id = if let Some(chunk_id) = res.get("chunk_id") {
            debug!("upload_chunk: id={}", chunk_id);
            chunk_id.parse().unwrap()
        } else {
            return Err(ClientError::NoCreatedChunkId);
        };
        info!("uploaded_chunk {}", chunk_id);
        Ok(chunk_id)
    }

    /// Get current client trust chunk from repository, if there is one.
    pub async fn get_client_trust(&self) -> Result<Option<ClientTrust>, ClientError> {
        let ids = self.find_client_trusts().await?;
        let mut latest: Option<ClientTrust> = None;
        for id in ids {
            let chunk = self.fetch_chunk(&id).await?;
            let new = ClientTrust::from_data_chunk(&chunk)?;
            if let Some(t) = &latest {
                if new.timestamp() > t.timestamp() {
                    latest = Some(new);
                }
            } else {
                latest = Some(new);
            }
        }
        Ok(latest)
    }

    async fn find_client_trusts(&self) -> Result<Vec<ChunkId>, ClientError> {
        let body = match self.get("", &[("label", "client-trust")]).await {
            Ok((_, body)) => body,
            Err(err) => return Err(err),
        };

        let hits: HashMap<String, ChunkMeta> =
            serde_json::from_slice(&body).map_err(ClientError::JsonParse)?;
        let ids = hits.iter().map(|(id, _)| id.into()).collect();
        Ok(ids)
    }

    /// List backup generations known by the server.
    pub fn list_generations(&self, trust: &ClientTrust) -> GenerationList {
        let finished = trust
            .backups()
            .iter()
            .map(|id| FinishedGeneration::new(&format!("{}", id), ""))
            .collect();
        GenerationList::new(finished)
    }

    /// Fetch a data chunk from the server, given the chunk identifier.
    pub async fn fetch_chunk(&self, chunk_id: &ChunkId) -> Result<DataChunk, ClientError> {
        let (headers, body) = self.get(&format!("/{}", chunk_id), &[]).await?;
        let meta = self.get_chunk_meta_header(chunk_id, &headers)?;

        let meta_bytes = meta.to_json_vec();
        let chunk = self.cipher.decrypt_chunk(&body, &meta_bytes)?;

        Ok(chunk)
    }

    async fn fetch_generation_chunk(&self, gen_id: &GenId) -> Result<GenerationChunk, ClientError> {
        let chunk = self.fetch_chunk(gen_id.as_chunk_id()).await?;
        let gen = GenerationChunk::from_data_chunk(&chunk)?;
        Ok(gen)
    }

    /// Fetch a backup generation's metadata, given it's identifier.
    pub async fn fetch_generation(
        &self,
        gen_id: &GenId,
        dbname: &Path,
    ) -> Result<LocalGeneration, ClientError> {
        let gen = self.fetch_generation_chunk(gen_id).await?;

        // Fetch the SQLite file, storing it in the named file.
        let mut dbfile = File::create(&dbname)
            .map_err(|err| ClientError::FileCreate(dbname.to_path_buf(), err))?;
        for id in gen.chunk_ids() {
            let chunk = self.fetch_chunk(id).await?;
            dbfile
                .write_all(chunk.data())
                .map_err(|err| ClientError::FileWrite(dbname.to_path_buf(), err))?;
        }
        info!("downloaded generation to {}", dbname.display());

        let gen = LocalGeneration::open(dbname)?;
        Ok(gen)
    }

    async fn get(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<(HeaderMap, Vec<u8>), ClientError> {
        let url = format!("{}{}", &self.chunks_url(), path);
        info!("GET {}", url);

        // Build HTTP request structure.
        let req = self
            .client
            .get(&url)
            .query(query)
            .build()
            .map_err(ClientError::ReqwestError)?;

        // Make HTTP request.
        let res = self
            .client
            .execute(req)
            .await
            .map_err(ClientError::ReqwestError)?;

        // Did it work?
        if res.status() != 200 {
            return Err(ClientError::NotFound(path.to_string()));
        }

        // Return headers and body.
        let headers = res.headers().clone();
        let body = res.bytes().await.map_err(ClientError::ReqwestError)?;
        let body = body.to_vec();
        Ok((headers, body))
    }

    fn get_chunk_meta_header(
        &self,
        chunk_id: &ChunkId,
        headers: &HeaderMap,
    ) -> Result<ChunkMeta, ClientError> {
        let meta = headers.get("chunk-meta");

        if meta.is_none() {
            let err = ClientError::NoChunkMeta(chunk_id.clone());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err);
        }

        let meta = meta
            .unwrap()
            .to_str()
            .map_err(ClientError::MetaHeaderToString)?;
        let meta: ChunkMeta = serde_json::from_str(meta).map_err(ClientError::JsonParse)?;

        Ok(meta)
    }
}

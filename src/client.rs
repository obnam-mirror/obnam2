//! Client to the Obnam server HTTP API.

use crate::chunk::{
    ClientTrust, ClientTrustError, DataChunk, GenerationChunk, GenerationChunkError,
};
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::chunkstore::{ChunkStore, StoreError};
use crate::cipher::{CipherEngine, CipherError};
use crate::config::{ClientConfig, ClientConfigError};
use crate::generation::{FinishedGeneration, GenId, LocalGeneration, LocalGenerationError};
use crate::genlist::GenerationList;
use crate::label::Label;

use log::{error, info};
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

    /// Error from a chunk store.
    #[error(transparent)]
    ChunkStore(#[from] StoreError),
}

/// Client for the Obnam server HTTP API.
pub struct BackupClient {
    store: ChunkStore,
    cipher: CipherEngine,
}

impl BackupClient {
    /// Create a new backup client.
    pub fn new(config: &ClientConfig) -> Result<Self, ClientError> {
        info!("creating backup client with config: {:#?}", config);
        let pass = config.passwords()?;
        Ok(Self {
            store: ChunkStore::remote(config)?,
            cipher: CipherEngine::new(&pass),
        })
    }

    /// Does the server have a chunk?
    pub async fn has_chunk(&self, meta: &ChunkMeta) -> Result<Option<ChunkId>, ClientError> {
        let mut ids = self.store.find_by_label(meta).await?;
        Ok(ids.pop())
    }

    /// Upload a data chunk to the server.
    pub async fn upload_chunk(&mut self, chunk: DataChunk) -> Result<ChunkId, ClientError> {
        let enc = self.cipher.encrypt_chunk(&chunk)?;
        let data = enc.ciphertext().to_vec();
        let id = self.store.put(data, chunk.meta()).await?;
        Ok(id)
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
        let label = Label::literal("client-trust");
        let meta = ChunkMeta::new(&label);
        let ids = self.store.find_by_label(&meta).await?;
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
        let (body, meta) = self.store.get(chunk_id).await?;
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
        let mut dbfile = File::create(dbname)
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
}

use crate::checksummer::sha256;
use crate::chunk::DataChunk;
use crate::chunk::{GenerationChunk, GenerationChunkError};
use crate::chunker::{Chunker, ChunkerError};
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::config::ClientConfig;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::{FinishedGeneration, LocalGeneration, LocalGenerationError};
use crate::genlist::GenerationList;

use chrono::{DateTime, Local};
use log::{debug, error, info, trace};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Server response claimed it had created a chunk, but lacked chunk id")]
    NoCreatedChunkId,

    #[error("Server does not have chunk {0}")]
    ChunkNotFound(String),

    #[error("Server does not have generation {0}")]
    GenerationNotFound(String),

    #[error("Server response did not have a 'chunk-meta' header for chunk {0}")]
    NoChunkMeta(ChunkId),

    #[error("Wrong checksum for chunk {0}, got {1}, expected {2}")]
    WrongChecksum(ChunkId, String, String),

    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    #[error(transparent)]
    ChunkerError(#[from] ChunkerError),

    #[error("couldn't convert response chunk-meta header to string: {0}")]
    MetaHeaderToString(reqwest::header::ToStrError),

    #[error("error from reqwest library: {0}")]
    ReqwestError(reqwest::Error),

    #[error("lookup by chunk checksum failed: {0}")]
    ChunkExists(reqwest::Error),

    #[error("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),

    #[error("failed to generate JSON: {0}")]
    JsonGenerate(serde_json::Error),

    #[error("failed to parse YAML: {0}")]
    YamlParse(serde_yaml::Error),

    #[error("failed to open file {0}: {1}")]
    FileOpen(PathBuf, std::io::Error),

    #[error("failed to create file {0}: {1}")]
    FileCreate(PathBuf, std::io::Error),

    #[error("failed to write to file {0}: {1}")]
    FileWrite(PathBuf, std::io::Error),
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct BackupClient {
    chunk_client: ChunkClient,
}

impl BackupClient {
    pub fn new(config: &ClientConfig) -> ClientResult<Self> {
        info!("creating backup client with config: {:#?}", config);
        Ok(Self {
            chunk_client: ChunkClient::new(config)?,
        })
    }

    pub fn upload_filesystem_entry(
        &self,
        e: &FilesystemEntry,
        size: usize,
    ) -> ClientResult<Vec<ChunkId>> {
        let path = e.pathbuf();
        info!("uploading {:?}", path);
        let ids = match e.kind() {
            FilesystemKind::Regular => self.read_file(&path, size)?,
            FilesystemKind::Directory => vec![],
            FilesystemKind::Symlink => vec![],
            FilesystemKind::Socket => vec![],
            FilesystemKind::Fifo => vec![],
        };
        info!("upload OK for {:?}", path);
        Ok(ids)
    }

    pub fn upload_generation(&self, filename: &Path, size: usize) -> ClientResult<ChunkId> {
        info!("upload SQLite {}", filename.display());
        let ids = self.read_file(filename, size)?;
        let gen = GenerationChunk::new(ids);
        let data = gen.to_data_chunk()?;
        let meta = ChunkMeta::new_generation(&sha256(data.data()), &current_timestamp());
        let gen_id = self.upload_gen_chunk(meta.clone(), gen)?;
        info!("uploaded generation {}, meta {:?}", gen_id, meta);
        Ok(gen_id)
    }

    fn read_file(&self, filename: &Path, size: usize) -> ClientResult<Vec<ChunkId>> {
        info!("upload file {}", filename.display());
        let file = std::fs::File::open(filename)
            .map_err(|err| ClientError::FileOpen(filename.to_path_buf(), err))?;
        let chunker = Chunker::new(size, file, filename);
        let chunk_ids = self.upload_new_file_chunks(chunker)?;
        Ok(chunk_ids)
    }

    pub fn has_chunk(&self, meta: &ChunkMeta) -> ClientResult<Option<ChunkId>> {
        self.chunk_client.has_chunk(meta)
    }

    pub fn upload_chunk(&self, meta: ChunkMeta, chunk: DataChunk) -> ClientResult<ChunkId> {
        self.chunk_client.upload_chunk(meta, chunk)
    }

    pub fn upload_gen_chunk(&self, meta: ChunkMeta, gen: GenerationChunk) -> ClientResult<ChunkId> {
        let data = gen.to_data_chunk()?;
        self.upload_chunk(meta, data)
    }

    pub fn upload_new_file_chunks(&self, chunker: Chunker) -> ClientResult<Vec<ChunkId>> {
        let mut chunk_ids = vec![];
        for item in chunker {
            let (meta, chunk) = item?;
            if let Some(chunk_id) = self.has_chunk(&meta)? {
                chunk_ids.push(chunk_id.clone());
                info!("reusing existing chunk {}", chunk_id);
            } else {
                let chunk_id = self.upload_chunk(meta, chunk)?;
                chunk_ids.push(chunk_id.clone());
                info!("created new chunk {}", chunk_id);
            }
        }

        Ok(chunk_ids)
    }

    pub fn list_generations(&self) -> ClientResult<GenerationList> {
        self.chunk_client.list_generations()
    }

    pub fn fetch_chunk(&self, chunk_id: &ChunkId) -> ClientResult<DataChunk> {
        self.chunk_client.fetch_chunk(chunk_id)
    }

    fn fetch_generation_chunk(&self, gen_id: &str) -> ClientResult<GenerationChunk> {
        let chunk_id = ChunkId::recreate(gen_id);
        let chunk = self.fetch_chunk(&chunk_id)?;
        let gen = GenerationChunk::from_data_chunk(&chunk)?;
        Ok(gen)
    }

    pub fn fetch_generation(&self, gen_id: &str, dbname: &Path) -> ClientResult<LocalGeneration> {
        let gen = self.fetch_generation_chunk(gen_id)?;

        // Fetch the SQLite file, storing it in the named file.
        let mut dbfile = File::create(&dbname)
            .map_err(|err| ClientError::FileCreate(dbname.to_path_buf(), err))?;
        for id in gen.chunk_ids() {
            let chunk = self.fetch_chunk(id)?;
            dbfile
                .write_all(chunk.data())
                .map_err(|err| ClientError::FileWrite(dbname.to_path_buf(), err))?;
        }
        info!("downloaded generation to {}", dbname.display());

        let gen = LocalGeneration::open(dbname)?;
        Ok(gen)
    }
}

pub struct ChunkClient {
    client: Client,
    base_url: String,
}

impl ChunkClient {
    pub fn new(config: &ClientConfig) -> ClientResult<Self> {
        let config = config.config();
        let client = Client::builder()
            .danger_accept_invalid_certs(!config.verify_tls_cert)
            .build()
            .map_err(ClientError::ReqwestError)?;
        Ok(Self {
            client,
            base_url: config.server_url.to_string(),
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn chunks_url(&self) -> String {
        format!("{}/chunks", self.base_url())
    }

    pub fn has_chunk(&self, meta: &ChunkMeta) -> ClientResult<Option<ChunkId>> {
        trace!("has_chunk: url={:?}", self.base_url());
        let req = self
            .client
            .get(&self.chunks_url())
            .query(&[("sha256", meta.sha256())])
            .build()
            .map_err(ClientError::ReqwestError)?;

        let res = self.client.execute(req).map_err(ClientError::ChunkExists)?;
        debug!("has_chunk: status={}", res.status());
        let has = if res.status() != 200 {
            debug!("has_chunk: error from server");
            None
        } else {
            let text = res.text().map_err(ClientError::ReqwestError)?;
            debug!("has_chunk: text={:?}", text);
            let hits: HashMap<String, ChunkMeta> =
                serde_json::from_str(&text).map_err(ClientError::JsonParse)?;
            debug!("has_chunk: hits={:?}", hits);
            let mut iter = hits.iter();
            if let Some((chunk_id, _)) = iter.next() {
                debug!("has_chunk: chunk_id={:?}", chunk_id);
                Some(chunk_id.into())
            } else {
                None
            }
        };

        info!("has_chunk result: {:?}", has);
        Ok(has)
    }

    pub fn upload_chunk(&self, meta: ChunkMeta, chunk: DataChunk) -> ClientResult<ChunkId> {
        let res = self
            .client
            .post(&self.chunks_url())
            .header("chunk-meta", meta.to_json())
            .body(chunk.data().to_vec())
            .send()
            .map_err(ClientError::ReqwestError)?;
        debug!("upload_chunk: res={:?}", res);
        let res: HashMap<String, String> = res.json().map_err(ClientError::ReqwestError)?;
        let chunk_id = if let Some(chunk_id) = res.get("chunk_id") {
            debug!("upload_chunk: id={}", chunk_id);
            chunk_id.parse().unwrap()
        } else {
            return Err(ClientError::NoCreatedChunkId);
        };
        info!("uploaded_chunk {} meta {:?}", chunk_id, meta);
        Ok(chunk_id)
    }

    pub fn list_generations(&self) -> ClientResult<GenerationList> {
        let url = format!("{}?generation=true", &self.chunks_url());
        trace!("list_generations: url={:?}", url);
        let req = self
            .client
            .get(&url)
            .build()
            .map_err(ClientError::ReqwestError)?;
        let res = self
            .client
            .execute(req)
            .map_err(ClientError::ReqwestError)?;
        debug!("list_generations: status={}", res.status());
        let body = res.bytes().map_err(ClientError::ReqwestError)?;
        debug!("list_generations: body={:?}", body);
        let map: HashMap<String, ChunkMeta> =
            serde_yaml::from_slice(&body).map_err(ClientError::YamlParse)?;
        debug!("list_generations: map={:?}", map);
        let finished = map
            .iter()
            .map(|(id, meta)| FinishedGeneration::new(id, meta.ended().map_or("", |s| s)))
            .collect();
        Ok(GenerationList::new(finished))
    }

    pub fn fetch_chunk(&self, chunk_id: &ChunkId) -> ClientResult<DataChunk> {
        info!("fetch chunk {}", chunk_id);

        let url = format!("{}/{}", &self.chunks_url(), chunk_id);
        let req = self
            .client
            .get(&url)
            .build()
            .map_err(ClientError::ReqwestError)?;
        let res = self
            .client
            .execute(req)
            .map_err(ClientError::ReqwestError)?;
        if res.status() != 200 {
            let err = ClientError::ChunkNotFound(chunk_id.to_string());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err);
        }

        let headers = res.headers();
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
        debug!("fetching chunk {}: meta={:?}", chunk_id, meta);
        let meta: ChunkMeta = serde_json::from_str(meta).map_err(ClientError::JsonParse)?;
        debug!("fetching chunk {}: meta={:?}", chunk_id, meta);

        let body = res.bytes().map_err(ClientError::ReqwestError)?;
        let body = body.to_vec();
        let actual = sha256(&body);
        if actual != meta.sha256() {
            let err =
                ClientError::WrongChecksum(chunk_id.clone(), actual, meta.sha256().to_string());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err);
        }

        let chunk: DataChunk = DataChunk::new(body);

        Ok(chunk)
    }
}

fn current_timestamp() -> String {
    let now: DateTime<Local> = Local::now();
    format!("{}", now.format("%Y-%m-%d %H:%M:%S.%f %z"))
}

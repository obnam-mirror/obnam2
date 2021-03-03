use crate::checksummer::sha256;
use crate::chunk::DataChunk;
use crate::chunk::{GenerationChunk, GenerationChunkError};
use crate::chunker::{Chunker, ChunkerError};
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::{FinishedGeneration, LocalGeneration, LocalGenerationError};
use crate::genlist::GenerationList;

use bytesize::MIB;
use chrono::{DateTime, Local};
use log::{debug, error, info, trace};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

const DEFAULT_CHUNK_SIZE: usize = MIB as usize;
const DEVNULL: &str = "/dev/null";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct TentativeClientConfig {
    server_url: String,
    verify_tls_cert: Option<bool>,
    chunk_size: Option<usize>,
    roots: Vec<PathBuf>,
    log: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClientConfig {
    pub server_url: String,
    pub verify_tls_cert: bool,
    pub chunk_size: usize,
    pub roots: Vec<PathBuf>,
    pub log: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientConfigError {
    #[error("server_url is empty")]
    ServerUrlIsEmpty,

    #[error("No backup roots in config; at least one is needed")]
    NoBackupRoot,

    #[error("server URL doesn't use https: {0}")]
    NotHttps(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeYamlError(#[from] serde_yaml::Error),
}

pub type ClientConfigResult<T> = Result<T, ClientConfigError>;

impl ClientConfig {
    pub fn read_config(filename: &Path) -> ClientConfigResult<Self> {
        trace!("read_config: filename={:?}", filename);
        let config = std::fs::read_to_string(filename)?;
        let tentative: TentativeClientConfig = serde_yaml::from_str(&config)?;

        let config = ClientConfig {
            server_url: tentative.server_url,
            roots: tentative.roots,
            verify_tls_cert: tentative.verify_tls_cert.or(Some(false)).unwrap(),
            chunk_size: tentative.chunk_size.or(Some(DEFAULT_CHUNK_SIZE)).unwrap(),
            log: tentative.log.or(Some(PathBuf::from(DEVNULL))).unwrap(),
        };

        config.check()?;
        Ok(config)
    }

    fn check(&self) -> Result<(), ClientConfigError> {
        if self.server_url.is_empty() {
            return Err(ClientConfigError::ServerUrlIsEmpty);
        }
        if !self.server_url.starts_with("https://") {
            return Err(ClientConfigError::NotHttps(self.server_url.to_string()));
        }
        if self.roots.is_empty() {
            return Err(ClientConfigError::NoBackupRoot);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Server successful response to creating chunk lacked chunk id")]
    NoCreatedChunkId,

    #[error("Server does not have chunk {0}")]
    ChunkNotFound(String),

    #[error("Server does not have generation {0}")]
    GenerationNotFound(String),

    #[error(transparent)]
    GenerationChunkError(#[from] GenerationChunkError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    #[error(transparent)]
    ChunkerError(#[from] ChunkerError),

    #[error("Server response did not have a 'chunk-meta' header for chunk {0}")]
    NoChunkMeta(ChunkId),

    #[error("Wrong checksum for chunk {0}, got {1}, expected {2}")]
    WrongChecksum(ChunkId, String, String),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    ReqwestToStrError(#[from] reqwest::header::ToStrError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    SerdeYamlError(#[from] serde_yaml::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct BackupClient {
    client: Client,
    base_url: String,
}

impl BackupClient {
    pub fn new(config: &ClientConfig) -> ClientResult<Self> {
        info!("creating backup client with config: {:#?}", config);
        let client = Client::builder()
            .danger_accept_invalid_certs(!config.verify_tls_cert)
            .build()?;
        Ok(Self {
            client,
            base_url: config.server_url.to_string(),
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
        let file = std::fs::File::open(filename)?;
        let chunker = Chunker::new(size, file);
        let chunk_ids = self.upload_new_file_chunks(chunker)?;
        Ok(chunk_ids)
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
            .build()?;

        let res = self.client.execute(req)?;
        debug!("has_chunk: status={}", res.status());
        let has = if res.status() != 200 {
            debug!("has_chunk: error from server");
            None
        } else {
            let text = res.text()?;
            debug!("has_chunk: text={:?}", text);
            let hits: HashMap<String, ChunkMeta> = serde_json::from_str(&text)?;
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
            .send()?;
        debug!("upload_chunk: res={:?}", res);
        let res: HashMap<String, String> = res.json()?;
        let chunk_id = if let Some(chunk_id) = res.get("chunk_id") {
            debug!("upload_chunk: id={}", chunk_id);
            chunk_id.parse().unwrap()
        } else {
            return Err(ClientError::NoCreatedChunkId.into());
        };
        info!("uploaded_chunk {} meta {:?}", chunk_id, meta);
        Ok(chunk_id)
    }

    pub fn upload_gen_chunk(&self, meta: ChunkMeta, gen: GenerationChunk) -> ClientResult<ChunkId> {
        let res = self
            .client
            .post(&self.chunks_url())
            .header("chunk-meta", meta.to_json())
            .body(serde_json::to_string(&gen)?)
            .send()?;
        debug!("upload_chunk: res={:?}", res);
        let res: HashMap<String, String> = res.json()?;
        let chunk_id = if let Some(chunk_id) = res.get("chunk_id") {
            debug!("upload_chunk: id={}", chunk_id);
            chunk_id.parse().unwrap()
        } else {
            return Err(ClientError::NoCreatedChunkId.into());
        };
        info!("uploaded_generation chunk {}", chunk_id);
        Ok(chunk_id)
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
        let url = format!("{}?generation=true", &self.chunks_url());
        trace!("list_generations: url={:?}", url);
        let req = self.client.get(&url).build()?;
        let res = self.client.execute(req)?;
        debug!("list_generations: status={}", res.status());
        let body = res.bytes()?;
        debug!("list_generations: body={:?}", body);
        let map: HashMap<String, ChunkMeta> = serde_yaml::from_slice(&body)?;
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
        let req = self.client.get(&url).build()?;
        let res = self.client.execute(req)?;
        if res.status() != 200 {
            let err = ClientError::ChunkNotFound(chunk_id.to_string());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err.into());
        }

        let headers = res.headers();
        let meta = headers.get("chunk-meta");
        if meta.is_none() {
            let err = ClientError::NoChunkMeta(chunk_id.clone());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err.into());
        }
        let meta = meta.unwrap().to_str()?;
        debug!("fetching chunk {}: meta={:?}", chunk_id, meta);
        let meta: ChunkMeta = serde_json::from_str(meta)?;
        debug!("fetching chunk {}: meta={:?}", chunk_id, meta);

        let body = res.bytes()?;
        let body = body.to_vec();
        let actual = sha256(&body);
        if actual != meta.sha256() {
            let err =
                ClientError::WrongChecksum(chunk_id.clone(), actual, meta.sha256().to_string());
            error!("fetching chunk {} failed: {}", chunk_id, err);
            return Err(err.into());
        }

        let chunk: DataChunk = DataChunk::new(body);

        Ok(chunk)
    }

    fn fetch_generation_chunk(&self, gen_id: &str) -> ClientResult<GenerationChunk> {
        let chunk_id = ChunkId::from_str(gen_id);
        let chunk = self.fetch_chunk(&chunk_id)?;
        let gen = GenerationChunk::from_data_chunk(&chunk)?;
        Ok(gen)
    }

    pub fn fetch_generation(&self, gen_id: &str, dbname: &Path) -> ClientResult<LocalGeneration> {
        let gen = self.fetch_generation_chunk(gen_id)?;

        // Fetch the SQLite file, storing it in the named file.
        let mut dbfile = File::create(&dbname)?;
        for id in gen.chunk_ids() {
            let chunk = self.fetch_chunk(id)?;
            dbfile.write_all(chunk.data())?;
        }
        info!("downloaded generation to {}", dbname.display());

        let gen = LocalGeneration::open(dbname)?;
        Ok(gen)
    }
}

fn current_timestamp() -> String {
    let now: DateTime<Local> = Local::now();
    format!("{}", now.format("%Y-%m-%d %H:%M:%S.%f %z"))
}

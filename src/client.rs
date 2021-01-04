use crate::checksummer::sha256;
use crate::chunk::DataChunk;
use crate::chunk::GenerationChunk;
use crate::chunker::Chunker;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::{FinishedGeneration, LocalGeneration};
use crate::genlist::GenerationList;

use chrono::{DateTime, Local};
use log::{debug, error, info, trace};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct ClientConfig {
    pub server_url: String,
    pub root: PathBuf,
    pub log: Option<PathBuf>,
}

impl ClientConfig {
    pub fn read_config(filename: &Path) -> anyhow::Result<Self> {
        trace!("read_config: filename={:?}", filename);
        let config = std::fs::read_to_string(filename)?;
        let config = serde_yaml::from_str(&config)?;
        Ok(config)
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
}

pub struct BackupClient {
    client: Client,
    base_url: String,
}

impl BackupClient {
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }

    pub fn upload_filesystem_entry(
        &self,
        e: FilesystemEntry,
        size: usize,
    ) -> anyhow::Result<(FilesystemEntry, Vec<ChunkId>)> {
        debug!("entry: {:?}", e);
        let ids = match e.kind() {
            FilesystemKind::Regular => self.read_file(e.pathbuf(), size)?,
            FilesystemKind::Directory => vec![],
            FilesystemKind::Symlink => vec![],
        };
        Ok((e, ids))
    }

    pub fn upload_generation(&self, filename: &Path, size: usize) -> anyhow::Result<ChunkId> {
        let ids = self.read_file(filename.to_path_buf(), size)?;
        let gen = GenerationChunk::new(ids);
        let data = gen.to_data_chunk()?;
        let meta = ChunkMeta::new_generation(&sha256(data.data()), &current_timestamp());
        let gen_id = self.upload_gen_chunk(meta, gen)?;
        Ok(gen_id)
    }

    fn read_file(&self, filename: PathBuf, size: usize) -> anyhow::Result<Vec<ChunkId>> {
        info!("uploading {}", filename.display());
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

    pub fn has_chunk(&self, meta: &ChunkMeta) -> anyhow::Result<Option<ChunkId>> {
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

        Ok(has)
    }

    pub fn upload_chunk(&self, meta: ChunkMeta, chunk: DataChunk) -> anyhow::Result<ChunkId> {
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
        Ok(chunk_id)
    }

    pub fn upload_gen_chunk(
        &self,
        meta: ChunkMeta,
        gen: GenerationChunk,
    ) -> anyhow::Result<ChunkId> {
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
        Ok(chunk_id)
    }

    pub fn upload_new_file_chunks(&self, chunker: Chunker) -> anyhow::Result<Vec<ChunkId>> {
        let mut chunk_ids = vec![];
        for item in chunker {
            let (meta, chunk) = item?;
            if let Some(chunk_id) = self.has_chunk(&meta)? {
                chunk_ids.push(chunk_id);
            } else {
                let chunk_id = self.upload_chunk(meta, chunk)?;
                chunk_ids.push(chunk_id);
            }
        }

        Ok(chunk_ids)
    }

    pub fn list_generations(&self) -> anyhow::Result<GenerationList> {
        let url = format!("{}?generation=true", &self.chunks_url());
        trace!("list_generations: url={:?}", url);
        let req = self.client.get(&url).build()?;
        let res = self.client.execute(req)?;
        debug!("list_generations: status={}", res.status());
        let body = res.bytes()?;
        debug!("list_generationgs: body={:?}", body);
        let map: HashMap<String, ChunkMeta> = serde_yaml::from_slice(&body)?;
        debug!("list_generations: map={:?}", map);
        let finished = map
            .iter()
            .map(|(id, meta)| FinishedGeneration::new(id, meta.ended().map_or("", |s| s)))
            .collect();
        Ok(GenerationList::new(finished))
    }

    pub fn fetch_chunk(&self, chunk_id: &ChunkId) -> anyhow::Result<DataChunk> {
        let url = format!("{}/{}", &self.chunks_url(), chunk_id);
        trace!("fetch_chunk: url={:?}", url);
        let req = self.client.get(&url).build()?;
        let res = self.client.execute(req)?;
        debug!("fetch_chunk: status={}", res.status());
        if res.status() != 200 {
            return Err(ClientError::ChunkNotFound(chunk_id.to_string()).into());
        }

        let headers = res.headers();
        let meta = headers.get("chunk-meta");
        if meta.is_none() {
            return Err(ObnamError::NoChunkMeta(chunk_id.to_string()).into());
        }
        let meta = meta.unwrap().to_str()?;
        let meta: ChunkMeta = serde_json::from_str(meta)?;

        let body = res.bytes()?;
        let body = body.to_vec();
        let actual = sha256(&body);
        if actual != meta.sha256() {
            return Err(ObnamError::WrongChecksum(chunk_id.to_string()).into());
        }

        let chunk: DataChunk = DataChunk::new(body);

        Ok(chunk)
    }

    fn fetch_generation_chunk(&self, gen_id: &str) -> anyhow::Result<GenerationChunk> {
        let url = format!("{}/{}", &self.chunks_url(), gen_id);
        trace!("fetch_generation_chunk: url={:?}", url);
        let req = self.client.get(&url).build()?;
        let res = self.client.execute(req)?;
        debug!("fetch_generation_chunk: status={}", res.status());
        if res.status() != 200 {
            return Err(ClientError::GenerationNotFound(gen_id.to_string()).into());
        }

        let text = res.text()?;
        debug!("fetch_generation_chunk: text={:?}", text);
        let gen: GenerationChunk = serde_json::from_str(&text)?;
        debug!("fetch_generation_chunk: {:?}", gen);
        Ok(gen)
    }

    pub fn fetch_generation(&self, gen_id: &str, dbname: &Path) -> anyhow::Result<LocalGeneration> {
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

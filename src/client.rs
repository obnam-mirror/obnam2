use crate::chunk::DataChunk;
use crate::chunk::GenerationChunk;
use crate::chunker::Chunker;
use crate::chunkid::ChunkId;
use crate::chunkmeta::ChunkMeta;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use log::{debug, error, info, trace};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct ClientConfig {
    pub server_name: String,
    pub server_port: u16,
    pub dbname: PathBuf,
    pub root: PathBuf,
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
}

pub struct BackupClient {
    client: Client,
    base_url: String,
}

impl BackupClient {
    pub fn new(host: &str, port: u16) -> anyhow::Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let base_url = format!("http://{}:{}/chunks", host, port,);
        Ok(Self { client, base_url })
    }

    pub fn upload_filesystem_entry(
        &self,
        e: FilesystemEntry,
        size: usize,
    ) -> anyhow::Result<(FilesystemEntry, Vec<ChunkId>)> {
        let ids = match e.kind() {
            FilesystemKind::Regular => self.read_file(e.path(), size)?,
            FilesystemKind::Directory => vec![],
        };
        Ok((e, ids))
    }

    pub fn upload_generation(&self, filename: &Path, size: usize) -> anyhow::Result<ChunkId> {
        let ids = self.read_file(filename, size)?;
        let gen = GenerationChunk::new(ids);
        let meta = ChunkMeta::new_generation("checksum", "endtime");
        let gen_id = self.upload_gen_chunk(meta, gen)?;
        Ok(gen_id)
    }

    fn read_file(&self, filename: &Path, size: usize) -> anyhow::Result<Vec<ChunkId>> {
        info!("uploading {}", filename.display());
        let file = std::fs::File::open(filename)?;
        let chunker = Chunker::new(size, file);
        let chunk_ids = self.upload_new_file_chunks(chunker)?;
        Ok(chunk_ids)
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn has_chunk(&self, meta: &ChunkMeta) -> anyhow::Result<Option<ChunkId>> {
        trace!("has_chunk: url={:?}", self.base_url());
        let req = self
            .client
            .get(self.base_url())
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
            .post(self.base_url())
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
            .post(self.base_url())
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
}

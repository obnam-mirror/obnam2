// Read stdin, split into chunks, upload new chunks to chunk server.

use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, trace};
use obnam::chunk::{DataChunk, GenerationChunk};
use obnam::chunkid::ChunkId;
use obnam::chunkmeta::ChunkMeta;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
enum ClientError {
    #[error("Server successful response to creating chunk lacked chunk id")]
    NoCreatedChunkId,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();
    //    let pb = ProgressBar::new_spinner();
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_bar()
            .template("backing up:\n{bytes} ({bytes_per_sec}) {elapsed} {msg} {spinner}"),
    );

    info!("obnam-backup starts up");
    info!("config: {:?}", config);

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let mut chunk_ids = vec![];
    let mut total_bytes = 0;
    let mut new_chunks = 0;
    let mut dup_chunks = 0;
    let mut new_bytes = 0;
    let mut dup_bytes = 0;

    let stdin = std::io::stdin();
    let mut stdin = BufReader::new(stdin);
    loop {
        match read_chunk(&mut stdin)? {
            None => break,
            Some((meta, chunk)) => {
                let n = chunk.data().len() as u64;
                debug!("read {} bytes", n);
                total_bytes += n;
                pb.inc(n);
                if let Some(chunk_id) = has_chunk(&client, &config, &meta)? {
                    debug!("dup chunk: {}", chunk_id);
                    chunk_ids.push(chunk_id);
                    dup_chunks += 1;
                    dup_bytes += n;
                } else {
                    let chunk_id = upload_chunk(&client, &config, meta, chunk)?;
                    debug!("new chunk: {}", chunk_id);
                    chunk_ids.push(chunk_id);
                    new_chunks += 1;
                    new_bytes += n;
                }
            }
        }
    }

    let gen = GenerationChunk::new(chunk_ids);
    let gen_id = upload_gen(&client, &config, &gen)?;

    pb.finish();
    info!("read total {} bytes from stdin", total_bytes);
    info!("duplicate bytes: {}", dup_bytes);
    info!("duplicate chunks: {}", dup_chunks);
    info!("new bytes: {}", new_bytes);
    info!("new chunks: {}", new_chunks);
    info!("total chunks: {}", gen.len());
    info!("generation id: {}", gen_id);
    info!("obnam-backup finished OK");
    println!("backup OK: generation id: {}", gen_id);
    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server_name: String,
    pub server_port: u16,
}

impl Config {
    pub fn read_config(filename: &Path) -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(filename)?;
        let config: Config = serde_yaml::from_str(&config)?;
        Ok(config)
    }
}

fn read_chunk<H>(handle: &mut H) -> anyhow::Result<Option<(ChunkMeta, DataChunk)>>
where
    H: Read + BufRead,
{
    let mut buffer = [0; BUFFER_SIZE];
    let mut used = 0;

    loop {
        let n = handle.read(&mut buffer[used..])?;
        used += n;
        if n == 0 || used == BUFFER_SIZE {
            break;
        }
    }

    if used == 0 {
        return Ok(None);
    }

    let buffer = &buffer[..used];
    let mut hasher = Sha256::new();
    hasher.update(buffer);
    let hash = hasher.finalize();
    let hash = format!("{:x}", hash);
    let meta = ChunkMeta::new(&hash);

    let chunk = DataChunk::new(buffer.to_vec());
    Ok(Some((meta, chunk)))
}

fn upload_chunk(
    client: &reqwest::blocking::Client,
    config: &Config,
    meta: ChunkMeta,
    chunk: DataChunk,
) -> anyhow::Result<ChunkId> {
    let url = format!(
        "http://{}:{}/chunks",
        config.server_name, config.server_port
    );

    let res = client
        .post(&url)
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

fn upload_gen(
    client: &reqwest::blocking::Client,
    config: &Config,
    gen: &GenerationChunk,
) -> anyhow::Result<ChunkId> {
    let meta = ChunkMeta::new_generation("metasha", "ended-sometime");
    let chunk = gen.to_data_chunk()?;
    upload_chunk(client, config, meta, chunk)
}

fn has_chunk(
    client: &reqwest::blocking::Client,
    config: &Config,
    meta: &ChunkMeta,
) -> anyhow::Result<Option<ChunkId>> {
    let url = format!(
        "http://{}:{}/chunks",
        config.server_name, config.server_port,
    );

    trace!("has_chunk: url={:?}", url);
    let req = client
        .get(&url)
        .query(&[("sha256", meta.sha256())])
        .build()?;

    let res = client.execute(req)?;
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

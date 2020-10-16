// Fetch a backup generation's chunks, write to stdout.

use log::{debug, info, trace};
use obnam::chunk::{DataChunk, GenerationChunk};
use obnam::chunkid::ChunkId;
//use obnam::chunkmeta::ChunkMeta;
use serde::Deserialize;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, thiserror::Error)]
enum ClientError {
    #[error("Server does not have generation {0}")]
    GenerationNotFound(String),

    #[error("Server does not have chunk {0}")]
    ChunkNotFound(String),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    #[structopt()]
    gen_id: String,
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();

    info!("obnam-restore starts up");
    info!("opt: {:?}", opt);
    info!("config: {:?}", config);

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let mut stdout = stdout();

    let gen = fetch_generation(&client, &config, &opt.gen_id)?;
    debug!("gen: {:?}", gen);
    for id in gen.chunk_ids() {
        let chunk = fetch_chunk(&client, &config, id)?;
        debug!("got chunk: {}", id);
        stdout.write_all(chunk.data())?;
    }

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

fn fetch_generation(
    client: &reqwest::blocking::Client,
    config: &Config,
    gen_id: &str,
) -> anyhow::Result<GenerationChunk> {
    let url = format!(
        "http://{}:{}/chunks/{}",
        config.server_name, config.server_port, gen_id,
    );

    trace!("fetch_generation: url={:?}", url);
    let req = client.get(&url).build()?;

    let res = client.execute(req)?;
    debug!("fetch_generation: status={}", res.status());
    if res.status() != 200 {
        debug!("fetch_generation: error from server");
        return Err(ClientError::GenerationNotFound(gen_id.to_string()).into());
    }

    let text = res.text()?;
    debug!("fetch_generation: text={:?}", text);
    let gen = serde_json::from_str(&text)?;
    Ok(gen)
}

fn fetch_chunk(
    client: &reqwest::blocking::Client,
    config: &Config,
    chunk_id: &ChunkId,
) -> anyhow::Result<DataChunk> {
    let url = format!(
        "http://{}:{}/chunks/{}",
        config.server_name, config.server_port, chunk_id,
    );

    trace!("fetch_chunk: url={:?}", url);
    let req = client.get(&url).build()?;

    let res = client.execute(req)?;
    debug!("fetch_chunk: status={}", res.status());
    if res.status() != 200 {
        debug!("fetch_chunk: error from server");
        return Err(ClientError::ChunkNotFound(chunk_id.to_string()).into());
    }

    let body = res.bytes()?;
    Ok(DataChunk::new(body.to_vec()))
}

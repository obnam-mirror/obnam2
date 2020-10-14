// Read stdin, split into chunks, upload new chunks to chunk server.

use indicatif::{ProgressBar, ProgressStyle};
use obnam::chunk::DataChunk;
use obnam::chunkmeta::ChunkMeta;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_bar()
            .template("backing up:\n{bytes} ({bytes_per_sec}) {elapsed} {msg} {spinner}"),
    );

    println!("config: {:?}", config);

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let stdin = std::io::stdin();
    let mut stdin = BufReader::new(stdin);
    let mut dup = 0;
    loop {
        match read_chunk(&mut stdin)? {
            None => break,
            Some((meta, chunk)) => {
                let n = chunk.data().len() as u64;
                if !has_chunk(&client, &config, &meta)? {
                    pb.inc(n);
                    upload_chunk(&client, &config, meta, chunk)?;
                } else {
                    dup += n;
                }
            }
        }
    }
    pb.finish();
    println!(
        "read total {} bytes from stdin ({} dup)",
        pb.position(),
        dup
    );
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
) -> anyhow::Result<()> {
    let url = format!(
        "http://{}:{}/chunks",
        config.server_name, config.server_port
    );

    client
        .post(&url)
        .header("chunk-meta", meta.to_json())
        .body(chunk.data().to_vec())
        .send()?;
    Ok(())
}

fn has_chunk(
    client: &reqwest::blocking::Client,
    config: &Config,
    meta: &ChunkMeta,
) -> anyhow::Result<bool> {
    let url = format!(
        "http://{}:{}/chunks",
        config.server_name, config.server_port,
    );

    let req = client
        .get(&url)
        .query(&[("sha256", meta.sha256())])
        .build()?;

    let res = client.execute(req)?;
    let has = if res.status() != 200 {
        false
    } else {
        let text = res.text()?;
        let hits: HashMap<String, ChunkMeta> = serde_json::from_str(&text)?;
        !hits.is_empty()
    };

    Ok(has)
}

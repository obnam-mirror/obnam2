use log::{debug, info};
use obnam::client::BackupClient;
//use obnam::chunkmeta::ChunkMeta;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    #[structopt()]
    gen_id: String,

    #[structopt(parse(from_os_str))]
    dbname: PathBuf,
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();

    info!("obnam-restore starts up");
    info!("opt: {:?}", opt);
    info!("config: {:?}", config);

    let client = BackupClient::new(&config.server_name, config.server_port)?;
    let gen = client.fetch_generation(&opt.gen_id)?;
    debug!("gen: {:?}", gen);
    {
        let mut dbfile = File::create(&opt.dbname)?;
        for id in gen.chunk_ids() {
            let chunk = client.fetch_chunk(id)?;
            dbfile.write_all(chunk.data())?;
        }
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

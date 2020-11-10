use log::{debug, info};
use obnam::client::BackupClient;
use obnam::fsentry::{FilesystemEntry, FilesystemKind};
use obnam::generation::Generation;
//use obnam::chunkmeta::ChunkMeta;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();

    info!("obnam-restore starts up");
    info!("opt: {:?}", opt);
    info!("config: {:?}", config);

    let client = BackupClient::new(&config.server_name, config.server_port)?;
    let gen_chunk = client.fetch_generation(&opt.gen_id)?;
    debug!("gen: {:?}", gen_chunk);
    {
        let mut dbfile = File::create(&opt.dbname)?;
        for id in gen_chunk.chunk_ids() {
            let chunk = client.fetch_chunk(id)?;
            dbfile.write_all(chunk.data())?;
        }
    }
    info!("downloaded generation to {}", opt.dbname.display());

    let gen = Generation::open(&opt.dbname)?;
    for (fileid, entry) in gen.files()? {
        restore(&client, &gen, fileid, entry, &opt.to)?;
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    #[structopt()]
    gen_id: String,

    #[structopt(parse(from_os_str))]
    dbname: PathBuf,

    #[structopt(parse(from_os_str))]
    to: PathBuf,
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

fn restore(
    client: &BackupClient,
    gen: &Generation,
    fileid: u64,
    entry: FilesystemEntry,
    to: &Path,
) -> anyhow::Result<()> {
    println!("restoring {}:{}", fileid, entry.path().display());

    let path = if entry.path().is_absolute() {
        entry.path().strip_prefix("/")?
    } else {
        entry.path()
    };
    let to = to.join(path);
    debug!("  to: {}", to.display());

    match entry.kind() {
        FilesystemKind::Regular => restore_regular(client, &gen, &to, fileid, &entry)?,
        FilesystemKind::Directory => restore_directory(&to)?,
    }
    Ok(())
}

fn restore_directory(path: &Path) -> anyhow::Result<()> {
    debug!("restoring directory {}", path.display());
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn restore_regular(
    client: &BackupClient,
    gen: &Generation,
    path: &Path,
    fileid: u64,
    _entry: &FilesystemEntry,
) -> anyhow::Result<()> {
    debug!("restoring regular {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    std::fs::create_dir_all(parent)?;
    {
        let mut file = std::fs::File::create(path)?;
        for chunkid in gen.chunkids(fileid)? {
            let chunk = client.fetch_chunk(&chunkid)?;
            file.write_all(chunk.data())?;
        }
    }
    debug!("restored regular {}", path.display());
    Ok(())
}

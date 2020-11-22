use crate::client::BackupClient;
use crate::client::ClientConfig;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::Generation;
use log::{debug, info};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tempfile::NamedTempFile;

pub fn restore(config: &Path, gen_id: &str, to: &Path) -> anyhow::Result<()> {
    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let dbname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    let config = ClientConfig::read_config(&config).unwrap();

    let client = BackupClient::new(&config.server_url)?;
    let gen_chunk = client.fetch_generation(&gen_id)?;
    debug!("gen: {:?}", gen_chunk);

    {
        // Fetch the SQLite file, storing it in the temporary file.
        let mut dbfile = File::create(&dbname)?;
        for id in gen_chunk.chunk_ids() {
            let chunk = client.fetch_chunk(id)?;
            dbfile.write_all(chunk.data())?;
        }
    }
    info!("downloaded generation to {}", dbname.display());

    let gen = Generation::open(&dbname)?;
    for (fileid, entry) in gen.files()? {
        restore_generation(&client, &gen, fileid, entry, &to)?;
    }

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

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

fn restore_generation(
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

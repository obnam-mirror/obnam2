use crate::client::{BackupClient, ClientConfig};
use crate::fsiter::FsIterator;
use crate::generation::Generation;
use std::path::Path;
use tempfile::NamedTempFile;

pub fn backup(config: &Path, buffer_size: usize) -> anyhow::Result<()> {
    let config = ClientConfig::read_config(config)?;
    let client = BackupClient::new(&config.server_url)?;

    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let dbname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    {
        // Create the SQLite database using the named temporary file.
        // The fetching is in its own block so that the file handles
        // get closed and data flushed to disk.
        let mut gen = Generation::create(&dbname)?;
        gen.insert_iter(FsIterator::new(&config.root).map(|entry| match entry {
            Err(err) => Err(err),
            Ok(entry) => client.upload_filesystem_entry(entry, buffer_size),
        }))?;
    }

    // Upload the SQLite file, i.e., the named temporary file, which
    // still exists, since we persisted it above.
    let gen_id = client.upload_generation(&dbname, buffer_size)?;
    println!("gen id: {}", gen_id);

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

    Ok(())
}

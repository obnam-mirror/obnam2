use crate::client::{BackupClient, ClientConfig};
use crate::fsiter::FsIterator;
use crate::generation::Generation;
use std::path::Path;

pub fn backup(config: &Path, buffer_size: usize) -> anyhow::Result<()> {
    let config = ClientConfig::read_config(config)?;
    let client = BackupClient::new(&config.server_name, config.server_port)?;

    {
        let mut gen = Generation::create(&config.dbname)?;
        gen.insert_iter(FsIterator::new(&config.root).map(|entry| match entry {
            Err(err) => Err(err),
            Ok(entry) => client.upload_filesystem_entry(entry, buffer_size),
        }))?;
    }
    let gen_id = client.upload_generation(&config.dbname, buffer_size)?;
    println!("gen id: {}", gen_id);

    Ok(())
}

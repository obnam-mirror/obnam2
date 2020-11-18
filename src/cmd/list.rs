use crate::client::{BackupClient, ClientConfig};
use std::path::Path;

pub fn list(config: &Path) -> anyhow::Result<()> {
    let config = ClientConfig::read_config(&config)?;
    let client = BackupClient::new(&config.server_url)?;

    for gen_id in client.list_generations()? {
        println!("{}", gen_id);
    }

    Ok(())
}

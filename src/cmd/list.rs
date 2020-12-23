use crate::client::{BackupClient, ClientConfig};

pub fn list(config: &ClientConfig) -> anyhow::Result<()> {
    let client = BackupClient::new(&config.server_url)?;

    for gen_id in client.list_generations()? {
        println!("{}", gen_id);
    }

    Ok(())
}

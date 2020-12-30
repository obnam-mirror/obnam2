use crate::client::{BackupClient, ClientConfig};

pub fn list(config: &ClientConfig) -> anyhow::Result<()> {
    let client = BackupClient::new(&config.server_url)?;

    let generations = client.list_generations()?;
    for finished in generations.iter() {
        println!("{} {}", finished.id(), finished.ended());
    }

    Ok(())
}

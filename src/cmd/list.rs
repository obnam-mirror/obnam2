use crate::client::{BackupClient, ClientConfig};

pub fn list(config: &ClientConfig) -> anyhow::Result<()> {
    let client = BackupClient::new(&config.server_url)?;

    let mut generations = client.list_generations()?;
    generations.sort_by_cached_key(|gen| gen.ended().to_string());
    for finished in generations {
        println!("{} {}", finished.id(), finished.ended());
    }

    Ok(())
}

use crate::client::{BackupClient, ClientConfig};
use crate::error::ObnamError;

pub fn list(config: &ClientConfig) -> Result<(), ObnamError> {
    let client = BackupClient::new(&config.server_url)?;

    let generations = client.list_generations()?;
    for finished in generations.iter() {
        println!("{} {}", finished.id(), finished.ended());
    }

    Ok(())
}

//! The `list` subcommand.

use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use clap::Parser;
use tokio::runtime::Runtime;

/// List generations on the server.
#[derive(Debug, Parser)]
pub struct List {}

impl List {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let client = BackupClient::new(config)?;
        let trust = client
            .get_client_trust()
            .await?
            .or_else(|| Some(ClientTrust::new("FIXME", None, "".to_string(), vec![])))
            .unwrap();

        let generations = client.list_generations(&trust);
        for finished in generations.iter() {
            println!("{} {}", finished.id(), finished.ended());
        }

        Ok(())
    }
}

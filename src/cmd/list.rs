//! The `list` subcommand.

use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use structopt::StructOpt;
use tokio::runtime::Runtime;

/// List generations on the server.
#[derive(Debug, StructOpt)]
pub struct List {}

impl List {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let client = BackupClient::new(config)?;

        let generations = client.list_generations().await?;
        for finished in generations.iter() {
            println!("{} {}", finished.id(), finished.ended());
        }

        Ok(())
    }
}

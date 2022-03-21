//! The `resolve` subcommand.

use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use structopt::StructOpt;
use tokio::runtime::Runtime;

/// Resolve a generation reference into a generation id.
#[derive(Debug, StructOpt)]
pub struct Resolve {
    /// The generation reference.
    generation: String,
}

impl Resolve {
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

        match generations.resolve(&self.generation) {
            Err(err) => {
                return Err(err.into());
            }
            Ok(gen_id) => {
                println!("{}", gen_id.as_chunk_id());
            }
        };

        Ok(())
    }
}

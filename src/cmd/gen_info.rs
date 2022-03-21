//! The `gen-info` subcommand.

use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use log::info;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

/// Show metadata for a generation.
#[derive(Debug, StructOpt)]
pub struct GenInfo {
    /// Reference of the generation.
    #[structopt()]
    gen_ref: String,
}

impl GenInfo {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;

        let client = BackupClient::new(config)?;

        let trust = client
            .get_client_trust()
            .await?
            .or_else(|| Some(ClientTrust::new("FIXME", None, "".to_string(), vec![])))
            .unwrap();

        let genlist = client.list_generations(&trust);
        let gen_id = genlist.resolve(&self.gen_ref)?;
        info!("generation id is {}", gen_id.as_chunk_id());

        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        let meta = gen.meta()?;
        println!("{}", serde_json::to_string_pretty(&meta)?);

        Ok(())
    }
}

//! The `inspect` subcommand.

use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;

use log::info;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

/// Make a backup.
#[derive(Debug, StructOpt)]
pub struct Inspect {
    /// Reference to generation to inspect.
    #[structopt()]
    gen_id: String,
}

impl Inspect {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;
        let client = BackupClient::new(config)?;
        let genlist = client.list_generations().await?;
        let gen_id = genlist.resolve(&self.gen_id)?;
        info!("generation id is {}", gen_id.as_chunk_id());

        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        let meta = gen.meta()?;
        println!("schema_version: {}", meta.schema_version());

        Ok(())
    }
}

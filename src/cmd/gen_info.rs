use crate::client::AsyncBackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use log::info;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

#[derive(Debug, StructOpt)]
pub struct GenInfo {
    #[structopt()]
    gen_ref: String,
}

impl GenInfo {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;

        let client = AsyncBackupClient::new(config)?;

        let genlist = client.list_generations().await?;
        let gen_id = genlist.resolve(&self.gen_ref)?;
        info!("generation id is {}", gen_id.as_chunk_id());

        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        let meta = gen.meta()?;
        println!("{}", serde_json::to_string_pretty(&meta)?);

        Ok(())
    }
}

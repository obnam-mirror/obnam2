use crate::client::AsyncBackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use structopt::StructOpt;
use tokio::runtime::Runtime;

#[derive(Debug, StructOpt)]
pub struct Resolve {
    generation: String,
}

impl Resolve {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let client = AsyncBackupClient::new(config)?;
        let generations = client.list_generations().await?;

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

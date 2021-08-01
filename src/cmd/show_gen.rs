use crate::client::AsyncBackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemKind;
use indicatif::HumanBytes;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

#[derive(Debug, StructOpt)]
pub struct ShowGeneration {
    #[structopt(default_value = "latest")]
    gen_id: String,
}

impl ShowGeneration {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;
        let client = AsyncBackupClient::new(config)?;

        let genlist = client.list_generations().await?;
        let gen_id = genlist.resolve(&self.gen_id)?;
        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        let mut files = gen.files()?;
        let mut files = files.iter()?;

        let total_bytes = files.try_fold(0, |acc, file| {
            file.map(|file| {
                let e = file.entry();
                if e.kind() == FilesystemKind::Regular {
                    acc + e.len()
                } else {
                    acc
                }
            })
        });
        let total_bytes = total_bytes?;

        println!("generation-id: {}", gen_id);
        println!("file-count: {}", gen.file_count()?);
        println!("file-bytes: {}", HumanBytes(total_bytes));
        println!("file-bytes-raw: {}", total_bytes);

        Ok(())
    }
}

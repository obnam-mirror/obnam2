use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemKind;
use indicatif::HumanBytes;
use structopt::StructOpt;
use tempfile::NamedTempFile;

#[derive(Debug, StructOpt)]
pub struct ShowGeneration {
    #[structopt(default_value = "latest")]
    gen_id: String,
}

impl ShowGeneration {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;

        let client = BackupClient::new(config)?;

        let genlist = client.list_generations()?;
        let gen_id: String = genlist.resolve(&self.gen_id)?;
        let gen = client.fetch_generation(&gen_id, temp.path())?;
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

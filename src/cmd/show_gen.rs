//! The `show-generation` subcommand.

use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::db::DbInt;
use crate::error::ObnamError;
use crate::fsentry::FilesystemKind;
use crate::generation::GenId;
use clap::Parser;
use indicatif::HumanBytes;
use serde::Serialize;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

/// Show information about a generation.
#[derive(Debug, Parser)]
pub struct ShowGeneration {
    /// Reference to the generation. Defaults to latest.
    #[clap(default_value = "latest")]
    gen_id: String,
}

impl ShowGeneration {
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
        let gen_id = genlist.resolve(&self.gen_id)?;
        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        let mut files = gen.files()?;
        let mut files = files.iter()?;

        let total_bytes = files.try_fold(0, |acc, file| {
            file.map(|(_, e, _, _)| {
                if e.kind() == FilesystemKind::Regular {
                    acc + e.len()
                } else {
                    acc
                }
            })
        });
        let total_bytes = total_bytes?;

        let output = Output::new(gen_id)
            .db_bytes(temp.path().metadata()?.len())
            .file_count(gen.file_count()?)
            .file_bytes(total_bytes);
        serde_json::to_writer_pretty(std::io::stdout(), &output)?;

        Ok(())
    }
}

#[derive(Debug, Default, Serialize)]
struct Output {
    generation_id: String,
    file_count: DbInt,
    file_bytes: String,
    file_bytes_raw: u64,
    db_bytes: String,
    db_bytes_raw: u64,
}

impl Output {
    fn new(gen_id: GenId) -> Self {
        Self {
            generation_id: format!("{}", gen_id),
            ..Self::default()
        }
    }

    fn file_count(mut self, n: DbInt) -> Self {
        self.file_count = n;
        self
    }

    fn file_bytes(mut self, n: u64) -> Self {
        self.file_bytes_raw = n;
        self.file_bytes = HumanBytes(n).to_string();
        self
    }

    fn db_bytes(mut self, n: u64) -> Self {
        self.db_bytes_raw = n;
        self.db_bytes = HumanBytes(n).to_string();
        self
    }
}

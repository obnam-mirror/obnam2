//! The `list-files` subcommand.

use crate::backup_reason::Reason;
use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use clap::Parser;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

/// List files in a backup.
#[derive(Debug, Parser)]
pub struct ListFiles {
    /// Reference to backup to list files in.
    #[clap(default_value = "latest")]
    gen_id: String,
}

impl ListFiles {
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
        for file in gen.files()?.iter()? {
            let (_, entry, reason, _) = file?;
            println!("{}", format_entry(&entry, reason));
        }

        Ok(())
    }
}

fn format_entry(e: &FilesystemEntry, reason: Reason) -> String {
    let kind = match e.kind() {
        FilesystemKind::Regular => "-",
        FilesystemKind::Directory => "d",
        FilesystemKind::Symlink => "l",
        FilesystemKind::Socket => "s",
        FilesystemKind::Fifo => "p",
    };
    format!("{} {} ({})", kind, e.pathbuf().display(), reason)
}

//! The `backup` subcommand.

use crate::backup_run::BackupRun;
use crate::client::AsyncBackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::generation::GenId;

use log::info;
use std::time::SystemTime;
use structopt::StructOpt;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

/// Make a backup.
#[derive(Debug, StructOpt)]
pub struct Backup {}

impl Backup {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let runtime = SystemTime::now();

        let client = AsyncBackupClient::new(config)?;
        let genlist = client.list_generations().await?;

        let oldtemp = NamedTempFile::new()?;
        let newtemp = NamedTempFile::new()?;

        let (is_incremental, outcome) = match genlist.resolve("latest") {
            Err(_) => {
                info!("fresh backup without a previous generation");
                let mut run = BackupRun::initial(config, &client)?;
                let old = run.start(None, oldtemp.path()).await?;
                (false, run.backup_roots(config, &old, newtemp.path()).await?)
            }
            Ok(old_id) => {
                info!("incremental backup based on {}", old_id);
                let mut run = BackupRun::incremental(config, &client)?;
                let old = run.start(Some(&old_id), oldtemp.path()).await?;
                (true, run.backup_roots(config, &old, newtemp.path()).await?)
            }
        };

        for w in outcome.warnings.iter() {
            println!("warning: {}", w);
        }

        if is_incremental && !outcome.new_cachedir_tags.is_empty() {
            println!("New CACHEDIR.TAG files since the last backup:");
            for t in &outcome.new_cachedir_tags {
                println!("- {:?}", t);
            }
            println!("You can configure Obnam to ignore all such files by setting `exclude_cache_tag_directories` to `false`.");
        }

        report_stats(
            &runtime,
            outcome.files_count,
            &outcome.gen_id,
            outcome.warnings.len(),
        )?;

        if is_incremental && !outcome.new_cachedir_tags.is_empty() {
            Err(ObnamError::NewCachedirTagsFound)
        } else {
            Ok(())
        }
    }
}

fn report_stats(
    runtime: &SystemTime,
    file_count: i64,
    gen_id: &GenId,
    num_warnings: usize,
) -> Result<(), ObnamError> {
    println!("status: OK");
    println!("warnings: {}", num_warnings);
    println!("duration: {}", runtime.elapsed()?.as_secs());
    println!("file-count: {}", file_count);
    println!("generation-id: {}", gen_id);
    Ok(())
}

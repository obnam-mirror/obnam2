//! The `backup` subcommand.

use crate::backup_run::{current_timestamp, BackupRun};
use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::dbgen::{schema_version, FileId, DEFAULT_SCHEMA_MAJOR};
use crate::error::ObnamError;
use crate::generation::GenId;
use crate::performance::{Clock, Performance};
use crate::schema::VersionComponent;

use log::info;
use std::time::SystemTime;
use structopt::StructOpt;
use tempfile::tempdir;
use tokio::runtime::Runtime;

/// Make a backup.
#[derive(Debug, StructOpt)]
pub struct Backup {
    /// Force a full backup, instead of an incremental one.
    #[structopt(long)]
    full: bool,

    /// Backup schema major version to use.
    #[structopt(long)]
    backup_version: Option<VersionComponent>,
}

impl Backup {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig, perf: &mut Performance) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config, perf))
    }

    async fn run_async(
        &self,
        config: &ClientConfig,
        perf: &mut Performance,
    ) -> Result<(), ObnamError> {
        let runtime = SystemTime::now();

        let major = self.backup_version.unwrap_or(DEFAULT_SCHEMA_MAJOR);
        let schema = schema_version(major)?;

        let client = BackupClient::new(config)?;
        let trust = client
            .get_client_trust()
            .await?
            .or_else(|| Some(ClientTrust::new("FIXME", None, current_timestamp(), vec![])))
            .unwrap();
        let genlist = client.list_generations(&trust);

        let temp = tempdir()?;
        let oldtemp = temp.path().join("old.db");
        let newtemp = temp.path().join("new.db");

        let old_id = if self.full {
            None
        } else {
            match genlist.resolve("latest") {
                Err(_) => None,
                Ok(old_id) => Some(old_id),
            }
        };

        let (is_incremental, outcome) = if let Some(old_id) = old_id {
            info!("incremental backup based on {}", old_id);
            let mut run = BackupRun::incremental(config, &client)?;
            let old = run.start(Some(&old_id), &oldtemp, perf).await?;
            (
                true,
                run.backup_roots(config, &old, &newtemp, schema, perf)
                    .await?,
            )
        } else {
            info!("fresh backup without a previous generation");
            let mut run = BackupRun::initial(config, &client)?;
            let old = run.start(None, &oldtemp, perf).await?;
            (
                false,
                run.backup_roots(config, &old, &newtemp, schema, perf)
                    .await?,
            )
        };

        perf.start(Clock::GenerationUpload);
        let mut trust = trust;
        trust.append_backup(outcome.gen_id.as_chunk_id());
        trust.finalize(current_timestamp());
        let trust = trust.to_data_chunk()?;
        let trust_id = client.upload_chunk(trust).await?;
        perf.stop(Clock::GenerationUpload);
        info!("uploaded new client-trust {}", trust_id);

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
    file_count: FileId,
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

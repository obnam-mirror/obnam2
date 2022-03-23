//! The `backup` subcommand.

use crate::backup_run::{current_timestamp, BackupRun};
use crate::chunk::ClientTrust;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::dbgen::{schema_version, FileId, DEFAULT_SCHEMA_MAJOR};
use crate::error::ObnamError;
use crate::generation::GenId;
use crate::schema::VersionComponent;

use log::info;
use std::time::SystemTime;
use structopt::StructOpt;
use tempfile::tempdir;
use tokio::runtime::Runtime;

/// Make a backup.
#[derive(Debug, StructOpt)]
pub struct Backup {
    /// Backup schema major version.
    #[structopt(long)]
    backup_version: Option<VersionComponent>,
}

impl Backup {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let runtime = SystemTime::now();

        let major = self.backup_version.or(Some(DEFAULT_SCHEMA_MAJOR)).unwrap();
        let schema = schema_version(major)?;
        eprintln!("backup: schema: {}", schema);

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

        let (is_incremental, outcome) = match genlist.resolve("latest") {
            Err(_) => {
                info!("fresh backup without a previous generation");
                let mut run = BackupRun::initial(config, &client)?;
                let old = run.start(None, &oldtemp).await?;
                (
                    false,
                    run.backup_roots(config, &old, &newtemp, schema).await?,
                )
            }
            Ok(old_id) => {
                info!("incremental backup based on {}", old_id);
                let mut run = BackupRun::incremental(config, &client)?;
                let old = run.start(Some(&old_id), &oldtemp).await?;
                (
                    true,
                    run.backup_roots(config, &old, &newtemp, schema).await?,
                )
            }
        };

        let mut trust = trust;
        trust.append_backup(outcome.gen_id.as_chunk_id());
        trust.finalize(current_timestamp());
        let trust = trust.to_data_chunk()?;
        let trust_id = client.upload_chunk(trust).await?;
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

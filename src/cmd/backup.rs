use crate::backup_progress::BackupProgress;
use crate::backup_run::BackupRun;
use crate::chunkid::ChunkId;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use bytesize::MIB;
use log::info;
use std::path::Path;
use std::time::SystemTime;
use structopt::StructOpt;
use tempfile::NamedTempFile;

const SQLITE_CHUNK_SIZE: usize = MIB as usize;

#[derive(Debug, StructOpt)]
pub struct Backup {}

impl Backup {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let runtime = SystemTime::now();

        let client = BackupClient::new(config)?;
        let genlist = client.list_generations()?;

        let oldtemp = NamedTempFile::new()?;
        let newtemp = NamedTempFile::new()?;

        let (is_incremental, (count, warnings, new_tags)) = match genlist.resolve("latest") {
            Err(_) => {
                info!("fresh backup without a previous generation");
                let mut run = BackupRun::initial(config, &client)?;
                let old = run.start(None, oldtemp.path())?;
                (false, run.backup_roots(config, &old, newtemp.path())?)
            }
            Ok(old_id) => {
                info!("incremental backup based on {}", old_id);
                let mut run = BackupRun::incremental(config, &client)?;
                let old = run.start(Some(&old_id), oldtemp.path())?;
                (true, run.backup_roots(config, &old, newtemp.path())?)
            }
        };

        let gen_id = upload_nascent_generation(&client, newtemp.path())?;

        for w in warnings.iter() {
            println!("warning: {}", w);
        }

        if is_incremental && !new_tags.is_empty() {
            println!("New CACHEDIR.TAG files since the last backup:");
            for t in new_tags {
                println!("- {:?}", t);
            }
            println!("You can configure Obnam to ignore all such files by setting `exclude_cache_tag_directories` to `false`.");
        }

        report_stats(&runtime, count, &gen_id, warnings.len())?;

        Ok(())
    }
}

fn report_stats(
    runtime: &SystemTime,
    file_count: i64,
    gen_id: &ChunkId,
    num_warnings: usize,
) -> Result<(), ObnamError> {
    println!("status: OK");
    println!("warnings: {}", num_warnings);
    println!("duration: {}", runtime.elapsed()?.as_secs());
    println!("file-count: {}", file_count);
    println!("generation-id: {}", gen_id);
    Ok(())
}

fn upload_nascent_generation(
    client: &BackupClient,
    filename: &Path,
) -> Result<ChunkId, ObnamError> {
    let progress = BackupProgress::upload_generation();
    let gen_id = client.upload_generation(filename, SQLITE_CHUNK_SIZE)?;
    progress.finish();
    Ok(gen_id)
}

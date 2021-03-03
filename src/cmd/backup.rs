use crate::backup_progress::BackupProgress;
use crate::backup_run::{IncrementalBackup, InitialBackup};
use crate::chunkid::ChunkId;
use crate::client::{BackupClient, ClientConfig};
use crate::error::ObnamError;
use crate::fsiter::FsIterator;
use crate::generation::NascentGeneration;
use bytesize::MIB;
use log::info;
use std::time::SystemTime;
use tempfile::NamedTempFile;

const SQLITE_CHUNK_SIZE: usize = MIB as usize;

pub fn backup(config: &ClientConfig) -> Result<(), ObnamError> {
    let runtime = SystemTime::now();

    let client = BackupClient::new(config)?;
    let genlist = client.list_generations()?;
    let (gen_id, file_count) = match genlist.resolve("latest") {
        Err(_) => initial_backup(&config, &client)?,
        Ok(old_ref) => incremental_backup(&old_ref, &config, &client)?,
    };

    report_stats(&runtime, file_count, &gen_id)?;

    Ok(())
}

fn report_stats(runtime: &SystemTime, file_count: i64, gen_id: &ChunkId) -> Result<(), ObnamError> {
    println!("status: OK");
    println!("duration: {}", runtime.elapsed()?.as_secs());
    println!("file-count: {}", file_count);
    println!("generation-id: {}", gen_id);
    Ok(())
}

fn initial_backup(
    config: &ClientConfig,
    client: &BackupClient,
) -> Result<(ChunkId, i64), ObnamError> {
    let run = InitialBackup::new(config, &client)?;
    let newtemp = NamedTempFile::new()?;
    let count = {
        info!("fresh backup without a previous generation");

        let mut new = NascentGeneration::create(newtemp.path())?;
        for root in &config.roots {
            let iter = FsIterator::new(root);
            new.insert_iter(iter.map(|entry| run.backup(entry)))?;
        }
        new.file_count()
    };

    let progress = BackupProgress::upload_generation();
    let gen_id = client.upload_generation(newtemp.path(), SQLITE_CHUNK_SIZE)?;
    progress.finish();

    Ok((gen_id, count))
}

fn incremental_backup(
    old_ref: &str,
    config: &ClientConfig,
    client: &BackupClient,
) -> Result<(ChunkId, i64), ObnamError> {
    let mut run = IncrementalBackup::new(config, &client)?;
    let newtemp = NamedTempFile::new()?;
    let count = {
        info!("incremental backup based on {}", old_ref);
        let oldtemp = NamedTempFile::new()?;

        let old = run.fetch_previous_generation(old_ref, oldtemp.path())?;
        run.start_backup(&old)?;
        let mut new = NascentGeneration::create(newtemp.path())?;
        for root in &config.roots {
            let iter = FsIterator::new(root);
            new.insert_iter(iter.map(|entry| run.backup(entry, &old)))?;
        }
        new.file_count()
    };

    let progress = BackupProgress::upload_generation();
    let gen_id = client.upload_generation(newtemp.path(), SQLITE_CHUNK_SIZE)?;
    progress.finish();

    Ok((gen_id, count))
}

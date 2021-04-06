use crate::backup_progress::BackupProgress;
use crate::backup_run::{BackupError, IncrementalBackup, InitialBackup};
use crate::chunkid::ChunkId;
use crate::client::{BackupClient, ClientConfig};
use crate::error::ObnamError;
use crate::fsiter::FsIterator;
use crate::generation::NascentGeneration;
use bytesize::MIB;
use log::info;
use std::path::Path;
use std::time::SystemTime;
use tempfile::NamedTempFile;

const SQLITE_CHUNK_SIZE: usize = MIB as usize;

pub fn backup(config: &ClientConfig) -> Result<(), ObnamError> {
    let runtime = SystemTime::now();

    let client = BackupClient::new(config)?;
    let genlist = client.list_generations()?;
    let (gen_id, file_count, warnings) = match genlist.resolve("latest") {
        Err(_) => initial_backup(&config, &client)?,
        Ok(old_ref) => incremental_backup(&old_ref, &config, &client)?,
    };

    for w in warnings.iter() {
        println!("warning: {}", w);
    }

    report_stats(&runtime, file_count, &gen_id, warnings.len())?;

    Ok(())
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

fn initial_backup(
    config: &ClientConfig,
    client: &BackupClient,
) -> Result<(ChunkId, i64, Vec<BackupError>), ObnamError> {
    info!("fresh backup without a previous generation");
    let newtemp = NamedTempFile::new()?;
    let run = InitialBackup::new(config, &client)?;
    let mut all_warnings = vec![];
    let count = {
        let mut new = NascentGeneration::create(newtemp.path())?;
        for root in &config.roots {
            let iter = FsIterator::new(root);
            let warnings = new.insert_iter(iter.map(|entry| run.backup(entry)))?;
            for w in warnings {
                all_warnings.push(w);
            }
        }
        new.file_count()
    };
    run.drop();

    let gen_id = upload_nascent_generation(client, newtemp.path())?;
    Ok((gen_id, count, all_warnings))
}

fn incremental_backup(
    old_ref: &str,
    config: &ClientConfig,
    client: &BackupClient,
) -> Result<(ChunkId, i64, Vec<BackupError>), ObnamError> {
    info!("incremental backup based on {}", old_ref);
    let newtemp = NamedTempFile::new()?;
    let mut run = IncrementalBackup::new(config, &client)?;
    let mut all_warnings = vec![];
    let count = {
        let oldtemp = NamedTempFile::new()?;
        let old = run.fetch_previous_generation(old_ref, oldtemp.path())?;
        run.start_backup(&old)?;
        let mut new = NascentGeneration::create(newtemp.path())?;
        for root in &config.roots {
            let iter = FsIterator::new(root);
            let warnings = new.insert_iter(iter.map(|entry| run.backup(entry, &old)))?;
            for w in warnings {
                all_warnings.push(w);
            }
        }
        new.file_count()
    };
    run.drop();

    let gen_id = upload_nascent_generation(client, newtemp.path())?;
    Ok((gen_id, count, all_warnings))
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

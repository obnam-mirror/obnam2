use crate::backup_run::BackupRun;
use crate::chunkid::ChunkId;
use crate::client::ClientConfig;
use crate::error::ObnamError;
use crate::fsiter::FsIterator;
use crate::generation::NascentGeneration;
use log::info;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tempfile::NamedTempFile;

const SQLITE_CHUNK_SIZE: usize = 1024 * 1024;

pub fn backup(config: &ClientConfig) -> Result<(), ObnamError> {
    let runtime = SystemTime::now();

    let run = BackupRun::new(config)?;

    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let oldname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let newname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    let genlist = run.client().list_generations()?;
    let file_count = match genlist.resolve("latest") {
        Err(_) => initial_backup(&config.roots, &newname, &run)?,
        Ok(old) => incremental_backup(&old, &config.roots, &newname, &oldname, &run)?,
    };
    run.progress().finish();

    // Upload the SQLite file, i.e., the named temporary file, which
    // still exists, since we persisted it above.
    let gen_id = run
        .client()
        .upload_generation(&newname, SQLITE_CHUNK_SIZE)?;

    // Delete the temporary file.q
    std::fs::remove_file(&newname)?;
    std::fs::remove_file(&oldname)?;

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

fn initial_backup(roots: &[PathBuf], newname: &Path, run: &BackupRun) -> Result<i64, ObnamError> {
    info!("fresh backup without a previous generation");

    let mut new = NascentGeneration::create(&newname)?;
    for root in roots {
        let iter = FsIterator::new(root);
        new.insert_iter(iter.map(|entry| run.backup_file_initially(entry)))?;
    }
    Ok(new.file_count())
}

fn incremental_backup(
    old: &str,
    roots: &[PathBuf],
    newname: &Path,
    oldname: &Path,
    run: &BackupRun,
) -> Result<i64, ObnamError> {
    info!("incremental backup based on {}", old);

    let old = run.client().fetch_generation(&old, &oldname)?;
    let mut new = NascentGeneration::create(&newname)?;
    for root in roots {
        let iter = FsIterator::new(root);
        run.progress()
            .files_in_previous_generation(old.file_count()? as u64);
        new.insert_iter(iter.map(|entry| run.backup_file_incrementally(entry, &old)))?;
    }
    Ok(new.file_count())
}

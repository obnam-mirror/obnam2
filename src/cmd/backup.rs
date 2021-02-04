use crate::backup_run::BackupRun;
use crate::client::ClientConfig;
use crate::error::ObnamError;
use crate::fsiter::FsIterator;
use crate::generation::NascentGeneration;
use log::info;
use std::time::SystemTime;
use tempfile::NamedTempFile;

pub fn backup(config: &ClientConfig, buffer_size: usize) -> Result<(), ObnamError> {
    let runtime = SystemTime::now();

    let run = BackupRun::new(config, buffer_size)?;

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
    let file_count = {
        let iter = FsIterator::new(&config.root);
        let mut new = NascentGeneration::create(&newname)?;

        match genlist.resolve("latest") {
            Err(_) => {
                info!("fresh backup without a previous generation");
                new.insert_iter(iter.map(|entry| run.backup_file_initially(entry)))?;
            }
            Ok(old) => {
                info!("incremental backup based on {}", old);
                let old = run.client().fetch_generation(&old, &oldname)?;
                run.progress()
                    .files_in_previous_generation(old.file_count()? as u64);
                new.insert_iter(iter.map(|entry| run.backup_file_incrementally(entry, &old)))?;
            }
        }
        run.progress().finish();
        new.file_count()
    };

    // Upload the SQLite file, i.e., the named temporary file, which
    // still exists, since we persisted it above.
    let gen_id = run.client().upload_generation(&newname, buffer_size)?;
    println!("status: OK");
    println!("duration: {}", runtime.elapsed()?.as_secs());
    println!("file-count: {}", file_count);
    println!("generation-id: {}", gen_id);

    // Delete the temporary file.q
    std::fs::remove_file(&newname)?;
    std::fs::remove_file(&oldname)?;

    Ok(())
}

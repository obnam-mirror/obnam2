use crate::client::{BackupClient, ClientConfig};
use crate::fsiter::FsIterator;
use crate::generation::NascentGeneration;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use tempfile::NamedTempFile;

const GUESS_FILE_COUNT: u64 = 0;

pub fn backup(config: &ClientConfig, buffer_size: usize) -> anyhow::Result<()> {
    let client = BackupClient::new(&config.server_url)?;

    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let dbname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    {
        // Create the SQLite database using the named temporary file.
        // The fetching is in its own block so that the file handles
        // get closed and data flushed to disk.
        let mut gen = NascentGeneration::create(&dbname)?;
        let progress = create_progress_bar(GUESS_FILE_COUNT, true);
        progress.enable_steady_tick(100);
        gen.insert_iter(FsIterator::new(&config.root).map(|entry| {
            progress.inc(1);
            match entry {
                Err(err) => Err(err),
                Ok(entry) => {
                    let path = &entry.pathbuf();
                    info!("backup: {}", path.display());
                    progress.set_message(&format!("{}", path.display()));
                    client.upload_filesystem_entry(entry, buffer_size)
                }
            }
        }))?;
        progress.set_length(gen.file_count());
        progress.finish();
        println!("file count: {}", gen.file_count());
    }

    // Upload the SQLite file, i.e., the named temporary file, which
    // still exists, since we persisted it above.
    let gen_id = client.upload_generation(&dbname, buffer_size)?;
    println!("gen id: {}", gen_id);

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

    Ok(())
}

fn create_progress_bar(file_count: u64, verbose: bool) -> ProgressBar {
    let progress = if verbose {
        ProgressBar::new(file_count)
    } else {
        ProgressBar::hidden()
    };
    let parts = vec![
        "{wide_bar}",
        "elapsed: {elapsed}",
        "files: {pos}/{len}",
        "current: {wide_msg}",
        "{spinner}",
    ];
    progress.set_style(ProgressStyle::default_bar().template(&parts.join("\n")));
    progress
}

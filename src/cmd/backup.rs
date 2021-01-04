use crate::client::{BackupClient, ClientConfig};
use crate::fsentry::FilesystemEntry;
use crate::fsiter::FsIterator;
use crate::generation::{LocalGeneration, NascentGeneration};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;
use std::fmt;
use tempfile::NamedTempFile;

pub fn backup(config: &ClientConfig, buffer_size: usize) -> anyhow::Result<()> {
    let client = BackupClient::new(&config.server_url)?;

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

    let genlist = client.list_generations()?;
    {
        let iter = FsIterator::new(&config.root);
        let mut new = NascentGeneration::create(&newname)?;
        let progress = create_progress_bar(true);
        progress.enable_steady_tick(100);

        match genlist.resolve("latest") {
            None => {
                info!("fresh backup without a previous generation");
                new.insert_iter(iter.map(|entry| {
                    progress.inc(1);
                    match entry {
                        Err(err) => Err(err),
                        Ok(entry) => {
                            let path = &entry.pathbuf();
                            info!("backup: {}", path.display());
                            progress.set_message(&format!("{}", path.display()));
                            let (new_entry, ids) =
                                client.upload_filesystem_entry(entry, buffer_size)?;
                            Ok((new_entry, ids, Reason::IsNew))
                        }
                    }
                }))?;
            }
            Some(old) => {
                info!("incremental backup based on {}", old);
                let old = client.fetch_generation(&old, &oldname)?;
                progress.set_length(old.file_count()? as u64);
                new.insert_iter(iter.map(|entry| {
                    progress.inc(1);
                    match entry {
                        Err(err) => Err(err),
                        Ok(entry) => {
                            let path = &entry.pathbuf();
                            info!("backup: {}", path.display());
                            progress.set_message(&format!("{}", path.display()));
                            let reason = needs_backup(&old, &entry);
                            match reason {
                                Reason::IsNew | Reason::Changed | Reason::Error => {
                                    let (new_entry, ids) =
                                        client.upload_filesystem_entry(entry, buffer_size)?;
                                    Ok((new_entry, ids, reason))
                                }
                                Reason::Unchanged => {
                                    let fileno = old.get_fileno(&entry.pathbuf())?;
                                    let ids = if let Some(fileno) = fileno {
                                        old.chunkids(fileno)?
                                    } else {
                                        vec![]
                                    };
                                    Ok((entry.clone(), ids, Reason::Unchanged))
                                }
                            }
                        }
                    }
                }))?;
            }
        }
        progress.set_length(new.file_count() as u64);
        progress.finish();
    }

    // Upload the SQLite file, i.e., the named temporary file, which
    // still exists, since we persisted it above.
    let gen_id = client.upload_generation(&newname, buffer_size)?;
    println!("gen id: {}", gen_id);

    // Delete the temporary file.q
    std::fs::remove_file(&newname)?;
    std::fs::remove_file(&oldname)?;

    Ok(())
}

#[derive(Debug)]
pub enum Reason {
    IsNew,
    Changed,
    Unchanged,
    Error,
}

impl ToSql for Reason {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Text(format!(
            "{}",
            self
        ))))
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let reason = match self {
            Reason::IsNew => "new",
            Reason::Changed => "changed",
            Reason::Unchanged => "unchanged",
            Reason::Error => "error",
        };
        write!(f, "{}", reason)
    }
}

fn create_progress_bar(verbose: bool) -> ProgressBar {
    let progress = if verbose {
        ProgressBar::new(0)
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

fn needs_backup(old: &LocalGeneration, new_entry: &FilesystemEntry) -> Reason {
    let new_name = new_entry.pathbuf();
    match old.get_file(&new_name) {
        // File is not in old generation.
        Ok(None) => {
            debug!(
                "needs_backup: file is not in old generation, needs backup: {:?}",
                new_name
            );
            Reason::IsNew
        }

        // File is in old generation. Has its metadata changed?
        Ok(Some(old_entry)) => {
            if file_has_changed(&old_entry, new_entry) {
                debug!("needs_backup: file has changed: {:?}", new_name);
                Reason::Changed
            } else {
                debug!("needs_backup: file has NOT changed: {:?}", new_name);
                Reason::Unchanged
            }
        }

        // There was an error, which we ignore, but we indicate the
        // file needs to be backed up now.
        Err(err) => {
            debug!(
                "needs_backup: lookup in old generation returned error, ignored: {:?}: {}",
                new_name, err
            );
            Reason::Error
        }
    }
}

fn file_has_changed(old: &FilesystemEntry, new: &FilesystemEntry) -> bool {
    let unchanged = old.kind() == new.kind()
        && old.len() == new.len()
        && old.mode() == new.mode()
        && old.mtime() == new.mtime()
        && old.mtime_ns() == new.mtime_ns()
        && old.symlink_target() == new.symlink_target();
    !unchanged
}

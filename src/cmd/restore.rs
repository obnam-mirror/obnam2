use crate::client::BackupClient;
use crate::client::ClientConfig;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::NascentGeneration;
use indicatif::{ProgressBar, ProgressStyle};
use libc::{fchmod, futimens, timespec};
use log::{debug, error, info};
use std::fs::File;
use std::io::prelude::*;
use std::io::Error;
use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tempfile::NamedTempFile;

pub fn restore(config: &ClientConfig, gen_id: &str, to: &Path) -> anyhow::Result<()> {
    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let dbname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    let client = BackupClient::new(&config.server_url)?;
    let gen_chunk = client.fetch_generation(&gen_id)?;
    debug!("gen: {:?}", gen_chunk);

    {
        // Fetch the SQLite file, storing it in the temporary file.
        let mut dbfile = File::create(&dbname)?;
        for id in gen_chunk.chunk_ids() {
            let chunk = client.fetch_chunk(id)?;
            dbfile.write_all(chunk.data())?;
        }
    }
    info!("downloaded generation to {}", dbname.display());

    let gen = NascentGeneration::open(&dbname)?;
    info!("restore file count: {}", gen.file_count());
    let progress = create_progress_bar(gen.file_count(), true);
    for (fileid, entry) in gen.files()? {
        restore_generation(&client, &gen, fileid, &entry, &to, &progress)?;
    }
    for (_, entry) in gen.files()? {
        if entry.is_dir() {
            restore_directory_metadata(&entry, &to)?;
        }
    }
    progress.finish();

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    #[structopt()]
    gen_id: String,

    #[structopt(parse(from_os_str))]
    dbname: PathBuf,

    #[structopt(parse(from_os_str))]
    to: PathBuf,
}

fn restore_generation(
    client: &BackupClient,
    gen: &NascentGeneration,
    fileid: u64,
    entry: &FilesystemEntry,
    to: &Path,
    progress: &ProgressBar,
) -> anyhow::Result<()> {
    debug!("restoring {:?}", entry);
    progress.set_message(&format!("{}", entry.pathbuf().display()));
    progress.inc(1);

    let to = restored_path(entry, to)?;
    match entry.kind() {
        FilesystemKind::Regular => restore_regular(client, &gen, &to, fileid, &entry)?,
        FilesystemKind::Directory => restore_directory(&to)?,
        FilesystemKind::Symlink => restore_symlink(&to, &entry)?,
    }
    Ok(())
}

fn restore_directory(path: &Path) -> anyhow::Result<()> {
    debug!("restoring directory {}", path.display());
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn restore_directory_metadata(entry: &FilesystemEntry, to: &Path) -> anyhow::Result<()> {
    let to = restored_path(entry, to)?;
    match entry.kind() {
        FilesystemKind::Directory => restore_metadata(&to, entry)?,
        _ => panic!(
            "restore_directory_metadata called with non-directory {:?}",
            entry,
        ),
    }
    Ok(())
}

fn restored_path(entry: &FilesystemEntry, to: &Path) -> anyhow::Result<PathBuf> {
    let path = &entry.pathbuf();
    let path = if path.is_absolute() {
        path.strip_prefix("/")?
    } else {
        path
    };
    Ok(to.join(path))
}

fn restore_regular(
    client: &BackupClient,
    gen: &NascentGeneration,
    path: &Path,
    fileid: u64,
    entry: &FilesystemEntry,
) -> anyhow::Result<()> {
    debug!("restoring regular {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    std::fs::create_dir_all(parent)?;
    {
        let mut file = std::fs::File::create(path)?;
        for chunkid in gen.chunkids(fileid)? {
            let chunk = client.fetch_chunk(&chunkid)?;
            file.write_all(chunk.data())?;
        }
        restore_metadata(path, entry)?;
    }
    debug!("restored regular {}", path.display());
    Ok(())
}

fn restore_symlink(path: &Path, entry: &FilesystemEntry) -> anyhow::Result<()> {
    debug!("restoring symlink {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
        {
            symlink(path, entry.symlink_target().unwrap())?;
        }
    }
    debug!("restored regular {}", path.display());
    Ok(())
}

fn restore_metadata(path: &Path, entry: &FilesystemEntry) -> anyhow::Result<()> {
    debug!("restoring metadata for {}", entry.pathbuf().display());

    let handle = File::open(path)?;

    let atime = timespec {
        tv_sec: entry.atime(),
        tv_nsec: entry.atime_ns(),
    };
    let mtime = timespec {
        tv_sec: entry.mtime(),
        tv_nsec: entry.mtime_ns(),
    };
    let times = [atime, mtime];
    let times: *const timespec = &times[0];

    // We have to use unsafe here to be able call the libc functions
    // below.
    unsafe {
        let fd = handle.as_raw_fd(); // FIXME: needs to NOT follow symlinks

        debug!("fchmod");
        if fchmod(fd, entry.mode()) == -1 {
            let error = Error::last_os_error();
            error!("fchmod failed on {}", path.display());
            return Err(error.into());
        }

        debug!("futimens");
        if futimens(fd, times) == -1 {
            let error = Error::last_os_error();
            error!("futimens failed on {}", path.display());
            return Err(error.into());
        }
    }
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

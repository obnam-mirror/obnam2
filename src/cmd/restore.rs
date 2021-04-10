use crate::backup_reason::Reason;
use crate::client::{BackupClient, ClientError};
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::{LocalGeneration, LocalGenerationError};
use indicatif::{ProgressBar, ProgressStyle};
use libc::{chmod, mkfifo, timespec, utimensat, AT_FDCWD};
use log::{debug, error, info};
use std::ffi::CString;
use std::io::prelude::*;
use std::io::Error;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::path::StripPrefixError;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tempfile::NamedTempFile;

#[derive(Debug, StructOpt)]
pub struct Restore {
    #[structopt()]
    gen_id: String,

    #[structopt(parse(from_os_str))]
    to: PathBuf,
}

impl Restore {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;

        let client = BackupClient::new(config)?;

        let genlist = client.list_generations()?;
        let gen_id: String = genlist.resolve(&self.gen_id)?;
        info!("generation id is {}", gen_id);

        let gen = client.fetch_generation(&gen_id, temp.path())?;
        info!("restoring {} files", gen.file_count()?);
        let progress = create_progress_bar(gen.file_count()?, true);
        for file in gen.files()? {
            match file.reason() {
                Reason::FileError => (),
                _ => restore_generation(
                    &client,
                    &gen,
                    file.fileno(),
                    file.entry(),
                    &self.to,
                    &progress,
                )?,
            }
        }
        for file in gen.files()? {
            if file.entry().is_dir() {
                restore_directory_metadata(file.entry(), &self.to)?;
            }
        }
        progress.finish();

        Ok(())
    }
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

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("Could not create named pipe (FIFO) {0}")]
    NamedPipeCreationError(PathBuf),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    #[error(transparent)]
    StripPrefixError(#[from] StripPrefixError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeYamlError(#[from] serde_yaml::Error),

    #[error(transparent)]
    NulError(#[from] std::ffi::NulError),
}

pub type RestoreResult<T> = Result<T, RestoreError>;

fn restore_generation(
    client: &BackupClient,
    gen: &LocalGeneration,
    fileid: i64,
    entry: &FilesystemEntry,
    to: &Path,
    progress: &ProgressBar,
) -> RestoreResult<()> {
    info!("restoring {:?}", entry);
    progress.set_message(&format!("{}", entry.pathbuf().display()));
    progress.inc(1);

    let to = restored_path(entry, to)?;
    match entry.kind() {
        FilesystemKind::Regular => restore_regular(client, &gen, &to, fileid, &entry)?,
        FilesystemKind::Directory => restore_directory(&to)?,
        FilesystemKind::Symlink => restore_symlink(&to, &entry)?,
        FilesystemKind::Socket => restore_socket(&to, &entry)?,
        FilesystemKind::Fifo => restore_fifo(&to, &entry)?,
    }
    Ok(())
}

fn restore_directory(path: &Path) -> RestoreResult<()> {
    debug!("restoring directory {}", path.display());
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn restore_directory_metadata(entry: &FilesystemEntry, to: &Path) -> RestoreResult<()> {
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

fn restored_path(entry: &FilesystemEntry, to: &Path) -> RestoreResult<PathBuf> {
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
    gen: &LocalGeneration,
    path: &Path,
    fileid: i64,
    entry: &FilesystemEntry,
) -> RestoreResult<()> {
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

fn restore_symlink(path: &Path, entry: &FilesystemEntry) -> RestoreResult<()> {
    debug!("restoring symlink {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    symlink(entry.symlink_target().unwrap(), path)?;
    debug!("restored symlink {}", path.display());
    Ok(())
}

fn restore_socket(path: &Path, entry: &FilesystemEntry) -> RestoreResult<()> {
    debug!("creating Unix domain socket {:?}", path);
    UnixListener::bind(path)?;
    restore_metadata(path, entry)?;
    Ok(())
}

fn restore_fifo(path: &Path, entry: &FilesystemEntry) -> RestoreResult<()> {
    debug!("creating fifo {:?}", path);
    let filename = path_to_cstring(path);
    match unsafe { mkfifo(filename.as_ptr(), 0) } {
        -1 => {
            return Err(RestoreError::NamedPipeCreationError(path.to_path_buf()));
        }
        _ => restore_metadata(path, entry)?,
    }
    Ok(())
}

fn restore_metadata(path: &Path, entry: &FilesystemEntry) -> RestoreResult<()> {
    debug!("restoring metadata for {}", entry.pathbuf().display());

    debug!("restoring metadata for {:?}", path);

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

    let path = path_to_cstring(path);

    // We have to use unsafe here to be able call the libc functions
    // below.
    unsafe {
        debug!("chmod {:?}", path);
        if chmod(path.as_ptr(), entry.mode()) == -1 {
            let error = Error::last_os_error();
            error!("chmod failed on {:?}", path);
            return Err(error.into());
        }

        debug!("utimens {:?}", path);
        if utimensat(AT_FDCWD, path.as_ptr(), times, 0) == -1 {
            let error = Error::last_os_error();
            error!("utimensat failed on {:?}", path);
            return Err(error.into());
        }
    }
    Ok(())
}

fn path_to_cstring(path: &Path) -> CString {
    let path = path.as_os_str();
    let path = path.as_bytes();
    CString::new(path).unwrap()
}

fn create_progress_bar(file_count: i64, verbose: bool) -> ProgressBar {
    let progress = if verbose {
        ProgressBar::new(file_count as u64)
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

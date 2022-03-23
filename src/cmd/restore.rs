//! The `restore` subcommand.

use crate::backup_reason::Reason;
use crate::chunk::ClientTrust;
use crate::client::{BackupClient, ClientError};
use crate::config::ClientConfig;
use crate::db::DatabaseError;
use crate::dbgen::FileId;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use crate::generation::{LocalGeneration, LocalGenerationError};
use indicatif::{ProgressBar, ProgressStyle};
use libc::{chmod, mkfifo, timespec, utimensat, AT_FDCWD, AT_SYMLINK_NOFOLLOW};
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
use tokio::runtime::Runtime;

/// Restore a backup.
#[derive(Debug, StructOpt)]
pub struct Restore {
    /// Reference to generation to restore.
    #[structopt()]
    gen_id: String,

    /// Path to directory where restored files are written.
    #[structopt(parse(from_os_str))]
    to: PathBuf,
}

impl Restore {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let temp = NamedTempFile::new()?;

        let client = BackupClient::new(config)?;
        let trust = client
            .get_client_trust()
            .await?
            .or_else(|| Some(ClientTrust::new("FIXME", None, "".to_string(), vec![])))
            .unwrap();

        let genlist = client.list_generations(&trust);
        let gen_id = genlist.resolve(&self.gen_id)?;
        info!("generation id is {}", gen_id.as_chunk_id());

        let gen = client.fetch_generation(&gen_id, temp.path()).await?;
        info!("restoring {} files", gen.file_count()?);
        let progress = create_progress_bar(gen.file_count()?, true);
        for file in gen.files()?.iter()? {
            let (fileno, entry, reason, _) = file?;
            match reason {
                Reason::FileError => (),
                _ => restore_generation(&client, &gen, fileno, &entry, &self.to, &progress).await?,
            }
        }
        for file in gen.files()?.iter()? {
            let (_, entry, _, _) = file?;
            if entry.is_dir() {
                restore_directory_metadata(&entry, &self.to)?;
            }
        }
        progress.finish();

        Ok(())
    }
}

/// Possible errors from restoring.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// An error using a Database.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Failed to create a name pipe.
    #[error("Could not create named pipe (FIFO) {0}")]
    NamedPipeCreationError(PathBuf),

    /// Error from HTTP client.
    #[error(transparent)]
    ClientError(#[from] ClientError),

    /// Error from local generation.
    #[error(transparent)]
    LocalGenerationError(#[from] LocalGenerationError),

    /// Error removing a prefix.
    #[error(transparent)]
    StripPrefixError(#[from] StripPrefixError),

    /// Error creating a directory.
    #[error("failed to create directory {0}: {1}")]
    CreateDirs(PathBuf, std::io::Error),

    /// Error creating a file.
    #[error("failed to create file {0}: {1}")]
    CreateFile(PathBuf, std::io::Error),

    /// Error writing a file.
    #[error("failed to write file {0}: {1}")]
    WriteFile(PathBuf, std::io::Error),

    /// Error creating a symbolic link.
    #[error("failed to create symbolic link {0}: {1}")]
    Symlink(PathBuf, std::io::Error),

    /// Error creating a UNIX domain socket.
    #[error("failed to create UNIX domain socket {0}: {1}")]
    UnixBind(PathBuf, std::io::Error),

    /// Error setting permissions.
    #[error("failed to set permissions for {0}: {1}")]
    Chmod(PathBuf, std::io::Error),

    /// Error settting timestamp.
    #[error("failed to set timestamp for {0}: {1}")]
    SetTimestamp(PathBuf, std::io::Error),
}

async fn restore_generation(
    client: &BackupClient,
    gen: &LocalGeneration,
    fileid: FileId,
    entry: &FilesystemEntry,
    to: &Path,
    progress: &ProgressBar,
) -> Result<(), RestoreError> {
    info!("restoring {:?}", entry);
    progress.set_message(format!("{}", entry.pathbuf().display()));
    progress.inc(1);

    let to = restored_path(entry, to)?;
    match entry.kind() {
        FilesystemKind::Regular => restore_regular(client, gen, &to, fileid, entry).await?,
        FilesystemKind::Directory => restore_directory(&to)?,
        FilesystemKind::Symlink => restore_symlink(&to, entry)?,
        FilesystemKind::Socket => restore_socket(&to, entry)?,
        FilesystemKind::Fifo => restore_fifo(&to, entry)?,
    }
    Ok(())
}

fn restore_directory(path: &Path) -> Result<(), RestoreError> {
    debug!("restoring directory {}", path.display());
    std::fs::create_dir_all(path)
        .map_err(|err| RestoreError::CreateDirs(path.to_path_buf(), err))?;
    Ok(())
}

fn restore_directory_metadata(entry: &FilesystemEntry, to: &Path) -> Result<(), RestoreError> {
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

fn restored_path(entry: &FilesystemEntry, to: &Path) -> Result<PathBuf, RestoreError> {
    let path = &entry.pathbuf();
    let path = if path.is_absolute() {
        path.strip_prefix("/")?
    } else {
        path
    };
    Ok(to.join(path))
}

async fn restore_regular(
    client: &BackupClient,
    gen: &LocalGeneration,
    path: &Path,
    fileid: FileId,
    entry: &FilesystemEntry,
) -> Result<(), RestoreError> {
    debug!("restoring regular {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    std::fs::create_dir_all(parent)
        .map_err(|err| RestoreError::CreateDirs(parent.to_path_buf(), err))?;
    {
        let mut file = std::fs::File::create(path)
            .map_err(|err| RestoreError::CreateFile(path.to_path_buf(), err))?;
        for chunkid in gen.chunkids(fileid)?.iter()? {
            let chunkid = chunkid?;
            let chunk = client.fetch_chunk(&chunkid).await?;
            file.write_all(chunk.data())
                .map_err(|err| RestoreError::WriteFile(path.to_path_buf(), err))?;
        }
        restore_metadata(path, entry)?;
    }
    debug!("restored regular {}", path.display());
    Ok(())
}

fn restore_symlink(path: &Path, entry: &FilesystemEntry) -> Result<(), RestoreError> {
    debug!("restoring symlink {}", path.display());
    let parent = path.parent().unwrap();
    debug!("  mkdir {}", parent.display());
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|err| RestoreError::CreateDirs(parent.to_path_buf(), err))?;
    }
    symlink(entry.symlink_target().unwrap(), path)
        .map_err(|err| RestoreError::Symlink(path.to_path_buf(), err))?;
    restore_metadata(path, entry)?;
    debug!("restored symlink {}", path.display());
    Ok(())
}

fn restore_socket(path: &Path, entry: &FilesystemEntry) -> Result<(), RestoreError> {
    debug!("creating Unix domain socket {:?}", path);
    UnixListener::bind(path).map_err(|err| RestoreError::UnixBind(path.to_path_buf(), err))?;
    restore_metadata(path, entry)?;
    Ok(())
}

fn restore_fifo(path: &Path, entry: &FilesystemEntry) -> Result<(), RestoreError> {
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

fn restore_metadata(path: &Path, entry: &FilesystemEntry) -> Result<(), RestoreError> {
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

    let pathbuf = path.to_path_buf();
    let path = path_to_cstring(path);

    // We have to use unsafe here to be able call the libc functions
    // below.
    unsafe {
        if entry.kind() != FilesystemKind::Symlink {
            debug!("chmod {:?}", path);
            if chmod(path.as_ptr(), entry.mode() as libc::mode_t) == -1 {
                let error = Error::last_os_error();
                error!("chmod failed on {:?}", path);
                return Err(RestoreError::Chmod(pathbuf, error));
            }
        } else {
            debug!(
                "skipping chmod of a symlink because it'll attempt to change the pointed-at file"
            );
        }

        debug!("utimens {:?}", path);
        if utimensat(AT_FDCWD, path.as_ptr(), times, AT_SYMLINK_NOFOLLOW) == -1 {
            let error = Error::last_os_error();
            error!("utimensat failed on {:?}", path);
            return Err(RestoreError::SetTimestamp(pathbuf, error));
        }
    }
    Ok(())
}

fn path_to_cstring(path: &Path) -> CString {
    let path = path.as_os_str();
    let path = path.as_bytes();
    CString::new(path).unwrap()
}

fn create_progress_bar(file_count: FileId, verbose: bool) -> ProgressBar {
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

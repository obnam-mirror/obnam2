use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::read_link;
use std::fs::{FileType, Metadata};
use std::os::linux::fs::MetadataExt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

/// A file system entry.
///
/// Represent all backup-relevant the metadata about a file system
/// object: fully qualified pathname, type, length (if applicable),
/// etc. Everything except the content of a regular file or the
/// contents of a directory.
///
/// This is everything Obnam cares about each file system object, when
/// making a backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemEntry {
    kind: FilesystemKind,
    path: Vec<u8>,
    len: u64,

    // 16 bits should be enough for a Unix mode_t.
    // https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_stat.h.html
    //  However, it's 32 bits on Linux, so that's what we store.
    mode: u32,

    // Linux can store file system time stamps in nanosecond
    // resolution. We store them as two 64-bit integers.
    mtime: i64,
    mtime_ns: i64,
    atime: i64,
    atime_ns: i64,

    // The target of a symbolic link, if any.
    symlink_target: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum FsEntryError {
    #[error("Unknown file kind {0}")]
    UnknownFileKindCode(u8),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub type FsEntryResult<T> = Result<T, FsEntryError>;

#[allow(clippy::len_without_is_empty)]
impl FilesystemEntry {
    pub fn from_metadata(path: &Path, meta: &Metadata) -> FsEntryResult<Self> {
        let kind = FilesystemKind::from_file_type(meta.file_type());
        let symlink_target = if kind == FilesystemKind::Symlink {
            debug!("reading symlink target for {:?}", path);
            let target = match read_link(path) {
                Ok(x) => x,
                Err(err) => {
                    error!("read_link failed: {}", err);
                    return Err(err.into());
                }
            };
            Some(target)
        } else {
            None
        };
        Ok(Self {
            path: path.to_path_buf().into_os_string().into_vec(),
            kind: FilesystemKind::from_file_type(meta.file_type()),
            len: meta.len(),
            mode: meta.st_mode(),
            mtime: meta.st_mtime(),
            mtime_ns: meta.st_mtime_nsec(),
            atime: meta.st_atime(),
            atime_ns: meta.st_atime_nsec(),
            symlink_target,
        })
    }

    pub fn kind(&self) -> FilesystemKind {
        self.kind
    }

    pub fn pathbuf(&self) -> PathBuf {
        let path = self.path.clone();
        PathBuf::from(OsString::from_vec(path))
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn mode(&self) -> u32 {
        self.mode
    }

    pub fn atime(&self) -> i64 {
        self.atime
    }

    pub fn atime_ns(&self) -> i64 {
        self.atime_ns
    }

    pub fn mtime(&self) -> i64 {
        self.mtime
    }

    pub fn mtime_ns(&self) -> i64 {
        self.mtime_ns
    }

    pub fn is_dir(&self) -> bool {
        self.kind() == FilesystemKind::Directory
    }

    pub fn symlink_target(&self) -> Option<PathBuf> {
        self.symlink_target.clone()
    }
}

/// Different types of file system entries.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilesystemKind {
    Regular,
    Directory,
    Symlink,
    Socket,
}

impl FilesystemKind {
    pub fn from_file_type(file_type: FileType) -> Self {
        if file_type.is_file() {
            FilesystemKind::Regular
        } else if file_type.is_dir() {
            FilesystemKind::Directory
        } else if file_type.is_symlink() {
            FilesystemKind::Symlink
        } else if file_type.is_socket() {
            FilesystemKind::Socket
        } else {
            panic!("unknown file type {:?}", file_type);
        }
    }

    pub fn as_code(&self) -> u8 {
        match self {
            FilesystemKind::Regular => 0,
            FilesystemKind::Directory => 1,
            FilesystemKind::Symlink => 2,
            FilesystemKind::Socket => 3,
        }
    }

    pub fn from_code(code: u8) -> FsEntryResult<Self> {
        match code {
            0 => Ok(FilesystemKind::Regular),
            1 => Ok(FilesystemKind::Directory),
            2 => Ok(FilesystemKind::Symlink),
            3 => Ok(FilesystemKind::Socket),
            _ => Err(FsEntryError::UnknownFileKindCode(code).into()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown file kind code {0}")]
    UnknownFileKindCode(u8),
}

#[cfg(test)]
mod test {
    use super::FilesystemKind;

    #[test]
    fn file_kind_regular_round_trips() {
        one_file_kind_round_trip(FilesystemKind::Regular);
        one_file_kind_round_trip(FilesystemKind::Directory);
        one_file_kind_round_trip(FilesystemKind::Symlink);
        one_file_kind_round_trip(FilesystemKind::Socket);
    }

    fn one_file_kind_round_trip(kind: FilesystemKind) {
        assert_eq!(kind, FilesystemKind::from_code(kind.as_code()).unwrap());
    }
}

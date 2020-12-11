use serde::{Deserialize, Serialize};
use std::fs::{FileType, Metadata};
use std::os::linux::fs::MetadataExt;
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
#[derive(Debug, Serialize, Deserialize)]
pub struct FilesystemEntry {
    kind: FilesystemKind,
    path: PathBuf,
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
}

#[allow(clippy::len_without_is_empty)]
impl FilesystemEntry {
    pub fn from_metadata(path: &Path, meta: &Metadata) -> Self {
        Self {
            path: path.to_path_buf(),
            kind: FilesystemKind::from_file_type(meta.file_type()),
            len: meta.len(),
            mode: meta.st_mode(),
            mtime: meta.st_mtime(),
            mtime_ns: meta.st_mtime_nsec(),
            atime: meta.st_atime(),
            atime_ns: meta.st_atime_nsec(),
        }
    }

    pub fn kind(&self) -> FilesystemKind {
        self.kind
    }

    pub fn path(&self) -> &Path {
        &self.path
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
}

/// Different types of file system entries.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilesystemKind {
    Regular,
    Directory,
}

impl FilesystemKind {
    pub fn from_file_type(file_type: FileType) -> Self {
        if file_type.is_file() {
            FilesystemKind::Regular
        } else if file_type.is_dir() {
            FilesystemKind::Directory
        } else {
            panic!("unknown file type {:?}", file_type);
        }
    }

    pub fn as_code(&self) -> u8 {
        match self {
            FilesystemKind::Regular => 0,
            FilesystemKind::Directory => 1,
        }
    }

    pub fn from_code(code: u8) -> anyhow::Result<Self> {
        match code {
            0 => Ok(FilesystemKind::Regular),
            1 => Ok(FilesystemKind::Directory),
            _ => Err(Error::UnknownFileKindCode(code).into()),
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
    }

    fn one_file_kind_round_trip(kind: FilesystemKind) {
        assert_eq!(kind, FilesystemKind::from_code(kind.as_code()).unwrap());
    }
}

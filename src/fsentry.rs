use serde::{Deserialize, Serialize};
use std::fs::{FileType, Metadata};
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
}

#[allow(clippy::len_without_is_empty)]
impl FilesystemEntry {
    pub fn from_metadata(path: &Path, meta: &Metadata) -> Self {
        let path = path.to_path_buf();
        let kind = FilesystemKind::from_file_type(meta.file_type());
        let len = meta.len();
        Self { path, kind, len }
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

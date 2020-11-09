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
#[derive(Debug)]
pub struct FilesystemEntry {
    kind: FilesystemKind,
    path: PathBuf,
    len: u64,
}

#[allow(clippy::len_without_is_empty)]
impl FilesystemEntry {
    fn new(kind: FilesystemKind, path: &Path, len: u64) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
            len,
        }
    }

    pub fn regular<P>(path: P, len: u64) -> Self
    where
        P: AsRef<Path>,
    {
        Self::new(FilesystemKind::Regular, path.as_ref(), len)
    }

    pub fn directory<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self::new(FilesystemKind::Directory, path.as_ref(), 0)
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
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FilesystemKind {
    Regular,
    Directory,
}

impl FilesystemKind {
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
    use super::{FilesystemEntry, FilesystemKind};
    use std::path::Path;

    #[test]
    fn regular_file() {
        let filename = Path::new("foo.dat");
        let len = 123;
        let e = FilesystemEntry::regular(filename, len);
        assert_eq!(e.kind(), FilesystemKind::Regular);
        assert_eq!(e.path(), filename);
        assert_eq!(e.len(), len);
    }

    #[test]
    fn directory() {
        let filename = Path::new("foo.dat");
        let e = FilesystemEntry::directory(filename);
        assert_eq!(e.kind(), FilesystemKind::Directory);
        assert_eq!(e.path(), filename);
        assert_eq!(e.len(), 0);
    }

    #[test]
    fn file_kind_regular_round_trips() {
        one_file_kind_round_trip(FilesystemKind::Regular);
        one_file_kind_round_trip(FilesystemKind::Directory);
    }

    fn one_file_kind_round_trip(kind: FilesystemKind) {
        assert_eq!(kind, FilesystemKind::from_code(kind.as_code()).unwrap());
    }
}

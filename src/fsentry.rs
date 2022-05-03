//! An entry in the file system.

use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::read_link;
use std::fs::{FileType, Metadata};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use users::{Groups, Users, UsersCache};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;

#[cfg(target_os = "macos")]
use std::os::macos::fs::MetadataExt;

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

    // User and group owning the file. We store them as both the
    // numeric id and the textual name corresponding to the numeric id
    // at the time of the backup.
    uid: u32,
    gid: u32,
    user: String,
    group: String,
}

/// Possible errors related to file system entries.
#[derive(Debug, thiserror::Error)]
pub enum FsEntryError {
    /// File kind numeric representation is unknown.
    #[error("Unknown file kind {0}")]
    UnknownFileKindCode(u8),

    /// Failed to read a symbolic link's target.
    #[error("failed to read symbolic link target {0}: {1}")]
    ReadLink(PathBuf, std::io::Error),
}

#[allow(clippy::len_without_is_empty)]
impl FilesystemEntry {
    /// Create an `FsEntry` from a file's metadata.
    pub(crate) fn new(
        path: &Path,
        kind: FilesystemKind,
        uid: u32,
        gid: u32,
        len: u64,
        mode: u32,
        mtime: i64,
        mtime_ns: i64,
        atime: i64,
        atime_ns: i64,
        cache: &mut UsersCache,
    ) -> Result<Self, FsEntryError> {
        let symlink_target = if kind == FilesystemKind::Symlink {
            debug!("reading symlink target for {:?}", path);
            let target =
                read_link(path).map_err(|err| FsEntryError::ReadLink(path.to_path_buf(), err))?;
            Some(target)
        } else {
            None
        };

        let user: String = if let Some(user) = cache.get_user_by_uid(uid) {
            user.name().to_string_lossy().to_string()
        } else {
            "".to_string()
        };
        let group = if let Some(group) = cache.get_group_by_gid(gid) {
            group.name().to_string_lossy().to_string()
        } else {
            "".to_string()
        };

        Ok(Self {
            path: path.to_path_buf().into_os_string().into_vec(),
            kind,
            len,
            mode,
            mtime,
            mtime_ns,
            atime,
            atime_ns,
            symlink_target,
            uid,
            gid,
            user,
            group,
        })
    }

    /// Create an `FsEntry` from a file's metadata.
    pub fn from_metadata(
        path: &Path,
        meta: &Metadata,
        cache: &mut UsersCache,
    ) -> Result<Self, FsEntryError> {
        let kind = FilesystemKind::from_file_type(meta.file_type());
        Self::new(
            path,
            kind,
            meta.st_uid(),
            meta.st_gid(),
            meta.len(),
            meta.st_mode(),
            meta.st_mtime(),
            meta.st_mtime_nsec(),
            meta.st_atime(),
            meta.st_atime_nsec(),
            cache,
        )
    }

    /// Return the kind of file the entry refers to.
    pub fn kind(&self) -> FilesystemKind {
        self.kind
    }

    /// Return full path to the entry.
    pub fn pathbuf(&self) -> PathBuf {
        let path = self.path.clone();
        PathBuf::from(OsString::from_vec(path))
    }

    /// Return number of bytes for the entity represented by the entry.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Return the entry's mode bits.
    pub fn mode(&self) -> u32 {
        self.mode
    }

    /// Return the entry's access time, whole seconds.
    pub fn atime(&self) -> i64 {
        self.atime
    }

    /// Return the entry's access time, nanoseconds since the last full second.
    pub fn atime_ns(&self) -> i64 {
        self.atime_ns
    }

    /// Return the entry's modification time, whole seconds.
    pub fn mtime(&self) -> i64 {
        self.mtime
    }

    /// Return the entry's modification time, nanoseconds since the last full second.
    pub fn mtime_ns(&self) -> i64 {
        self.mtime_ns
    }

    /// Does the entry represent a directory?
    pub fn is_dir(&self) -> bool {
        self.kind() == FilesystemKind::Directory
    }

    /// Return target of the symlink the entry represents.
    pub fn symlink_target(&self) -> Option<PathBuf> {
        self.symlink_target.clone()
    }
}

/// Different types of file system entries.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilesystemKind {
    /// Regular file, including a hard link to one.
    Regular,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// A UNIX domain socket.
    Socket,
    /// A UNIX named pipe.
    Fifo,
}

impl FilesystemKind {
    /// Create a kind from a file type.
    pub fn from_file_type(file_type: FileType) -> Self {
        if file_type.is_file() {
            FilesystemKind::Regular
        } else if file_type.is_dir() {
            FilesystemKind::Directory
        } else if file_type.is_symlink() {
            FilesystemKind::Symlink
        } else if file_type.is_socket() {
            FilesystemKind::Socket
        } else if file_type.is_fifo() {
            FilesystemKind::Fifo
        } else {
            panic!("unknown file type {:?}", file_type);
        }
    }

    /// Represent a kind as a numeric code.
    pub fn as_code(&self) -> u8 {
        match self {
            FilesystemKind::Regular => 0,
            FilesystemKind::Directory => 1,
            FilesystemKind::Symlink => 2,
            FilesystemKind::Socket => 3,
            FilesystemKind::Fifo => 4,
        }
    }

    /// Create a kind from a numeric code.
    pub fn from_code(code: u8) -> Result<Self, FsEntryError> {
        match code {
            0 => Ok(FilesystemKind::Regular),
            1 => Ok(FilesystemKind::Directory),
            2 => Ok(FilesystemKind::Symlink),
            3 => Ok(FilesystemKind::Socket),
            4 => Ok(FilesystemKind::Fifo),
            _ => Err(FsEntryError::UnknownFileKindCode(code)),
        }
    }
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
        one_file_kind_round_trip(FilesystemKind::Fifo);
    }

    fn one_file_kind_round_trip(kind: FilesystemKind) {
        assert_eq!(kind, FilesystemKind::from_code(kind.as_code()).unwrap());
    }
}

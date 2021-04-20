use crate::fsentry::{FilesystemEntry, FsEntryError};
use log::{debug, warn};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, IntoIter, WalkDir};

/// Iterator over file system entries in a directory tree.
pub struct FsIterator {
    iter: SkipCachedirs,
}

#[derive(Debug, thiserror::Error)]
pub enum FsIterError {
    #[error(transparent)]
    WalkError(#[from] walkdir::Error),

    #[error("I/O error on {0}: {1}")]
    IoError(PathBuf, #[source] std::io::Error),

    #[error(transparent)]
    FsEntryError(#[from] FsEntryError),
}

pub type FsIterResult<T> = Result<T, FsIterError>;

impl FsIterator {
    pub fn new(root: &Path, exclude_cache_tag_directories: bool) -> Self {
        Self {
            iter: SkipCachedirs::new(
                WalkDir::new(root).into_iter(),
                exclude_cache_tag_directories,
            ),
        }
    }
}

impl Iterator for FsIterator {
    type Item = FsIterResult<FilesystemEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Cachedir-aware adaptor for WalkDir: it skips the contents of dirs that contain CACHEDIR.TAG,
/// but still yields entries for the dir and the tag themselves.
struct SkipCachedirs {
    iter: IntoIter,
    exclude_cache_tag_directories: bool,
    // This is the last tag we've found. `next()` will yield it before asking `iter` for more
    // entries.
    cachedir_tag: Option<FsIterResult<FilesystemEntry>>,
}

impl SkipCachedirs {
    fn new(iter: IntoIter, exclude_cache_tag_directories: bool) -> Self {
        Self {
            iter,
            exclude_cache_tag_directories,
            cachedir_tag: None,
        }
    }

    fn try_enqueue_cachedir_tag(&mut self, entry: &DirEntry) {
        if !self.exclude_cache_tag_directories {
            return;
        }

        // If this entry is not a directory, it means we already processed its
        // parent dir and decided that it's not cached.
        if !entry.file_type().is_dir() {
            return;
        }

        let mut tag_path = entry.path().to_owned();
        tag_path.push("CACHEDIR.TAG");

        // Tags are required to be regular files -- not even symlinks are allowed.
        if !tag_path.is_file() {
            return;
        };

        const CACHEDIR_TAG: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";
        let mut content = [0u8; CACHEDIR_TAG.len()];

        let mut file = if let Ok(file) = std::fs::File::open(&tag_path) {
            file
        } else {
            return;
        };

        use std::io::Read;
        match file.read_exact(&mut content) {
            Ok(_) => (),
            // If we can't read the tag file, proceed as if's not there
            Err(_) => return,
        }

        if content == CACHEDIR_TAG {
            self.iter.skip_current_dir();
            self.cachedir_tag = Some(new_entry(&tag_path));
        }
    }
}

impl Iterator for SkipCachedirs {
    type Item = FsIterResult<FilesystemEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.cachedir_tag.take().or_else(|| {
            let next = self.iter.next();
            debug!("walkdir found: {:?}", next);
            match next {
                None => None,
                Some(Err(err)) => Some(Err(err.into())),
                Some(Ok(entry)) => {
                    self.try_enqueue_cachedir_tag(&entry);
                    Some(new_entry(entry.path()))
                }
            }
        })
    }
}

fn new_entry(path: &Path) -> FsIterResult<FilesystemEntry> {
    let meta = std::fs::symlink_metadata(path);
    debug!("metadata for {:?}: {:?}", path, meta);
    let meta = match meta {
        Ok(meta) => meta,
        Err(err) => {
            warn!("failed to get metadata for {}: {}", path.display(), err);
            return Err(FsIterError::IoError(path.to_path_buf(), err));
        }
    };
    let entry = FilesystemEntry::from_metadata(path, &meta)?;
    debug!("FileSystemEntry for {:?}: {:?}", path, entry);
    Ok(entry)
}

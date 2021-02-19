use crate::fsentry::{FilesystemEntry, FsEntryError};
use log::{debug, error};
use std::path::Path;
use walkdir::{DirEntry, IntoIter, WalkDir};

/// Iterator over file system entries in a directory tree.
pub struct FsIterator {
    iter: IntoIter,
}

#[derive(Debug, thiserror::Error)]
pub enum FsIterError {
    #[error(transparent)]
    WalkError(#[from] walkdir::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    FsEntryError(#[from] FsEntryError),
}

pub type FsIterResult<T> = Result<T, FsIterError>;

impl FsIterator {
    pub fn new(root: &Path) -> Self {
        Self {
            iter: WalkDir::new(root).into_iter(),
        }
    }
}

impl Iterator for FsIterator {
    type Item = FsIterResult<FilesystemEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iter.next();
        debug!("walkdir found: {:?}", next);
        match next {
            None => None,
            Some(Ok(entry)) => Some(new_entry(&entry)),
            Some(Err(err)) => Some(Err(err.into())),
        }
    }
}

fn new_entry(e: &DirEntry) -> FsIterResult<FilesystemEntry> {
    let path = e.path();
    let meta = std::fs::metadata(path);
    debug!("metadata for {:?}: {:?}", path, meta);
    let meta = match meta {
        Ok(meta) => meta,
        Err(err) => {
            error!("failed to get metadata: {}", err);
            return Err(err.into());
        }
    };
    let entry = FilesystemEntry::from_metadata(path, &meta)?;
    debug!("FileSystemEntry for {:?}: {:?}", path, entry);
    Ok(entry)
}

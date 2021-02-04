use crate::fsentry::{FilesystemEntry, FsEntryError};
use log::info;
use std::path::Path;
use walkdir::{IntoIter, WalkDir};

/// Iterator over file system entries in a directory tree.
pub struct FsIterator {
    iter: IntoIter,
}

#[derive(Debug, thiserror::Error)]
pub enum FsIterError {
    #[error(transparent)]
    WalkError(#[from] walkdir::Error),

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
        match self.iter.next() {
            None => None,
            Some(Ok(entry)) => {
                info!("found {}", entry.path().display());
                Some(new_entry(&entry))
            }
            Some(Err(err)) => Some(Err(err.into())),
        }
    }
}

fn new_entry(e: &walkdir::DirEntry) -> FsIterResult<FilesystemEntry> {
    let meta = e.metadata()?;
    let entry = FilesystemEntry::from_metadata(e.path(), &meta)?;
    Ok(entry)
}

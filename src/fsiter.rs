use crate::fsentry::FilesystemEntry;
use log::info;
use std::path::Path;
use walkdir::{IntoIter, WalkDir};

/// Iterator over file system entries in a directory tree.
pub struct FsIterator {
    iter: IntoIter,
}

impl FsIterator {
    pub fn new(root: &Path) -> Self {
        Self {
            iter: WalkDir::new(root).into_iter(),
        }
    }
}

impl Iterator for FsIterator {
    type Item = Result<FilesystemEntry, anyhow::Error>;
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

fn new_entry(e: &walkdir::DirEntry) -> anyhow::Result<FilesystemEntry> {
    let meta = e.metadata()?;
    let entry = FilesystemEntry::from_metadata(e.path(), &meta)?;
    Ok(entry)
}

use crate::fsentry::FilesystemEntry;
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
            Some(Ok(entry)) => Some(new_entry(&entry)),
            Some(Err(err)) => Some(Err(err.into())),
        }
    }
}

fn new_entry(e: &walkdir::DirEntry) -> anyhow::Result<FilesystemEntry> {
    let meta = e.metadata()?;
    let kind = if meta.is_dir() {
        FilesystemEntry::directory(e.path())
    } else {
        FilesystemEntry::regular(e.path(), meta.len())
    };
    Ok(kind)
}

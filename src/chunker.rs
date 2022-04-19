//! Split file data into chunks.

use crate::chunk::DataChunk;
use crate::chunkmeta::ChunkMeta;
use crate::label::{Label, LabelChecksumKind};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

/// Iterator over chunks in a file.
pub struct FileChunks {
    chunk_size: usize,
    kind: LabelChecksumKind,
    buf: Vec<u8>,
    filename: PathBuf,
    handle: std::fs::File,
}

/// Possible errors from data chunking.
#[derive(Debug, thiserror::Error)]
pub enum ChunkerError {
    /// Error reading from a file.
    #[error("failed to read file {0}: {1}")]
    FileRead(PathBuf, std::io::Error),
}

impl FileChunks {
    /// Create new iterator.
    pub fn new(
        chunk_size: usize,
        handle: std::fs::File,
        filename: &Path,
        kind: LabelChecksumKind,
    ) -> Self {
        let mut buf = vec![];
        buf.resize(chunk_size, 0);
        Self {
            chunk_size,
            kind,
            buf,
            handle,
            filename: filename.to_path_buf(),
        }
    }

    fn read_chunk(&mut self) -> Result<Option<DataChunk>, ChunkerError> {
        let mut used = 0;

        loop {
            let n = self
                .handle
                .read(&mut self.buf.as_mut_slice()[used..])
                .map_err(|err| ChunkerError::FileRead(self.filename.to_path_buf(), err))?;
            used += n;
            if n == 0 || used == self.chunk_size {
                break;
            }
        }

        if used == 0 {
            return Ok(None);
        }

        let buffer = &self.buf.as_slice()[..used];
        let hash = match self.kind {
            LabelChecksumKind::Blake2 => Label::blake2(buffer),
            LabelChecksumKind::Sha256 => Label::sha256(buffer),
        };
        let meta = ChunkMeta::new(&hash);
        let chunk = DataChunk::new(buffer.to_vec(), meta);
        Ok(Some(chunk))
    }
}

impl Iterator for FileChunks {
    type Item = Result<DataChunk, ChunkerError>;

    /// Return the next chunk, if any, or an error.
    fn next(&mut self) -> Option<Result<DataChunk, ChunkerError>> {
        match self.read_chunk() {
            Ok(None) => None,
            Ok(Some(chunk)) => Some(Ok(chunk)),
            Err(e) => Some(Err(e)),
        }
    }
}

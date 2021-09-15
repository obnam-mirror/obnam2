use crate::checksummer::Checksum;
use crate::chunk::DataChunk;
use crate::chunkmeta::ChunkMeta;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

pub struct Chunker {
    chunk_size: usize,
    buf: Vec<u8>,
    filename: PathBuf,
    handle: std::fs::File,
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkerError {
    #[error("failed to read file {0}: {1}")]
    FileRead(PathBuf, std::io::Error),
}

impl Chunker {
    pub fn new(chunk_size: usize, handle: std::fs::File, filename: &Path) -> Self {
        let mut buf = vec![];
        buf.resize(chunk_size, 0);
        Self {
            chunk_size,
            buf,
            handle,
            filename: filename.to_path_buf(),
        }
    }

    pub fn read_chunk(&mut self) -> Result<Option<DataChunk>, ChunkerError> {
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
        let hash = Checksum::sha256(buffer);
        let meta = ChunkMeta::new(&hash);
        let chunk = DataChunk::new(buffer.to_vec(), meta);
        Ok(Some(chunk))
    }
}

impl Iterator for Chunker {
    type Item = Result<DataChunk, ChunkerError>;

    fn next(&mut self) -> Option<Result<DataChunk, ChunkerError>> {
        match self.read_chunk() {
            Ok(None) => None,
            Ok(Some(chunk)) => Some(Ok(chunk)),
            Err(e) => Some(Err(e)),
        }
    }
}

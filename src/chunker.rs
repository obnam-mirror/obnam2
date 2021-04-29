use crate::checksummer::sha256;
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

pub type ChunkerResult<T> = Result<T, ChunkerError>;

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

    pub fn read_chunk(&mut self) -> ChunkerResult<Option<(ChunkMeta, DataChunk)>> {
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
        let hash = sha256(buffer);
        let meta = ChunkMeta::new(&hash);
        let chunk = DataChunk::new(buffer.to_vec());
        Ok(Some((meta, chunk)))
    }
}

impl Iterator for Chunker {
    type Item = ChunkerResult<(ChunkMeta, DataChunk)>;

    fn next(&mut self) -> Option<ChunkerResult<(ChunkMeta, DataChunk)>> {
        match self.read_chunk() {
            Ok(None) => None,
            Ok(Some((meta, chunk))) => Some(Ok((meta, chunk))),
            Err(e) => Some(Err(e)),
        }
    }
}

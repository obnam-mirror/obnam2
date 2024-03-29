//! The `chunkify` subcommand.

use crate::config::ClientConfig;
use crate::engine::Engine;
use crate::error::ObnamError;
use crate::workqueue::WorkQueue;
use clap::Parser;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

// Size of queue with unprocessed chunks, and also queue of computed
// checksums.
const Q: usize = 8;

/// Split files into chunks and show their metadata.
#[derive(Debug, Parser)]
pub struct Chunkify {
    /// Names of files to split into chunks.
    filenames: Vec<PathBuf>,
}

impl Chunkify {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let mut q = WorkQueue::new(Q);
        for filename in self.filenames.iter() {
            tokio::spawn(split_file(
                filename.to_path_buf(),
                config.chunk_size,
                q.push(),
            ));
        }
        q.close();

        let mut summer = Engine::new(q, just_hash);

        let mut checksums = vec![];
        while let Some(sum) = summer.next().await {
            checksums.push(sum);
        }

        println!("{}", serde_json::to_string_pretty(&checksums)?);

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Chunk {
    filename: PathBuf,
    offset: u64,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct Checksum {
    filename: PathBuf,
    offset: u64,
    pub len: u64,
    checksum: String,
}

async fn split_file(filename: PathBuf, chunk_size: usize, tx: mpsc::Sender<Chunk>) {
    // println!("split_file {}", filename.display());
    let mut file = BufReader::new(File::open(&*filename).await.unwrap());

    let mut offset = 0;
    loop {
        let mut data = vec![0; chunk_size];
        let n = file.read(&mut data).await.unwrap();
        if n == 0 {
            break;
        }
        let data: Vec<u8> = data[..n].to_vec();

        let chunk = Chunk {
            filename: filename.clone(),
            offset,
            data,
        };
        tx.send(chunk).await.unwrap();
        // println!("split_file sent chunk at offset {}", offset);

        offset += n as u64;
    }
    // println!("split_file EOF at {}", offset);
}

fn just_hash(chunk: Chunk) -> Checksum {
    let mut hasher = Sha256::new();
    hasher.update(&chunk.data);
    let hash = hasher.finalize();
    let hash = format!("{:x}", hash);
    Checksum {
        filename: chunk.filename,
        offset: chunk.offset,
        len: chunk.data.len() as u64,
        checksum: hash,
    }
}

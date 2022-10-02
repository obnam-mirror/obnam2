//! The `get-chunk` subcommand.

use crate::chunkid::ChunkId;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use clap::Parser;
use std::io::{stdout, Write};
use tokio::runtime::Runtime;

/// Fetch a chunk from the server.
#[derive(Debug, Parser)]
pub struct GetChunk {
    /// Identifier of chunk to fetch.
    chunk_id: String,
}

impl GetChunk {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let rt = Runtime::new()?;
        rt.block_on(self.run_async(config))
    }

    async fn run_async(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let client = BackupClient::new(config)?;
        let chunk_id: ChunkId = self.chunk_id.parse().unwrap();
        let chunk = client.fetch_chunk(&chunk_id).await?;
        let stdout = stdout();
        let mut handle = stdout.lock();
        handle.write_all(chunk.data())?;
        Ok(())
    }
}

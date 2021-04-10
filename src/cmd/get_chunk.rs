use crate::chunkid::ChunkId;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use std::io::{stdout, Write};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct GetChunk {
    #[structopt()]
    chunk_id: String,
}

impl GetChunk {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let client = BackupClient::new(config)?;
        let chunk_id: ChunkId = self.chunk_id.parse().unwrap();
        let chunk = client.fetch_chunk(&chunk_id)?;

        let stdout = stdout();
        let mut handle = stdout.lock();
        handle.write_all(chunk.data())?;
        Ok(())
    }
}

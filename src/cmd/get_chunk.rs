use crate::chunkid::ChunkId;
use crate::client::BackupClient;
use crate::client::ClientConfig;
use crate::error::ObnamError;
use std::io::{stdout, Write};

pub fn get_chunk(config: &ClientConfig, chunk_id: &str) -> Result<(), ObnamError> {
    let client = BackupClient::new(config)?;
    let chunk_id: ChunkId = chunk_id.parse().unwrap();
    let chunk = client.fetch_chunk(&chunk_id)?;

    let stdout = stdout();
    let mut handle = stdout.lock();
    handle.write_all(chunk.data())?;
    Ok(())
}

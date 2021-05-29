use crate::chunk::DataChunk;
use crate::chunkmeta::ChunkMeta;
use crate::cipher::CipherEngine;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct EncryptChunk {
    #[structopt(parse(from_os_str))]
    filename: PathBuf,

    #[structopt(parse(from_os_str))]
    output: PathBuf,

    #[structopt()]
    json: String,
}

impl EncryptChunk {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let pass = config.passwords()?;
        let cipher = CipherEngine::new(&pass);

        let meta = ChunkMeta::from_json(&self.json)?;

        let cleartext = std::fs::read(&self.filename)?;
        let chunk = DataChunk::new(cleartext, meta);
        let encrypted = cipher.encrypt_chunk(&chunk)?;

        std::fs::write(&self.output, encrypted.ciphertext())?;

        Ok(())
    }
}

#[derive(Debug, StructOpt)]
pub struct DecryptChunk {
    #[structopt(parse(from_os_str))]
    filename: PathBuf,

    #[structopt(parse(from_os_str))]
    output: PathBuf,

    #[structopt()]
    json: String,
}

impl DecryptChunk {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let pass = config.passwords()?;
        let cipher = CipherEngine::new(&pass);

        let meta = ChunkMeta::from_json(&self.json)?;

        let encrypted = std::fs::read(&self.filename)?;
        let chunk = cipher.decrypt_chunk(&encrypted, &meta.to_json_vec())?;

        std::fs::write(&self.output, chunk.data())?;

        Ok(())
    }
}

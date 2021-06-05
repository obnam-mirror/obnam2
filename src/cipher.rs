use crate::chunk::DataChunk;
use crate::chunkmeta::ChunkMeta;
use crate::passwords::Passwords;

use aes_gcm::aead::{generic_array::GenericArray, Aead, NewAead, Payload};
use aes_gcm::Aes256Gcm; // Or `Aes128Gcm`
use rand::Rng;

use std::str::FromStr;

const CHUNK_V1: &[u8] = b"0001";

pub struct EncryptedChunk {
    ciphertext: Vec<u8>,
    aad: Vec<u8>,
}

impl EncryptedChunk {
    fn new(ciphertext: Vec<u8>, aad: Vec<u8>) -> Self {
        Self { ciphertext, aad }
    }

    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    pub fn aad(&self) -> &[u8] {
        &self.aad
    }
}

pub struct CipherEngine {
    cipher: Aes256Gcm,
}

impl CipherEngine {
    pub fn new(pass: &Passwords) -> Self {
        let key = GenericArray::from_slice(pass.encryption_key());
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn encrypt_chunk(&self, chunk: &DataChunk) -> Result<EncryptedChunk, CipherError> {
        // Payload with metadata as associated data, to be encrypted.
        //
        // The metadata will be stored in cleartext after encryption.
        let aad = chunk.meta().to_json_vec();
        let payload = Payload {
            msg: chunk.data(),
            aad: &aad,
        };

        // Unique random key for each encryption.
        let nonce = Nonce::new();
        let nonce_arr = GenericArray::from_slice(nonce.as_bytes());

        // Encrypt the sensitive part.
        let ciphertext = self
            .cipher
            .encrypt(nonce_arr, payload)
            .map_err(CipherError::EncryptError)?;

        // Construct the blob to be stored on the server.
        let mut vec: Vec<u8> = vec![];
        push_bytes(&mut vec, CHUNK_V1);
        push_bytes(&mut vec, nonce.as_bytes());
        push_bytes(&mut vec, &ciphertext);

        Ok(EncryptedChunk::new(vec, aad))
    }

    pub fn decrypt_chunk(&self, bytes: &[u8], meta: &[u8]) -> Result<DataChunk, CipherError> {
        // Does encrypted chunk start with the right version?
        if !bytes.starts_with(CHUNK_V1) {
            return Err(CipherError::UnknownChunkVersion);
        }
        let version_len = CHUNK_V1.len();
        let bytes = &bytes[version_len..];

        let (nonce, ciphertext) = match bytes.get(..NONCE_SIZE) {
            Some(nonce) => (GenericArray::from_slice(nonce), &bytes[NONCE_SIZE..]),
            None => return Err(CipherError::NoNonce),
        };

        let payload = Payload {
            msg: ciphertext,
            aad: meta,
        };

        let payload = self
            .cipher
            .decrypt(nonce, payload)
            .map_err(CipherError::DecryptError)?;
        let payload = Payload::from(payload.as_slice());

        let meta = std::str::from_utf8(meta)?;
        let meta = ChunkMeta::from_str(&meta)?;

        let chunk = DataChunk::new(payload.msg.to_vec(), meta);

        Ok(chunk)
    }
}

fn push_bytes(vec: &mut Vec<u8>, bytes: &[u8]) {
    for byte in bytes.iter() {
        vec.push(*byte);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CipherError {
    #[error("failed to encrypt with AES-GEM: {0}")]
    EncryptError(aes_gcm::Error),

    #[error("encrypted chunk does not start with correct version")]
    UnknownChunkVersion,

    #[error("encrypted chunk does not have a complete nonce")]
    NoNonce,

    #[error("failed to decrypt with AES-GEM: {0}")]
    DecryptError(aes_gcm::Error),

    #[error("failed to parse decrypted data as a DataChunk: {0}")]
    Parse(serde_yaml::Error),

    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
}

const NONCE_SIZE: usize = 12;

#[derive(Debug)]
struct Nonce {
    nonce: Vec<u8>,
}

impl Nonce {
    fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), NONCE_SIZE);
        Self {
            nonce: bytes.to_vec(),
        }
    }

    fn new() -> Self {
        let mut bytes: Vec<u8> = vec![0; NONCE_SIZE];
        let mut rng = rand::thread_rng();
        for x in bytes.iter_mut() {
            *x = rng.gen();
        }
        Self::from_bytes(&bytes)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.nonce
    }
}

#[cfg(test)]
mod test {
    use crate::chunk::DataChunk;
    use crate::chunkmeta::ChunkMeta;
    use crate::cipher::{CipherEngine, CipherError, CHUNK_V1, NONCE_SIZE};
    use crate::passwords::Passwords;

    #[test]
    fn metadata_as_aad() {
        let meta = ChunkMeta::new("dummy-checksum");
        let meta_as_aad = meta.to_json_vec();
        let chunk = DataChunk::new("hello".as_bytes().to_vec(), meta);
        let pass = Passwords::new("secret");
        let cipher = CipherEngine::new(&pass);
        let enc = cipher.encrypt_chunk(&chunk).unwrap();

        assert_eq!(meta_as_aad, enc.aad());
    }

    #[test]
    fn round_trip() {
        let meta = ChunkMeta::new("dummy-checksum");
        let chunk = DataChunk::new("hello".as_bytes().to_vec(), meta);
        let pass = Passwords::new("secret");

        let cipher = CipherEngine::new(&pass);
        let enc = cipher.encrypt_chunk(&chunk).unwrap();

        let bytes: Vec<u8> = enc.ciphertext().to_vec();
        let dec = cipher.decrypt_chunk(&bytes, enc.aad()).unwrap();
        assert_eq!(chunk, dec);
    }

    #[test]
    fn decrypt_errors_if_nonce_is_too_short() {
        let pass = Passwords::new("our little test secret");
        let e = CipherEngine::new(&pass);

        // *Almost* a valid chunk header, except it's one byte too short
        let bytes = {
            let mut result = [0; CHUNK_V1.len() + NONCE_SIZE - 1];
            for (i, x) in CHUNK_V1.iter().enumerate() {
                result[i] = *x;
            }
            result
        };

        let meta = [0; 0];

        assert!(matches!(
            e.decrypt_chunk(&bytes, &meta),
            Err(CipherError::NoNonce)
        ));
    }
}

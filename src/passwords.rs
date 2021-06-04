use pbkdf2::{
    password_hash::{PasswordHasher, SaltString},
    Pbkdf2,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::io::prelude::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const KEY_LEN: usize = 32; // Only size accepted by aead crate?

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Passwords {
    encryption: String,
}

impl Passwords {
    pub fn new(passphrase: &str) -> Self {
        let mut key = derive_password(passphrase);
        let _ = key.split_off(KEY_LEN);
        assert_eq!(key.len(), KEY_LEN);
        Self { encryption: key }
    }

    pub fn encryption_key(&self) -> &[u8] {
        self.encryption.as_bytes()
    }

    pub fn load(filename: &Path) -> Result<Self, PasswordError> {
        let data = std::fs::read(filename)
            .map_err(|err| PasswordError::Read(filename.to_path_buf(), err))?;
        serde_yaml::from_slice(&data)
            .map_err(|err| PasswordError::Parse(filename.to_path_buf(), err))
    }

    pub fn save(&self, filename: &Path) -> Result<(), PasswordError> {
        eprintln!("saving passwords to {:?}", filename);

        let data = serde_yaml::to_string(&self).map_err(PasswordError::Serialize)?;

        let mut file = std::fs::File::create(filename)
            .map_err(|err| PasswordError::Write(filename.to_path_buf(), err))?;
        let metadata = file
            .metadata()
            .map_err(|err| PasswordError::Write(filename.to_path_buf(), err))?;
        let mut permissions = metadata.permissions();

        // Make readadable by owner only. We still have the open file
        // handle, so we can write the content.
        permissions.set_mode(0o400);
        std::fs::set_permissions(filename, permissions)
            .map_err(|err| PasswordError::Write(filename.to_path_buf(), err))?;

        // Write actual content.
        file.write_all(data.as_bytes())
            .map_err(|err| PasswordError::Write(filename.to_path_buf(), err))?;

        Ok(())
    }
}

pub fn passwords_filename(config_filename: &Path) -> PathBuf {
    let mut filename = config_filename.to_path_buf();
    filename.set_file_name("passwords.yaml");
    filename
}

fn derive_password(passphrase: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);

    Pbkdf2
        .hash_password_simple(passphrase.as_bytes(), salt.as_ref())
        .unwrap()
        .to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("failed to serialize passwords for saving: {0}")]
    Serialize(serde_yaml::Error),

    #[error("failed to save passwords to {0}: {1}")]
    Write(PathBuf, std::io::Error),

    #[error("failed to read passwords from {0}: {1}")]
    Read(PathBuf, std::io::Error),

    #[error("failed to parse saved passwords from {0}: {1}")]
    Parse(PathBuf, serde_yaml::Error),
}

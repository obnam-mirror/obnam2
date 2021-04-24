use crate::passwords::{passwords_filename, PasswordError, Passwords};

use bytesize::MIB;
use log::{error, trace};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_CHUNK_SIZE: usize = MIB as usize;
const DEVNULL: &str = "/dev/null";

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct TentativeClientConfig {
    server_url: String,
    verify_tls_cert: Option<bool>,
    chunk_size: Option<usize>,
    roots: Vec<PathBuf>,
    log: Option<PathBuf>,
    encrypt: Option<bool>,
    exclude_cache_tag_directories: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub enum ClientConfig {
    Plain(ClientConfigWithoutPasswords),
    WithPasswords(ClientConfigWithoutPasswords, Passwords),
}

impl ClientConfig {
    pub fn read_without_passwords(filename: &Path) -> Result<Self, ClientConfigError> {
        let config = ClientConfigWithoutPasswords::read_config(filename)?;
        Ok(ClientConfig::Plain(config))
    }

    pub fn read_with_passwords(filename: &Path) -> Result<Self, ClientConfigError> {
        let config = ClientConfigWithoutPasswords::read_config(filename)?;
        if config.encrypt {
            let passwords = Passwords::load(&passwords_filename(filename))
                .map_err(ClientConfigError::PasswordsMissing)?;
            Ok(ClientConfig::WithPasswords(config, passwords))
        } else {
            Ok(ClientConfig::Plain(config))
        }
    }

    pub fn config(&self) -> &ClientConfigWithoutPasswords {
        match self {
            Self::Plain(config) => &config,
            Self::WithPasswords(config, _) => &config,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ClientConfigWithoutPasswords {
    pub filename: PathBuf,
    pub server_url: String,
    pub verify_tls_cert: bool,
    pub chunk_size: usize,
    pub roots: Vec<PathBuf>,
    pub log: PathBuf,
    pub encrypt: bool,
    pub exclude_cache_tag_directories: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientConfigError {
    #[error("server_url is empty")]
    ServerUrlIsEmpty,

    #[error("No backup roots in config; at least one is needed")]
    NoBackupRoot,

    #[error("server URL doesn't use https: {0}")]
    NotHttps(String),

    #[error("No passwords are set: you may need to run 'obnam init': {0}")]
    PasswordsMissing(PasswordError),

    #[error("failed to read configuration file {0}: {1}")]
    Read(PathBuf, std::io::Error),

    #[error("failed to parse configuration file {0} as YAML: {1}")]
    YamlParse(PathBuf, serde_yaml::Error),
}

pub type ClientConfigResult<T> = Result<T, ClientConfigError>;

impl ClientConfigWithoutPasswords {
    pub fn read_config(filename: &Path) -> ClientConfigResult<Self> {
        trace!("read_config: filename={:?}", filename);
        let config = std::fs::read_to_string(filename)
            .map_err(|err| ClientConfigError::Read(filename.to_path_buf(), err))?;
        let tentative: TentativeClientConfig = serde_yaml::from_str(&config)
            .map_err(|err| ClientConfigError::YamlParse(filename.to_path_buf(), err))?;
        let roots = tentative
            .roots
            .iter()
            .map(|path| expand_tilde(path))
            .collect();
        let log = tentative
            .log
            .map(|path| expand_tilde(&path))
            .unwrap_or_else(|| PathBuf::from(DEVNULL));
        let encrypt = tentative.encrypt.or(Some(false)).unwrap();
        let exclude_cache_tag_directories = tentative.exclude_cache_tag_directories.unwrap_or(true);

        let config = Self {
            chunk_size: tentative.chunk_size.or(Some(DEFAULT_CHUNK_SIZE)).unwrap(),
            encrypt,
            filename: filename.to_path_buf(),
            roots,
            server_url: tentative.server_url,
            verify_tls_cert: tentative.verify_tls_cert.or(Some(false)).unwrap(),
            log,
            exclude_cache_tag_directories,
        };

        config.check()?;
        Ok(config)
    }

    fn check(&self) -> Result<(), ClientConfigError> {
        if self.server_url.is_empty() {
            return Err(ClientConfigError::ServerUrlIsEmpty);
        }
        if !self.server_url.starts_with("https://") {
            return Err(ClientConfigError::NotHttps(self.server_url.to_string()));
        }
        if self.roots.is_empty() {
            return Err(ClientConfigError::NoBackupRoot);
        }
        Ok(())
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            let mut expanded = PathBuf::from(home);
            for comp in path.components().skip(1) {
                expanded.push(comp);
            }
            expanded
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    }
}

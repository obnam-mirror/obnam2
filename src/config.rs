//! Client configuration.

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
    exclude_cache_tag_directories: Option<bool>,
}

/// Configuration for the Obnam client.
#[derive(Debug, Serialize, Clone)]
pub struct ClientConfig {
    /// Name of configuration file.
    pub filename: PathBuf,
    /// URL of Obnam server.
    pub server_url: String,
    /// Should server's TLS certificate be verified using CA
    /// signatures? Set to false, for self-signed certificates.
    pub verify_tls_cert: bool,
    /// Size of chunks when splitting files for backup.
    pub chunk_size: usize,
    /// Backup root directories.
    pub roots: Vec<PathBuf>,
    /// File where logs should be written.
    pub log: PathBuf,
    /// Should cache directories be excluded? Cache directories
    /// contain a specially formatted CACHEDIR.TAG file.
    pub exclude_cache_tag_directories: bool,
}

impl ClientConfig {
    /// Read a client configuration from a file.
    pub fn read(filename: &Path) -> Result<Self, ClientConfigError> {
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
        let exclude_cache_tag_directories = tentative.exclude_cache_tag_directories.unwrap_or(true);

        let config = Self {
            chunk_size: tentative.chunk_size.or(Some(DEFAULT_CHUNK_SIZE)).unwrap(),
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

    /// Read encryption passwords from a file.
    ///
    /// The password file is expected to be next to the configuration file.
    pub fn passwords(&self) -> Result<Passwords, ClientConfigError> {
        Passwords::load(&passwords_filename(&self.filename))
            .map_err(ClientConfigError::PasswordsMissing)
    }
}

/// Possible errors from configuration files.
#[derive(Debug, thiserror::Error)]
pub enum ClientConfigError {
    /// The configuration specifies the server URL as an empty string.
    #[error("server_url is empty")]
    ServerUrlIsEmpty,

    /// The configuration does not specify any backup root directories.
    #[error("No backup roots in config; at least one is needed")]
    NoBackupRoot,

    /// The server URL is not an https: one.
    #[error("server URL doesn't use https: {0}")]
    NotHttps(String),

    /// There are no passwords stored.
    #[error("No passwords are set: you may need to run 'obnam init': {0}")]
    PasswordsMissing(PasswordError),

    /// Error reading a configuation file.
    #[error("failed to read configuration file {0}: {1}")]
    Read(PathBuf, std::io::Error),

    /// Error parsing configuration file as YAML.
    #[error("failed to parse configuration file {0} as YAML: {1}")]
    YamlParse(PathBuf, serde_yaml::Error),
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

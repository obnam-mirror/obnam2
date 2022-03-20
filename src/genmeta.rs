//! Backup generations metadata.

use crate::schema::{SchemaVersion, VersionComponent};
use serde::Serialize;
use std::collections::HashMap;

/// Metadata about the local generation.
#[derive(Debug, Serialize)]
pub struct GenerationMeta {
    schema_version: SchemaVersion,
    extras: HashMap<String, String>,
}

impl GenerationMeta {
    /// Create from a hash map.
    pub fn from(mut map: HashMap<String, String>) -> Result<Self, GenerationMetaError> {
        let major: VersionComponent = metaint(&mut map, "schema_version_major")?;
        let minor: VersionComponent = metaint(&mut map, "schema_version_minor")?;
        Ok(Self {
            schema_version: SchemaVersion::new(major, minor),
            extras: map,
        })
    }

    /// Return schema version of local generation.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }
}

fn metastr(map: &mut HashMap<String, String>, key: &str) -> Result<String, GenerationMetaError> {
    if let Some(v) = map.remove(key) {
        Ok(v)
    } else {
        Err(GenerationMetaError::NoMetaKey(key.to_string()))
    }
}

fn metaint(map: &mut HashMap<String, String>, key: &str) -> Result<u32, GenerationMetaError> {
    let v = metastr(map, key)?;
    let v = v
        .parse()
        .map_err(|err| GenerationMetaError::BadMetaInteger(key.to_string(), err))?;
    Ok(v)
}

/// Possible errors from getting generation metadata.
#[derive(Debug, thiserror::Error)]
pub enum GenerationMetaError {
    /// Missing from from 'meta' table.
    #[error("Generation 'meta' table does not have a row {0}")]
    NoMetaKey(String),

    /// Bad data in 'meta' table.
    #[error("Generation 'meta' row {0} has badly formed integer: {1}")]
    BadMetaInteger(String, std::num::ParseIntError),
}

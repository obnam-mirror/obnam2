//! The `backup` subcommand.

use crate::config::ClientConfig;
use crate::dbgen::{schema_version, DEFAULT_SCHEMA_MAJOR, SCHEMA_MAJORS};
use crate::error::ObnamError;

use clap::Parser;

/// List supported backup schema versions.
#[derive(Debug, Parser)]
pub struct ListSchemaVersions {
    /// List only the default version.
    #[clap(long)]
    default_only: bool,
}

impl ListSchemaVersions {
    /// Run the command.
    pub fn run(&self, _config: &ClientConfig) -> Result<(), ObnamError> {
        if self.default_only {
            let schema = schema_version(DEFAULT_SCHEMA_MAJOR)?;
            println!("{}", schema);
        } else {
            for major in SCHEMA_MAJORS {
                let schema = schema_version(*major)?;
                println!("{}", schema);
            }
        }
        Ok(())
    }
}

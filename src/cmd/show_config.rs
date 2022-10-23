//! The `show-config` subcommand.

use crate::config::ClientConfig;
use crate::error::ObnamError;
use clap::Parser;

/// Show actual client configuration.
#[derive(Debug, Parser)]
pub struct ShowConfig {}

impl ShowConfig {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        println!("{}", serde_json::to_string_pretty(config)?);
        Ok(())
    }
}

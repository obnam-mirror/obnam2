//! The `show-config` subcommand.

use crate::config::ClientConfig;
use crate::error::ObnamError;
use structopt::StructOpt;

/// Show actual client configuration.
#[derive(Debug, StructOpt)]
pub struct ShowConfig {}

impl ShowConfig {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        println!("{}", serde_json::to_string_pretty(config)?);
        Ok(())
    }
}

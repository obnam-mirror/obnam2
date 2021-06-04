use crate::config::ClientConfig;
use crate::error::ObnamError;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ShowConfig {}

impl ShowConfig {
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        println!("{}", serde_json::to_string_pretty(config)?);
        Ok(())
    }
}

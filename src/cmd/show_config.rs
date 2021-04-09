use crate::client::ClientConfig;
use crate::error::ObnamError;

pub fn show_config(config: &ClientConfig) -> Result<(), ObnamError> {
    println!("{}", serde_json::to_string_pretty(&config.config())?);
    Ok(())
}

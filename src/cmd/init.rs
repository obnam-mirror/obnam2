//! The `init` subcommand.

use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::passwords::{passwords_filename, Passwords};
use clap::Parser;

const PROMPT: &str = "Obnam passphrase: ";

/// Initialize client by setting passwords.
#[derive(Debug, Parser)]
pub struct Init {
    /// Only for testing.
    #[clap(long)]
    insecure_passphrase: Option<String>,
}

impl Init {
    /// Run the command.
    pub fn run(&self, config: &ClientConfig) -> Result<(), ObnamError> {
        let passphrase = match &self.insecure_passphrase {
            Some(x) => x.to_string(),
            None => rpassword::read_password_from_tty(Some(PROMPT)).unwrap(),
        };

        let passwords = Passwords::new(&passphrase);
        let filename = passwords_filename(&config.filename);
        passwords
            .save(&filename)
            .map_err(|err| ObnamError::PasswordSave(filename, err))?;
        Ok(())
    }
}

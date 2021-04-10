use crate::config::ClientConfigWithoutPasswords;
use crate::error::ObnamError;
use crate::passwords::{passwords_filename, Passwords};
use std::path::Path;
use structopt::StructOpt;

const PROMPT: &str = "Obnam passphrase: ";

#[derive(Debug, StructOpt)]
pub struct Init {
    #[structopt(long)]
    insecure_passphrase: Option<String>,
}

impl Init {
    pub fn run(
        &self,
        config: &ClientConfigWithoutPasswords,
        config_filename: &Path,
    ) -> Result<(), ObnamError> {
        if !config.encrypt {
            panic!("no encryption specified");
        }

        let passphrase = match &self.insecure_passphrase {
            Some(x) => x.to_string(),
            None => rpassword::read_password_from_tty(Some(PROMPT)).unwrap(),
        };

        let passwords = Passwords::new(&passphrase);
        let filename = passwords_filename(config_filename);
        passwords
            .save(&filename)
            .map_err(|err| ObnamError::PasswordSave(filename, err))?;
        Ok(())
    }
}

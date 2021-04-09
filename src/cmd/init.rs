use crate::client::ClientConfigWithoutPasswords;
use crate::error::ObnamError;
use crate::passwords::{passwords_filename, Passwords};
use std::path::Path;

const PROMPT: &str = "Obnam passphrase: ";

pub fn init(
    config: &ClientConfigWithoutPasswords,
    config_filename: &Path,
    insecure_passphrase: Option<String>,
) -> Result<(), ObnamError> {
    if !config.encrypt {
        panic!("no encryption specified");
    }

    let passphrase = match insecure_passphrase {
        Some(x) => x,
        None => rpassword::read_password_from_tty(Some(PROMPT)).unwrap(),
    };

    let passwords = Passwords::new(&passphrase);
    let filename = passwords_filename(config_filename);
    passwords
        .save(&filename)
        .map_err(|err| ObnamError::PasswordSave(filename, err))?;
    Ok(())
}

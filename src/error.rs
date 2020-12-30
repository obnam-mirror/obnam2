use thiserror::Error;

/// Define all the kinds of errors any part of this crate can return.
#[derive(Debug, Error)]
pub enum ObnamError {
    #[error("Can't find backup '{0}'")]
    UnknownGeneration(String),
}

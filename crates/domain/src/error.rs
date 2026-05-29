use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid state transition: {0} -> {1}")]
    InvalidTransition(&'static str, &'static str),
    #[error("encryption error: {0}")]
    Crypto(String),
}

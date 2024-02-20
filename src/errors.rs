//! This module comprises the definition of the errors that could possibly arise
use std::num::ParseIntError;

use thiserror::Error;

/// The possible errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("io error {0}")]
    Io(#[from] std::io::Error),
    #[error("not an int {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("regex error {0}")]
    Regex(#[from] regex::Error)
}

/// Easy result redefinition
pub type Result<T> = std::result::Result<T, Error>;
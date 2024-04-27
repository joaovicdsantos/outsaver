use core::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct MegaException {
    message: String,
}

impl MegaException {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for MegaException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for MegaException {
    fn description(&self) -> &str {
        &self.message
    }
}

use core::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct OutsaverException {
    message: String,
}

impl OutsaverException {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for OutsaverException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for OutsaverException {
    fn description(&self) -> &str {
        &self.message
    }
}

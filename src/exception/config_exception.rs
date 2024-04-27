use core::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct ConfigException {
    message: String,
}

impl ConfigException {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ConfigException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ConfigException {
    fn description(&self) -> &str {
        &self.message
    }
}

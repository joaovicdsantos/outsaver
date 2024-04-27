use std::env;

use crate::exception::config_exception::ConfigException;

pub struct Config {
    pub env: Env,
}

impl Config {
    pub fn load() -> Result<Self, ConfigException> {
        Ok(Config { env: Env::load()? })
    }
}

pub struct Env {
    pub discord: Discord,
    pub mega: Mega,
}

impl Env {
    fn load() -> Result<Self, ConfigException> {
        let discord = Discord::load()?;
        let mega = Mega::load()?;
        Ok(Env { discord, mega })
    }
}

pub struct Discord {
    pub token: String,
}

impl Discord {
    fn load() -> Result<Self, ConfigException> {
        match env::var("DISCORD_TOKEN") {
            Ok(token) => Ok(Discord { token }),
            Err(_) => Err(ConfigException::new(
                "DISCORD_TOKEN is not set in the environment",
            )),
        }
    }
}

pub struct Mega {
    pub email: String,
    pub password: String,
    pub destination_node: String,
}

impl Mega {
    fn load() -> Result<Self, ConfigException> {
        let email = match env::var("MEGA_EMAIL") {
            Ok(email) => email,
            Err(_) => {
                return Err(ConfigException::new(
                    "MEGA_EMAIL is not set in the environment",
                ))
            }
        };
        let password = match env::var("MEGA_PASSWORD") {
            Ok(password) => password,
            Err(_) => {
                return Err(ConfigException::new(
                    "MEGA_PASSWORD is not set in the environment",
                ))
            }
        };
        let destination_node = match env::var("MEGA_DESTINATION_NODE") {
            Ok(destination_node) => destination_node,
            Err(_) => {
                return Err(ConfigException::new(
                    "MEGA_DESTINATION_NODE is not set in the environment",
                ))
            }
        };

        Ok(Mega {
            email,
            password,
            destination_node,
        })
    }
}

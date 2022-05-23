use std::io::Error as IoError;

use thiserror::Error as ThisError;
use toml::{de::Error as TomlDeError, ser::Error as TomlSerError};

use crate::dirs;
use crate::utils;

pub use self::format::*;
pub use self::resolver::*;

mod format;
mod resolver;

#[derive(ThisError, Debug)]
pub enum ConfigError {
    #[error("cannot get config")]
    Io(#[from] IoError),
    #[error("cannot parse config")]
    TomlDe(#[from] TomlDeError),
    #[error("cannot stringify config")]
    TomlSer(#[from] TomlSerError),
}

impl Config {
    pub async fn from_file() -> Result<Config, ConfigError> {
        let config_file = dirs::config_file();
        let config = if config_file.is_file() {
            parse(utils::read_to_string(&config_file).await?.as_bytes())?
        } else {
            utils::create_dir_all(&dirs::config_dir()).await?;
            let default_config = Config::default();
            utils::write(&config_file, &stringify(&default_config)?).await?;
            default_config
        };
        Ok(config)
    }

    pub async fn update_file(&self) -> Result<(), ConfigError> {
        let config_file = dirs::config_file();
        if config_file.is_file() {
            utils::write(&config_file, &stringify(self)?).await?;
        };
        Ok(())
    }
}

fn parse(slice: &[u8]) -> Result<Config, TomlDeError> {
    toml::from_slice(slice)
}

fn stringify(config: &Config) -> Result<String, TomlSerError> {
    toml::to_string(config)
}

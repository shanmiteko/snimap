use std::io::Error as IoError;

use thiserror::Error as ThisError;
use toml::{de::Error as TomlDeError, ser::Error as TomlSerError};

use crate::dirs;
use crate::utils::{create_dir_all, read_to_string, write};

pub use self::format::*;

mod format;

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
            parse(read_to_string(&config_file)?.as_bytes())?
        } else {
            create_dir_all(&dirs::config_dir())?;
            let default_config = Config::default();
            write(&config_file, &stringify(&default_config)?)?;
            default_config
        };
        Ok(config)
    }
}

fn parse(slice: &[u8]) -> Result<Config, TomlDeError> {
    toml::from_slice(slice)
}

fn stringify(config: &Config) -> Result<String, TomlSerError> {
    toml::to_string(config)
}

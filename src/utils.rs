use std::{io::Error, path::PathBuf};

use tokio::fs;

pub async fn read_to_string(path: &PathBuf) -> Result<String, Error> {
    log::debug!("read_to_string {:?}", path);
    fs::read_to_string(path).await
}

pub async fn write(path: &PathBuf, contents: &str) -> Result<(), Error> {
    log::debug!("write {:?} {}", path, contents);
    fs::write(path, contents).await
}

pub async fn create_dir_all(path: &PathBuf) -> Result<(), Error> {
    log::debug!("create_dir_all {:?}", path);
    fs::create_dir_all(path).await
}

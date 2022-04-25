use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    dirs_next::config_dir()
        .map(|config_dir| config_dir.join("disable-sni-reverse-proxy"))
        .expect("config directory not found")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn hosts_path() -> Option<PathBuf> {
    let path = if cfg!(windows) {
        PathBuf::from(r"C:\Windows\System32\drivers\etc")
    } else {
        PathBuf::from("/etc/hosts")
    };
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

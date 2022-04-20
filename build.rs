use std::{path::Path, process::Command};

fn main() {
    if !has_paths(vec!["private/ca.pem", "private/cakey.pem"]) {
        Command::new("./gen_cacert.sh").output().unwrap();
    }
}

fn has_paths(paths: Vec<&str>) -> bool {
    paths.iter().all(|path| Path::new(path).exists())
}

use std::{fs, io::Error, path::PathBuf};

use crate::dirs::hosts_path;

pub fn read_to_string(path: &PathBuf) -> Result<String, Error> {
    log::debug!("read_to_string {:?}", path);
    fs::read_to_string(path)
}

pub fn write(path: &PathBuf, contents: &str) -> Result<(), Error> {
    log::debug!("write {:?} {}", path, contents);
    fs::write(path, contents)
}

pub fn create_dir_all(path: &PathBuf) -> Result<(), Error> {
    log::debug!("create_dir_all {:?}", path);
    fs::create_dir_all(path)
}

pub async fn edit_hosts(hostnames: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let hosts_path = hosts_path().ok_or("hosts file not found")?;

    let mut hosts_string = read_to_string(&hosts_path)?;

    hosts_string = gen_hosts(&hosts_string, hostnames);

    write(&hosts_path, &hosts_string)?;

    Ok(())
}

fn gen_hosts(old_hosts: &str, hostnames: &[&str]) -> String {
    let mut is_will_change = false;
    let flag = "# Auto Generate by disable_sni_reverse_proxy";

    let mut hosts_vec = old_hosts
        .lines()
        .filter(|line| {
            let is_flag_line = line.starts_with(&flag[..15]);
            if is_flag_line {
                is_will_change = !is_will_change;
                return false;
            }
            !is_will_change
        })
        .collect::<Vec<&str>>();

    if !hostnames.is_empty() {
        hosts_vec.push(flag);
    }

    let hostpair = hostnames
        .iter()
        .map(|hostname| format!("127.0.0.1\t{}", hostname))
        .collect::<Vec<String>>();

    hosts_vec.append(
        hostpair
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>()
            .as_mut(),
    );

    if !hostnames.is_empty() {
        hosts_vec.push(flag);
    }

    hosts_vec.join("\n")
}

#[cfg(test)]
#[test]
fn test_gen_hosts() {
    let old_hosts = "# ...
# ...
127.0.0.1\tlocalhost
";
    let hostnames = vec!["hostname1", "hostname2"];
    let new_hosts = "# ...
# ...
127.0.0.1\tlocalhost
# Auto Generate by disable_sni_reverse_proxy
127.0.0.1\thostname1
127.0.0.1\thostname2
# Auto Generate by disable_sni_reverse_proxy";
    assert_eq!(gen_hosts(old_hosts, &hostnames), new_hosts);
    assert_eq!(gen_hosts(new_hosts, &hostnames), new_hosts);
}

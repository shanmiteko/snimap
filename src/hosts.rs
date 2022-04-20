use tokio::fs;

pub async fn edit_hosts(hostnames: Vec<&str>) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(unix))]
    unimplemented!();

    let hosts_path = "/etc/hosts";

    let mut hosts_string = fs::read_to_string(hosts_path).await?;

    hosts_string = gen_hosts(&hosts_string, hostnames);

    fs::write(hosts_path, hosts_string).await?;

    Ok(())
}

fn gen_hosts(old_hosts: &str, hostnames: Vec<&str>) -> String {
    let mut is_will_change = false;
    let flag = "# disable_sni_reverse_proxy auto generate";

    let mut hosts_vec = old_hosts
        .lines()
        .filter(|line| {
            let is_flag_line = line.starts_with(flag);
            if is_flag_line {
                is_will_change = !is_will_change;
                return false;
            }
            !is_will_change
        })
        .collect::<Vec<&str>>();

    hosts_vec.push(flag);

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

    hosts_vec.push(flag);

    hosts_vec.join("\n")
}

#[cfg(test)]
#[test]
fn gen_hosts_is_ok() {
    let old_hosts = "# ...
# ...
127.0.0.1\tlocalhost
";
    let hostnames = vec!["hostname1", "hostname2"];
    let new_hosts = "# ...
# ...
127.0.0.1\tlocalhost
# disable_sni_reverse_proxy auto generate
127.0.0.1\thostname1
127.0.0.1\thostname2
# disable_sni_reverse_proxy auto generate";
    assert_eq!(gen_hosts(old_hosts, hostnames.clone()), new_hosts);
    assert_eq!(gen_hosts(new_hosts, hostnames.clone()), new_hosts);
}

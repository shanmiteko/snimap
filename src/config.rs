use std::{io, net::SocketAddr};

use futures::future;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

lazy_static! {
    static ref CLIENT: Client = Client::new();
    static ref RE_CAPTURE_IP: Regex = Regex::new(r"ipaddress.com/ipv4/((\d+\.){3}\d+)").unwrap();
}

#[derive(Serialize, Deserialize)]
pub struct Hosts {
    pub dns: Option<Vec<Dns>>,
}

#[derive(Serialize, Deserialize)]
pub struct Dns {
    pub hostname: String,
    address: Option<String>,
}

#[derive(Error, Debug)]
pub enum HostsError {
    #[error("serializing Hosts error")]
    TomlSer(#[from] toml::ser::Error),
    #[error("hosts file format error")]
    TomlDe(#[from] toml::de::Error),
    #[error("net error")]
    Reqwest(#[from] reqwest::Error),
    #[error("io error")]
    Io(#[from] io::Error),
}

impl Hosts {
    pub async fn lookup_all(&mut self) -> Result<(), HostsError> {
        if let Some(dns_vec) = self.dns.as_mut() {
            future::join_all(dns_vec.iter_mut().map(|dns| dns.lookup())).await;
        }

        Ok(())
    }

    pub async fn from_path(path: &str) -> Result<Self, HostsError> {
        parse(&fs::read(path).await?)
    }

    pub async fn write_to_path(&self, path: &str) -> Result<(), HostsError> {
        Ok(fs::write(path, &stringify(self)?).await?)
    }
}

impl Dns {
    pub fn addr(&self) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.address.as_ref()?.parse().expect("address error"),
            0,
        ))
    }

    pub async fn lookup(&mut self) -> Result<(), HostsError> {
        if self.address.is_none() {
            if let Ok(html) = ip_lookup_on_ipaddress_com(&self.hostname).await {
                if let Some(ip) = capture_ip_from_html_plain(&html) {
                    self.address = Some(ip.into())
                }
            }
        }

        Ok(())
    }
}

pub fn parse(data: &[u8]) -> Result<Hosts, HostsError> {
    Ok(toml::from_slice::<Hosts>(data)?)
}

pub fn stringify(hosts: &Hosts) -> Result<String, HostsError> {
    Ok(toml::to_string_pretty(hosts)?)
}

fn capture_ip_from_html_plain(html: &str) -> Option<&str> {
    Some(RE_CAPTURE_IP.captures(html)?.get(1)?.as_str())
}

async fn ip_lookup_on_ipaddress_com(host: &str) -> Result<String, HostsError> {
    Ok(CLIENT
        .post("https://www.ipaddress.com/ip-lookup")
        .form(&[("host", host)])
        .send()
        .await?
        .text()
        .await?)
}

#[cfg(test)]
#[test]
fn regex_from_html_get_ip_is_ok() {
    let html = r#"<a href="https://www.ipaddress.com/ipv4/220.181.38.251">220.181.38.251</a>"#;
    assert_eq!(capture_ip_from_html_plain(html), Some("220.181.38.251"));
    let html = r#"<a href="https://www.ipaddress.com/ipv4/">"#;
    assert_eq!(capture_ip_from_html_plain(html), None);
}

#[cfg(test)]
#[tokio::test]
async fn ip_lookup_on_ipaddress_com_is_ok() {
    let html = ip_lookup_on_ipaddress_com("duckduckgo.com").await.unwrap();
    assert!(!html.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{parse, stringify, Dns, Hosts};

    #[test]
    fn hosts_toml_parse() {
        let hosts_toml = br#"[[dns]]
hostname = "duck.com"
"#;
        let hosts = parse(hosts_toml).unwrap();
        assert_eq!(hosts.dns.unwrap().get(0).unwrap().hostname, "duck.com")
    }

    #[test]
    fn hosts_stringify_to_toml() {
        let hosts = Hosts {
            dns: Some(vec![
                Dns {
                    hostname: "duckduckgo.com".into(),
                    address: None,
                },
                Dns {
                    hostname: "duck.com".into(),
                    address: None,
                },
            ]),
        };
        assert_eq!(
            stringify(&hosts).unwrap(),
            "[[dns]]
hostname = 'duckduckgo.com'

[[dns]]
hostname = 'duck.com'
"
            .to_string()
        );
    }

    #[tokio::test]
    async fn struct_dns_can_lookup_host() {
        let mut dns = Dns {
            hostname: "duckduckgo.com".into(),
            address: None,
        };
        dns.lookup().await.unwrap();
        assert!(dns.address.is_some());
    }

    #[tokio::test]
    async fn struct_hosts_can_lookup_all_host() {
        let mut hosts = Hosts {
            dns: Some(vec![
                Dns {
                    hostname: "duckduckgo.com".into(),
                    address: None,
                },
                Dns {
                    hostname: "duck.com".into(),
                    address: None,
                },
            ]),
        };
        hosts.lookup_all().await.unwrap();
        hosts
            .dns
            .unwrap()
            .iter()
            .for_each(|host| assert!(host.address.is_some()))
    }
}

use futures::future::join_all;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{Client, Error};

use super::format::{Config, Dns, Group};

lazy_static! {
    static ref CLIENT: Client = Client::new();
    static ref RE_CAPTURE_IP: Regex = Regex::new(r"ipaddress.com/ipv4/((\d+\.){3}\d+)").unwrap();
}

impl Dns {
    pub async fn lookup(&mut self) -> Result<(), Error> {
        let hostname = self.hostname();
        if self.enable() {
            if self.address().is_none() {
                tracing::info!(target: "lookup", "lookup {} ...", hostname);
                match capture_ip_from_html_plain(&ip_lookup_on_ipaddress_com(&hostname).await?) {
                    Some(address) => {
                        tracing::info!("{} -> {}", hostname, &address);
                        self.set_address(address)
                    }
                    None => {
                        tracing::warn!(target: "lookup","{} not found", hostname);
                    }
                }
            } else {
                tracing::info!(target: "lookup","{} had address", hostname);
            }
        } else {
            tracing::info!(target: "lookup","disable {}", hostname);
        }
        Ok(())
    }
}

impl Group {
    pub async fn lookup(&mut self) -> Result<(), Error> {
        if self.enable() {
            join_all(self.dns_mut().iter_mut().map(|dns| dns.lookup()))
                .await
                .into_iter()
                .try_for_each(|r| r)?
        }
        Ok(())
    }
}

impl Config {
    pub async fn lookup(&mut self) -> Result<(), Error> {
        if self.enable() {
            join_all(self.group_mut().iter_mut().map(|group| group.lookup()))
                .await
                .into_iter()
                .try_for_each(|r| r)?
        }
        Ok(())
    }
}

fn capture_ip_from_html_plain(html: &str) -> Option<String> {
    Some(RE_CAPTURE_IP.captures(html)?.get(1)?.as_str().to_string())
}

async fn ip_lookup_on_ipaddress_com(host: &str) -> Result<String, Error> {
    CLIENT
        .post("https://www.ipaddress.com/ip-lookup")
        .form(&[("host", host)])
        .send()
        .await?
        .text()
        .await
}

#[cfg(test)]
#[test]
fn regex_from_html_get_ip_is_ok() {
    let html =
        r#"<a href="https://www.ipaddress.com/ipv4/220.181.38.251">220.181.38.251</a>"#.to_string();
    assert_eq!(
        capture_ip_from_html_plain(&html),
        Some("220.181.38.251".to_string())
    );
    let html = r#"<a href="https://www.ipaddress.com/ipv4/">"#.to_string();
    assert_eq!(capture_ip_from_html_plain(&html), None);
}

#[cfg(test)]
#[tokio::test]
async fn ip_lookup_on_ipaddress_com_is_ok() {
    let html = ip_lookup_on_ipaddress_com("duckduckgo.com").await.unwrap();
    assert!(!html.is_empty())
}

#[cfg(test)]
mod tests {
    use crate::config::format::{Dns, Group};

    fn ip_ok(ip: &str) {
        assert!(ip.contains('.'));
        assert!(ip
            .split('.')
            .try_for_each(|ip| ip.parse::<u8>().map(|_| ()))
            .is_ok());
    }

    #[tokio::test]
    async fn struct_dns_can_lookup_host() {
        let mut dns = Dns::new("duckduckgo.com");
        dns.lookup().await.unwrap();
        if let Some(addr) = dns.address() {
            ip_ok(addr);
        }
    }

    #[tokio::test]
    async fn struct_group_can_lookup_host() {
        let mut group = Group::new(
            "name",
            vec![Dns::new("duckduckgo.com"), Dns::new("duckduckgo.com")],
        );
        group.lookup().await.unwrap();
        group
            .dns()
            .iter()
            .for_each(|d| ip_ok(d.address().as_ref().unwrap()))
    }
}

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
        if self.address().is_none() {
            if let Some(hostname) = self.hostname() {
                log::info!("lookup {}", hostname);
                if let Some(address) =
                    capture_ip_from_html_plain(ip_lookup_on_ipaddress_com(hostname).await?)
                {
                    self.address_mut(address);
                }
            }
        }
        Ok(())
    }
}

impl Group {
    pub async fn lookup(&mut self) -> Result<(), Error> {
        if let Some(dns) = self.dns() {
            join_all(dns.iter_mut().map(|d| d.lookup()))
                .await
                .into_iter()
                .try_for_each(|r| r)?
        }
        Ok(())
    }
}

impl Config {
    pub async fn lookup(&mut self) -> Result<(), Error> {
        if let Some(group) = self.group() {
            join_all(group.iter_mut().map(|g| g.lookup()))
                .await
                .into_iter()
                .try_for_each(|r| r)?
        }
        Ok(())
    }
}

fn capture_ip_from_html_plain(html: String) -> Option<String> {
    Some(RE_CAPTURE_IP.captures(&html)?.get(1)?.as_str().to_string())
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
        capture_ip_from_html_plain(html),
        Some("220.181.38.251".to_string())
    );
    let html = r#"<a href="https://www.ipaddress.com/ipv4/">"#.to_string();
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
        if let Some(dns) = group.dns() {
            dns.iter()
                .for_each(|d| ip_ok(d.address().as_ref().unwrap()))
        }
    }
}

use std::collections::HashMap;

use async_trait::async_trait;
use futures::future::join_all;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, ClientBuilder, Error};

use super::format::{Config, Dns, Group};

static LOOKUP_CLIENT: Lazy<Client> = Lazy::new(|| {
    ClientBuilder::new()
        .default_headers(
            (&HashMap::from([
                ("Referer", "https://www.ipaddress.com/ip-lookup"),
                ("Accept-Encoding", "br"),
            ])
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect::<HashMap<String, String>>())
                .try_into()
                .unwrap(),
        )
        .build()
        .unwrap()
});

static RE_CAPTURE_IP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"ipaddress.com/ipv4/((\d+\.){3}\d+)").unwrap());

#[async_trait]
pub trait DnsResolve {
    async fn resolve(&mut self) -> Result<(), Error>;
}

#[async_trait]
impl DnsResolve for Dns {
    async fn resolve(&mut self) -> Result<(), Error> {
        if let Some(hostname) = self.hostname_ref() {
            if self.address_ref().is_none() {
                log::info!(target: "lookup", "lookup {} ...", hostname);
                match capture_ip_from_html_plain(&ip_lookup_on_ipaddress_com(hostname).await?) {
                    Some(address) => {
                        log::info!(target: "lookup", "{} -> {}", hostname, &address);
                        self.set_address(address)
                    }
                    None => {
                        log::warn!(target: "lookup", "{} not found", hostname);
                    }
                }
            } else {
                log::info!(target: "lookup", "{} had address", hostname);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl DnsResolve for Group {
    async fn resolve(&mut self) -> Result<(), Error> {
        if let Some(dns) = self.dns_mut() {
            join_all(dns.iter_mut().map(|dns| dns.resolve()))
                .await
                .into_iter()
                .try_for_each(|r| r)?
        }
        Ok(())
    }
}

#[async_trait]
impl DnsResolve for Config {
    async fn resolve(&mut self) -> Result<(), Error> {
        if let Some(dns) = self.group_mut() {
            join_all(dns.iter_mut().map(|dns| dns.resolve()))
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
    LOOKUP_CLIENT
        .post("https://www.ipaddress.com/ip-lookup")
        .form(&[("host", host)])
        .send()
        .await?
        .text()
        .await
}

#[cfg(test)]
#[test]
fn regex_from_html_get_ip() {
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
#[actix_web::test]
async fn test_ip_lookup_on_ipaddress_com() {
    let html = ip_lookup_on_ipaddress_com("duckduckgo.com").await.unwrap();
    assert!(!html.is_empty())
}

#[cfg(test)]
mod tests {
    use super::DnsResolve;
    use crate::config::format::{Dns, Group};

    fn ip_ok(ip: &str) {
        assert!(ip.contains('.'));
        ip.split('.')
            .try_for_each(|ip: &str| {
                if let Err(e) = ip.parse::<u8>() {
                    eprintln!("{} {}", ip, e);
                    return Err(());
                }
                Ok(())
            })
            .expect("cannot paser to u8")
    }

    #[actix_web::test]
    async fn struct_dns_can_resolve() {
        let mut dns = Dns::new("duckduckgo.com");
        dns.resolve().await.unwrap();
        ip_ok(dns.address_ref().unwrap());
    }

    #[actix_web::test]
    async fn struct_group_can_resolve() {
        let mut group = Group::new(
            "name",
            None,
            None,
            vec![Dns::new("duckduckgo.com"), Dns::new("duckduckgo.com")],
        );
        group.resolve().await.unwrap();
        group
            .dns_mut()
            .unwrap()
            .iter()
            .for_each(|d| ip_ok(d.address_ref().as_ref().unwrap()))
    }
}

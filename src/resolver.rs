use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use actix_tls::connect::Resolve;
use dns_lookup::lookup_host;
use futures::future::LocalBoxFuture;
use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use reqwest::{Client, ClientBuilder, Error as ReqwestError};

use crate::error::AnyError;

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

async fn ip_lookup_on_ipaddress_com(host: &str) -> Result<String, ReqwestError> {
    match LOOKUP_CLIENT
        .post("https://www.ipaddress.com/ip-lookup")
        .form(&[("host", host)])
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(text) => Ok(text),
            Err(err) => {
                log::error!(target: "lookup", "{host} -> failed to get text: {err}");
                Err(err)
            }
        },
        Err(err) => {
            log::error!(target: "lookup", "{host} -> failed to post: {err}");
            Err(err)
        }
    }
}

fn capture_ip_from_html_plain(html: &str) -> Option<IpAddr> {
    match RE_CAPTURE_IP
        .captures(html)?
        .get(1)?
        .as_str()
        .parse::<IpAddr>()
    {
        Ok(socket_addr) => Some(socket_addr),
        Err(err) => {
            log::error!(target: "lookup", "capture_ip_from_html_plain error: {err} {html}");
            None
        }
    }
}

pub struct DnsCache {
    white_list: Arc<HashSet<String>>,
    cache: Arc<Mutex<LruCache<String, Option<SocketAddr>>>>,
}

impl Default for DnsCache {
    fn default() -> Self {
        Self {
            white_list: Arc::new(HashSet::new()),
            cache: Arc::new(Mutex::new(LruCache::new(1024))),
        }
    }
}

impl Clone for DnsCache {
    fn clone(&self) -> Self {
        Self {
            white_list: self.white_list.clone(),
            cache: self.cache.clone(),
        }
    }
}

impl DnsCache {
    pub fn new(white_list: &[&str]) -> Self {
        Self {
            white_list: Arc::new(HashSet::from_iter(
                white_list.iter().map(|host| host.to_string()),
            )),
            ..Default::default()
        }
    }

    fn white_list_has(&self, host: &str) -> bool {
        self.white_list.contains(host)
    }

    fn get(&self, host: &str) -> Option<SocketAddr> {
        self.cache.lock().get(host).cloned().flatten()
    }

    fn insert(&self, host: &str, addr: SocketAddr) {
        self.cache.lock().put(host.to_string(), Some(addr));
    }
}

impl Resolve for DnsCache {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> LocalBoxFuture<'a, Result<Vec<SocketAddr>, AnyError>> {
        Box::pin(async move {
            match self.get(host) {
                Some(socket_addr) => Ok(vec![socket_addr]),
                None => match self.white_list_has(host) {
                    true => ip_lookup_on_ipaddress_com(host)
                        .await
                        .map(|html| {
                            capture_ip_from_html_plain(&html)
                                .map(|ip_addr| {
                                    self.insert(host, SocketAddr::new(ip_addr, port));
                                    vec![SocketAddr::new(ip_addr, port)]
                                })
                                .unwrap_or_else(Vec::new)
                        })
                        .map_err(|e| e.into()),
                    false => lookup_host(host)
                        .map(|ip_addrs| {
                            ip_addrs
                                .into_iter()
                                .map(|ip_addr| SocketAddr::new(ip_addr, port))
                                .collect::<Vec<SocketAddr>>()
                                .first()
                                .map(|socket_addr| {
                                    self.insert(host, *socket_addr);
                                    vec![*socket_addr]
                                })
                                .unwrap_or_else(|| {
                                    log::error!(target: "lookup", "{host} -> failed to lookup");
                                    vec![]
                                })
                        })
                        .map_err(|e| e.into()),
                },
            }
        })
    }
}

#[cfg(test)]
#[actix_web::test]
async fn test_ip_lookup_on_ipaddress_com() {
    let html = ip_lookup_on_ipaddress_com("duckduckgo.com").await.unwrap();
    dbg!(&html);
    assert!(!html.is_empty())
}

#[cfg(test)]
#[test]
fn regex_from_html_get_ip() {
    let html =
        r#"<a href="https://www.ipaddress.com/ipv4/220.181.38.251">220.181.38.251</a>"#.to_string();
    assert_eq!(
        capture_ip_from_html_plain(&html),
        Some("220.181.38.251".parse().unwrap())
    );
    let html = r#"<a href="https://www.ipaddress.com/ipv4/">"#.to_string();
    assert_eq!(capture_ip_from_html_plain(&html), None);
}

#[cfg(test)]
#[actix_web::test]
async fn test_dns_cache() {
    let cache = DnsCache::default();
    let addr = cache.lookup("duckduckgo.com", 443).await.unwrap();
    assert_eq!(addr.len(), 1);

    let cache = DnsCache::new(&["duckduckgo.com"]);
    let addr = cache.lookup("duckduckgo.com", 443).await.unwrap();
    assert_eq!(addr.len(), 1);
}

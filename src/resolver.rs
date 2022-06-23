use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use actix_tls::connect::Resolve;
use dns_lookup::lookup_host;
use futures::{future::LocalBoxFuture, lock::Mutex};
use lru::LruCache;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, ClientBuilder};

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

async fn ip_lookup_on_ipaddress_com<S: AsRef<str>>(host: S) -> Result<String, AnyError> {
    LOOKUP_CLIENT
        .post("https://www.ipaddress.com/ip-lookup")
        .form(&[("host", host.as_ref())])
        .send()
        .await?
        .text()
        .await
        .map_err(Into::into)
}

fn capture_ip_from_html_plain<S: AsRef<str>>(html: S) -> Result<IpAddr, AnyError> {
    RE_CAPTURE_IP
        .captures(html.as_ref())
        .ok_or("err in capture_ip_from_html_plain: no match is found")?
        .get(1)
        .ok_or("err in capture_ip_from_html_plain: this group didn't participate in the match")?
        .as_str()
        .parse::<IpAddr>()
        .map_err(Into::into)
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_whitelist(mut self, white_list: &[&str]) -> Self {
        self.white_list = Arc::new(HashSet::from_iter(
            white_list.iter().map(|host| host.to_string()),
        ));
        self
    }
}

impl Resolve for DnsCache {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> LocalBoxFuture<'a, Result<Vec<SocketAddr>, AnyError>> {
        Box::pin(async move {
            let mut cache = self.cache.lock().await;
            match cache.get(host).cloned().flatten() {
                Some(socket_addr) => Ok(vec![socket_addr]),
                None => match self.white_list.contains(host) {
                    true => ip_lookup_on_ipaddress_com(host)
                        .await
                        .and_then(capture_ip_from_html_plain)
                        .map(|ip_addr| {
                            log::info!(target: "lookup", "{host} -> {}", &ip_addr);
                            let socket_addr = SocketAddr::new(ip_addr, port);
                            cache.put(host.to_string(), Some(socket_addr));
                            vec![socket_addr]
                        })
                        .map_err(|e| {
                            log::error!(target: "lookup", "{host} -> failed to lookup: {e}");
                            e
                        })
                        .or(Ok(Vec::new())),
                    false => lookup_host(host)
                        .map_err(|e| e.to_string())
                        .and_then(|ip_addrs| {
                            ip_addrs
                                .into_iter()
                                .map(|ip_addr| SocketAddr::new(ip_addr, port))
                                .collect::<Vec<SocketAddr>>()
                                .first()
                                .cloned()
                                .ok_or_else(|| {
                                    "no socket_addr found in return value of lookup_host function"
                                        .to_string()
                                })
                        })
                        .map(|socket_addr| {
                            log::info!(target: "lookup", "{host} -> {}", &socket_addr);
                            cache.put(host.to_string(), Some(socket_addr));
                            vec![socket_addr]
                        })
                        .map_err(|e| {
                            log::error!(target: "lookup", "{host} -> failed to lookup: {e}");
                            e
                        })
                        .or(Ok(Vec::new())),
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
        capture_ip_from_html_plain(html).unwrap(),
        "220.181.38.251".parse::<IpAddr>().unwrap()
    );
    let html = r#"<a href="https://www.ipaddress.com/ipv4/">"#.to_string();
    assert!(capture_ip_from_html_plain(html).is_err());
}

#[cfg(test)]
#[actix_web::test]
async fn test_dns_cache_lookup() {
    let cache = DnsCache::default();
    let addr = cache.lookup("duckduckgo.com", 443).await.unwrap();
    assert_eq!(addr.len(), 1);

    let cache = DnsCache::new().with_whitelist(&["duckduckgo.com"]);
    let addr = cache.lookup("duckduckgo.com", 443).await.unwrap();
    assert_eq!(addr.len(), 1);
}

#[cfg(test)]
#[actix_web::test]
async fn test_dns_cache_clone() {
    use actix_web::rt::spawn;
    use futures::future::join_all;

    let cache = DnsCache::new().with_whitelist(&["duckduckgo.com", "google.com"]);
    let cache2 = cache.clone();
    let (s, r) = std::sync::mpsc::channel::<Option<SocketAddr>>();
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let cache_clone = cache2.clone();
        let s_clone = s.clone();
        jobs.push(spawn(async move {
            let mut cache_lock = dbg!(cache_clone.cache.lock().await);
            match cache_lock.get("duckduckgo.com").cloned().flatten() {
                Some(_) => {}
                None => {
                    dbg!(cache_lock
                        .put("duckduckgo.com".to_string(), "1.1.1.1:443".parse().ok())
                        .flatten());
                    s_clone.send("1.1.1.1:443".parse().ok()).unwrap();
                }
            }
        }))
    }
    join_all(jobs.into_iter()).await;
    drop(s);
    assert_eq!(r.recv().unwrap(), "1.1.1.1:443".parse().ok());
    assert!(r.recv().is_err());
}

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use actix_tls::connect::Resolve;
use dns_lookup::lookup_host;
use futures::future::LocalBoxFuture;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;

use crate::{anyway::AnyResult, config::SniMap};

static RE_CAPTURE_IP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"ipaddress.com/ipv4/((\d+\.){3}\d+)").unwrap());

fn ip_lookup_on_ipaddress_com<S: AsRef<str>>(host: S) -> AnyResult<String> {
    attohttpc::post("https://www.ipaddress.com/ip-lookup")
        .header("Referer", "https://www.ipaddress.com/ip-lookup")
        .header("Accept-Encoding", "br")
        .form(&[("host", host.as_ref())])?
        .send()?
        .text()
        .map_err(Into::into)
}

fn capture_ip_from_html_plain<S: AsRef<str>>(html: S) -> AnyResult<IpAddr> {
    RE_CAPTURE_IP
        .captures(html.as_ref())
        .ok_or("err in capture_ip_from_html_plain: no match is found")?
        .get(1)
        .ok_or("err in capture_ip_from_html_plain: this group didn't participate in the match")?
        .as_str()
        .parse::<IpAddr>()
        .map_err(Into::into)
}

enum ResolveResult<LateInitAddr = OnceCell<SocketAddr>> {
    CGetAddrInfo(LateInitAddr),
    WwwIpaddressCom(LateInitAddr),
}

impl ResolveResult {
    pub fn get_or_init(&self, host: &str) -> Option<SocketAddr> {
        match self {
            ResolveResult::CGetAddrInfo(socket_addr) => socket_addr.get_or_try_init(|| {
                lookup_host(host)
                    .map_err(|e| e.to_string())
                    .and_then(|ip_addrs| {
                        ip_addrs
                            .into_iter()
                            .next()
                            .map(|ip_addr| SocketAddr::new(ip_addr, 443))
                            .ok_or_else(|| {
                                "no socket_addr found in return value of `lookup_host` function"
                                    .to_string()
                            })
                    })
            }),
            ResolveResult::WwwIpaddressCom(socket_addr) => socket_addr.get_or_try_init(|| {
                ip_lookup_on_ipaddress_com(host)
                    .and_then(capture_ip_from_html_plain)
                    .map(|ip_addr| SocketAddr::new(ip_addr, 443))
                    .map_err(|e| e.to_string())
            }),
        }
        .inspect(|socket_addr| log::info!(target: "lookup", "{host} -> {socket_addr}"))
        .inspect_err(|e| log::error!(target: "lookup", "{host} -> failed to lookup: {e}"))
        .ok()
        .cloned()
    }
}

pub struct SniMapResolver {
    cache: Arc<HashMap<String, ResolveResult>>,
}

impl SniMapResolver {
    pub fn from_snimap(snimap: &SniMap) -> Self {
        Self {
            cache: Arc::new(
                snimap
                    .hostnames()
                    .iter()
                    .map(|s| {
                        (
                            s.to_string(),
                            ResolveResult::WwwIpaddressCom(OnceCell::new()),
                        )
                    })
                    .chain(
                        snimap
                            .overrided_sni()
                            .iter()
                            .map(|s| (s.to_string(), ResolveResult::CGetAddrInfo(OnceCell::new()))),
                    )
                    .collect(),
            ),
        }
    }

    pub fn get(&self, host: &str) -> Option<SocketAddr> {
        match self.cache.get(host) {
            Some(resolve_result) => resolve_result.get_or_init(host),
            _ => unreachable!("`SniMapResolver` should only resolve host in `SniMap`"),
        }
    }
}

impl Clone for SniMapResolver {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache.clone(),
        }
    }
}

impl Resolve for SniMapResolver {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
        _port: u16,
    ) -> LocalBoxFuture<'a, Result<Vec<SocketAddr>, Box<dyn std::error::Error>>> {
        Box::pin(async move {
            Ok(match self.get(host) {
                Some(socket_addr) => vec![socket_addr],
                None => vec![],
            })
        })
    }
}

#[cfg(test)]
#[test]
fn test_ip_lookup_on_ipaddress_com() {
    let html = ip_lookup_on_ipaddress_com("duckduckgo.com").unwrap();
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
async fn test_snimap_resolver() {
    use crate::config::Mapping;

    let snimap = Mapping::new("duckduckgo.com")
        .override_sni("example.com")
        .into();

    let snimap_resolver = SniMapResolver::from_snimap(&snimap);

    assert_ne!(snimap_resolver.get("example.com"), None);
    assert!(snimap_resolver.lookup("example.com", 443).await.is_ok());
    assert_ne!(snimap_resolver.get("duckduckgo.com"), None);
    assert!(snimap_resolver.lookup("duckduckgo.com", 443).await.is_ok());
}

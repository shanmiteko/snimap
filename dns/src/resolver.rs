use std::{net::IpAddr, sync::Arc};

use dashmap::DashMap;
use log::debug;

use crate::spiders::{MysslCom, NexcessNet};

#[async_trait::async_trait]
pub trait Lookup {
    async fn lookup(&self, hostname: &str) -> Option<IpAddr>;
}

pub struct Resolver {
    cache: Arc<DashMap<String, IpAddr>>,
    dnspiders: Arc<Vec<Box<dyn Lookup + Send + Sync>>>,
}

impl Resolver {
    pub async fn lookup_ip(&self, hostname: &str) -> Option<IpAddr> {
        for dnspider in self.dnspiders.iter() {
            if let Some(ip) = self.lookup_ip_from_cache(hostname) {
                debug!("{} -> cached", hostname);
                return Some(ip);
            };
            if let Some(ip) = dnspider.lookup(hostname).await {
                if let Some(ip) = self.lookup_ip_from_cache(hostname) {
                    debug!("{} -> cached", hostname);
                    return Some(ip);
                };
                self.cache.insert(hostname.into(), ip);
                debug!("{} -> {}", hostname, ip);
                return Some(ip);
            };
        }
        None
    }

    #[inline]
    fn lookup_ip_from_cache(&self, hostname: &str) -> Option<IpAddr> {
        self.cache.get(hostname).map(|v| *v)
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            dnspiders: Arc::new(vec![
                Box::<MysslCom>::default(),
                Box::<NexcessNet>::default(),
            ]),
        }
    }
}

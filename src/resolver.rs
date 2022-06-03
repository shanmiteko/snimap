use std::sync::Arc;

use actix_tls::connect::Resolve;

use crate::config::DomainMap;

pub struct LocalHosts(Arc<DomainMap>);

impl LocalHosts {
    pub fn new(domain_map: Arc<DomainMap>) -> Self {
        Self(domain_map)
    }
}

impl Resolve for LocalHosts {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
        _port: u16,
    ) -> futures::future::LocalBoxFuture<
        'a,
        Result<Vec<std::net::SocketAddr>, Box<dyn std::error::Error>>,
    > {
        Box::pin(async move {
            if let Some(socket_addr) = self.0.get(host) {
                Ok(vec![*socket_addr])
            } else {
                log::error!("No dns records for {}", host);
                Ok(vec![])
            }
        })
    }
}

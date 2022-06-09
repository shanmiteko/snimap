use std::{
    cell::RefCell,
    net::{SocketAddr, ToSocketAddrs},
};

use actix_tls::connect::{Resolve, Resolver as ActixResolver};
use futures::FutureExt;
use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};

struct DnsResolver(TokioAsyncResolver);

impl DnsResolver {
    fn doh() -> Self {
        let (conf, mut opts) = (
            ResolverConfig::from_parts(
                None,
                vec![],
                [
                    ("1.0.0.1:443", "one.one.one.one"),
                    ("1.1.1.1:443", "1dot1dot1dot1.cloudflare-dns.com"),
                    ("146.112.41.2:443", "doh.opendns.com"),
                ]
                .into_iter()
                .map(|(socket_addr, sni)| NameServerConfig {
                    socket_addr: socket_addr.to_socket_addrs().unwrap().next().unwrap(),
                    protocol: Protocol::Https,
                    tls_dns_name: Some(sni.to_string()),
                    trust_nx_responses: true,
                    tls_config: None,
                    bind_addr: None,
                })
                .collect::<Vec<NameServerConfig>>(),
            ),
            ResolverOpts::default(),
        );

        opts.use_hosts_file = false;
        opts.validate = true;

        Self(TokioAsyncResolver::tokio(conf, opts).unwrap())
    }
}

impl Resolve for DnsResolver {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> futures::future::LocalBoxFuture<
        'a,
        Result<Vec<std::net::SocketAddr>, Box<dyn std::error::Error>>,
    > {
        async move {
            Ok(self
                .0
                .lookup_ip(host)
                .await?
                .iter()
                .map(|ip| SocketAddr::new(ip, port))
                .inspect(|x| log::info!(target: "resolver", "lookup {}:{} => {:?}", host, port, x))
                .collect())
        }
        .boxed_local()
    }
}

pub fn resolver() -> ActixResolver {
    thread_local! {
        static TRUST_DNS_RESOLVER: RefCell<Option<ActixResolver>> = RefCell::new(None);
    }

    TRUST_DNS_RESOLVER.with(|local| {
        let resolver = local.borrow().as_ref().map(Clone::clone);

        match resolver {
            Some(resolver) => resolver,

            None => {
                let resolver = ActixResolver::custom(DnsResolver::doh());
                *local.borrow_mut() = Some(resolver.clone());
                resolver
            }
        }
    })
}

#[cfg(test)]
#[actix_web::test]
async fn test_trust_dot_resolver() {
    assert!(
        DnsResolver::doh()
            .lookup("duck.com", 443)
            .await
            .unwrap()
            .len()
            > 0
    );
}

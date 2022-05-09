#![feature(async_closure)]
use std::{
    cell::Cell,
    env,
    net::{IpAddr, SocketAddr},
};

use async_ctrlc::CtrlC;
use config::Config;
use once_cell::sync::OnceCell;
use reqwest::Client;
use rustls::ClientConfig as TlsConfig;
use warp::{Filter, Rejection, Reply};
use warp_reverse_proxy::{
    extract_request_data_filter, proxy_to_and_forward_response_use_client, with_client,
};

mod cert;
mod config;
mod dirs;
mod hosts;
mod utils;

static CLIENT_ENABLE_SNI: OnceCell<Client> = OnceCell::new();
static CLIENT_DISABLE_SNI: OnceCell<Client> = OnceCell::new();
static mut HOSTS_ENABLE_SNI: Vec<String> = Vec::new();
static mut HOSTS_DISABLE_SNI: Vec<String> = Vec::new();

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let tls_cnf_enable_sni = tls_config();

    let mut tls_cnf_disable_sni = tls_cnf_enable_sni.clone();
    tls_cnf_disable_sni.enable_sni = false;

    let client_builder = Cell::new(Client::builder().use_preconfigured_tls(tls_cnf_enable_sni));
    let client_builder_disable_sni =
        Cell::new(Client::builder().use_preconfigured_tls(tls_cnf_disable_sni));

    let mut config = Config::from_file().await?;

    config.lookup().await?;

    config.update_file().await?;

    if config.enable() {
        let config_enable_sni = config.enable_sni();
        config.group().into_iter().for_each(|group| {
            if group.enable() {
                let group_enable_sni = group.enable_sni();
                group.dns().into_iter().for_each(|dns| {
                    if dns.enable() {
                        let dns_enable_sni = dns.enable_sni();
                        if let Some(Ok(addr)) =
                            dns.address().map(|address| address.parse::<IpAddr>())
                        {
                            if config_enable_sni || group_enable_sni || dns_enable_sni {
                                unsafe {
                                    HOSTS_ENABLE_SNI.push(dns.hostname());
                                }
                                client_builder.set(
                                    client_builder
                                        .take()
                                        .resolve(&dns.hostname(), SocketAddr::new(addr, 443)),
                                );
                            } else {
                                unsafe {
                                    HOSTS_DISABLE_SNI.push(dns.hostname());
                                }
                                client_builder_disable_sni.set(
                                    client_builder_disable_sni
                                        .take()
                                        .resolve(&dns.hostname(), SocketAddr::new(addr, 443)),
                                );
                            }
                        };
                    }
                })
            }
        })
    }

    let host_all = unsafe {
        HOSTS_DISABLE_SNI
            .iter()
            .chain(HOSTS_ENABLE_SNI.iter())
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>()
    };

    let certificate = cert::generate(&host_all).await?;

    hosts::edit_hosts(&host_all).await?;

    let reverse_proxy_enable_sni = exact_hosts(unsafe { HOSTS_ENABLE_SNI.as_ref() })
        .map(|host: String| (format!("https://{}", host), String::new()))
        .untuple_one()
        .and(extract_request_data_filter())
        .and(with_client(CLIENT_ENABLE_SNI.get_or_init(|| {
            client_builder
                .take()
                .build()
                .expect("cannot get client which one enable sni")
        })))
        .and_then(proxy_to_and_forward_response_use_client)
        .with(warp::log::custom(|info| {
            tracing::info!(
                target: "enable_sni",
                "{} \"{} {:?} {}\" {:?}",
                info.status().as_u16(),
                info.method(),
                info.referer(),
                info.path(),
                info.elapsed()
            )
        }));

    let reverse_proxy_disable_sni = exact_hosts(unsafe { HOSTS_DISABLE_SNI.as_ref() })
        .map(|host: String| (format!("https://{}", host), String::new()))
        .untuple_one()
        .and(extract_request_data_filter())
        .and(with_client(CLIENT_DISABLE_SNI.get_or_init(|| {
            client_builder_disable_sni
                .take()
                .build()
                .expect("cannot get client which one disable sni")
        })))
        .and_then(proxy_to_and_forward_response_use_client)
        .with(warp::log::custom(|info| {
            tracing::info!(
                target: "disable_sni",
                "{} \"{} {:?} {}\" {:?}",
                info.status().as_u16(),
                info.method(),
                info.referer(),
                info.path(),
                info.elapsed()
            )
        }));

    let router = reverse_proxy_disable_sni
        .or(reverse_proxy_enable_sni)
        .recover(handle_error);

    let (addr, server) = warp::serve(router)
        .tls()
        .key(certificate.key)
        .cert(certificate.cert)
        .bind_with_graceful_shutdown(([127, 0, 0, 1], 443), async {
            if let Ok(ctrlc) = CtrlC::new() {
                tracing::info!(target: "reverse_proxy", "pressed CtrlC to shutdown");
                ctrlc.await;
                if let Err(e) = hosts::edit_hosts(&Vec::new()).await {
                    tracing::error!(target: "reverse_proxy", "failed to restore hosts {:?}", e);
                } else {
                    tracing::info!(target: "reverse_proxy", "graceful shutdown")
                }
            } else {
                tracing::error!(target: "reverse_proxy", "ctrlc hook failed")
            }
        });

    tracing::info!(target: "reverse_proxy", "listening on https://{}", addr);

    server.await;

    Ok(())
}

fn init_logger() {
    let log_name = "RUST_APP_LOG";
    if env::var(log_name).is_err() {
        env::set_var(log_name, "INFO");
    }
    pretty_env_logger::init_custom_env(log_name);
}

fn tls_config() -> TlsConfig {
    let mut root_store = rustls::RootCertStore::empty();

    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

fn extract_host() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::host::optional().and_then(async move |auth: Option<warp::host::Authority>| {
        auth.map(|auth| auth.host().to_string())
            .ok_or_else(warp::reject)
    })
}

#[allow(clippy::needless_lifetimes)]
fn exact_hosts<'a>(
    expected_hosts: &'a [String],
) -> impl Filter<Extract = (String,), Error = Rejection> + Clone + 'a {
    extract_host().and_then(async move |host: String| {
        if expected_hosts.contains(&host) {
            Ok(host)
        } else {
            Err(warp::reject())
        }
    })
}

async fn handle_error(rejection: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(err) = rejection.find::<warp_reverse_proxy::errors::Error>() {
        match err {
            warp_reverse_proxy::errors::Error::Request(e) => Ok(format!("reqwest: {:?}", e)),
            warp_reverse_proxy::errors::Error::HTTP(e) => Ok(format!("warp::http: {:?}", e)),
        }
    } else {
        Err(rejection)
    }
}

use std::env;

use config::{Hosts, HostsError};
use reqwest::Client;
use rustls::ClientConfig as TlsConfig;
use warp::{Filter, Rejection, Reply};
use warp_reverse_proxy::{
    extract_request_data_filter, proxy_to_and_forward_response, CLIENT as PROXY_CLIENT,
};

mod cert;
mod config;
mod hosts;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let mut disable_sni_reverse_proxy_client_builder =
        Client::builder().use_preconfigured_tls(tls_config());

    let hosts = parse_hosts("hosts.toml").await?;

    let mut alt_dnsname_vec = vec!["localhost"];

    if let Some(dns_vec) = hosts.dns.as_ref() {
        for dns in dns_vec {
            if let Some(addr) = dns.addr() {
                alt_dnsname_vec.push(&dns.hostname);
                disable_sni_reverse_proxy_client_builder =
                    disable_sni_reverse_proxy_client_builder.resolve(dns.hostname.as_ref(), addr);
            }
        }
    }

    let certificate = cert::generate(&alt_dnsname_vec).await?;

    hosts::edit_hosts(alt_dnsname_vec).await?;

    let disable_sni_reverse_proxy_client = disable_sni_reverse_proxy_client_builder.build()?;

    PROXY_CLIENT
        .set(disable_sni_reverse_proxy_client)
        .expect("error on proxy client set");

    let proxy = warp::any()
        .and(warp::host::optional())
        .and_then(extract_host_from_authority)
        .map(|host: String| (format!("https://{}", host), String::new()))
        .untuple_one()
        .and(extract_request_data_filter())
        .and_then(proxy_to_and_forward_response)
        .recover(handle_error)
        .with(warp::log("proxy"));

    warp::serve(proxy)
        .tls()
        .key(certificate.key)
        .cert(certificate.cert)
        .run(([127, 0, 0, 1], 443))
        .await;

    Ok(())
}

fn init_logger() {
    env::set_var("RUST_APP_LOG", "INFO");
    pretty_env_logger::init_custom_env("RUST_APP_LOG");
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

    let mut tls = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    tls.enable_sni = false;

    tls
}

async fn parse_hosts(path: &str) -> Result<Hosts, HostsError> {
    let mut hosts = Hosts::from_path(path).await?;

    hosts.lookup_all().await?;

    hosts.write_to_path(path).await?;

    Ok(hosts)
}

async fn extract_host_from_authority(
    auth: Option<warp::host::Authority>,
) -> Result<String, Rejection> {
    #[derive(Debug)]
    struct HostNotFound {}

    impl warp::reject::Reject for HostNotFound {}

    auth.and_then(|auth| Some(auth.host().to_string()))
        .ok_or(warp::reject::custom(HostNotFound {}))
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

#![feature(async_closure)]
use std::{env, sync::Arc};

use actix_tls::connect::{Connector as ActixTlsConnector, Resolver as ActixTlsResolver};
use actix_web::{middleware, web, App, HttpServer};
use async_ctrlc::CtrlC;
use awc::{Client as AwcClient, Connector as AwcConnector};
use config::{Config, ConfigMap};
use handler::{reverse_proxy, ClientPair};
use resolver::LocalHosts;
use tlscert::{cert_generate, rustls_client_config, rustls_server_config, DisableSni};
use utils::edit_hosts;

use crate::config::DnsResolve;

mod config;
mod dirs;
mod handler;
mod resolver;
mod tlscert;
mod utils;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let mut config = Config::from_file().await?;

    config.resolve().await?;

    config.update_file().await?;

    let mut all_hostname: Vec<&str> = vec![];

    let (domain_map, sni_map) = ConfigMap::from(config).split();

    let domain_map_arc = Arc::new(domain_map);
    let sni_map_data = web::Data::new(sni_map);

    sni_map_data.keys().for_each(|hostname| {
        all_hostname.push(hostname.as_str());
    });

    edit_hosts(&all_hostname).await?;

    let cert = cert_generate(&all_hostname).await?;

    let client_config_enable_sni = Arc::new(rustls_client_config());
    let client_config_disable_sni = Arc::new(rustls_client_config().disable_sni());

    let server = HttpServer::new(move || {
        let client_enable_sni = AwcClient::builder()
            .connector(
                AwcConnector::new()
                    .connector(
                        ActixTlsConnector::new(ActixTlsResolver::custom(LocalHosts::new(
                            domain_map_arc.clone(),
                        )))
                        .service(),
                    )
                    .rustls(client_config_enable_sni.clone()),
            )
            .finish();

        let client_disable_sni = AwcClient::builder()
            .connector(
                AwcConnector::new()
                    .connector(
                        ActixTlsConnector::new(ActixTlsResolver::custom(LocalHosts::new(
                            domain_map_arc.clone(),
                        )))
                        .service(),
                    )
                    .rustls(client_config_disable_sni.clone()),
            )
            .finish();

        let client_pair = web::Data::new(ClientPair::new(client_enable_sni, client_disable_sni));

        App::new()
            .app_data(sni_map_data.clone())
            .app_data(client_pair)
            .wrap(middleware::Logger::new("%{HOST}i \"%r\" %s %b %Dms"))
            .default_service(web::to(reverse_proxy))
    })
    .bind_rustls("127.0.0.1:443", rustls_server_config(cert)?)?
    .disable_signals()
    .run();

    let server_handle = server.handle();

    futures::try_join!(
        async {
            CtrlC::new()
                .expect("Failed to install Ctrl-C handler")
                .await;
            server_handle.stop(true).await;
            edit_hosts(&Vec::new()).await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        },
        async {
            server.await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        }
    )?;

    Ok(())
}

fn init_logger() {
    let log_name = "RUST_LOG";
    if env::var(log_name).is_err() {
        env::set_var(log_name, "INFO");
    }
    pretty_env_logger::init_custom_env(log_name);
}

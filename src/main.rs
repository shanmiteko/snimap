use std::{collections::HashSet, env, sync::Arc, time::Duration};

use actix_web::{
    web::{to, Data},
    App, HttpServer,
};
use async_ctrlc::CtrlC;
use config::{Config, SniMap};
use error::AnyError;
use handler::{reverse_proxy, ClientPair};
use resolver::SniMapResolver;
use tlscert::{cert_generate, rustls_client_config, rustls_server_config, DisableSni};
use utils::edit_hosts;

mod config;
mod dirs;
mod error;
mod handler;
mod resolver;
mod tlscert;
mod utils;

#[actix_web::main]
async fn main() -> Result<(), AnyError> {
    init_logger();

    let snimap = SniMap::from(Config::from_default_file().await?);

    let snimap_resolver = SniMapResolver::from_snimap(&snimap);

    let snimap = Data::new(SniMap::from(Config::from_default_file().await?));

    let hostnames = snimap.hostnames();

    edit_hosts(&hostnames).await?;

    let cert = cert_generate(&hostnames).await?;

    let (client_config_enable_sni, client_config_disable_sni) = (
        Arc::new(rustls_client_config()),
        Arc::new(rustls_client_config().disable_sni()),
    );

    let server = HttpServer::new(move || {
        App::new()
            .app_data(snimap.clone())
            .app_data(Data::new(ClientPair::new(
                client_config_enable_sni.clone(),
                client_config_disable_sni.clone(),
                snimap_resolver.clone(),
            )))
            .default_service(to(reverse_proxy))
    })
    .bind_rustls("127.0.0.1:443", rustls_server_config(cert)?)?
    .disable_signals()
    .client_request_timeout(Duration::from_secs(30))
    .client_disconnect_timeout(Duration::from_secs(30))
    .run();

    let server_handle = server.handle();

    futures::try_join!(
        async {
            CtrlC::new()
                .expect("Failed to install Ctrl-C handler")
                .await;
            log::info!(target: "proxy", "waiting for server stop ...");
            server_handle.stop(true).await;
            edit_hosts(&HashSet::new()).await?;
            log::info!(target: "proxy", "restore hosts");
            Ok::<(), AnyError>(())
        },
        async {
            log::info!(target: "proxy", "start server on :443");
            server.await?;
            Ok::<(), AnyError>(())
        }
    )?;

    Ok(())
}

fn init_logger() {
    let log_name = "RUST_LOG";
    if env::var(log_name).is_err() {
        env::set_var(log_name, "error,proxy,resolver,forward,lookup");
    }
    pretty_env_logger::init_custom_env(log_name);
}

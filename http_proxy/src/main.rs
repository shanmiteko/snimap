use std::io;

use log::info;
use logger::log_init;
use tokio::{net::TcpListener, spawn};

use crate::httproxy::HTTProxy;

mod error;
mod httproxy;
mod logger;
mod utils;

#[tokio::main]
async fn main() -> io::Result<()> {
    log_init();

    let listener = TcpListener::bind(("0.0.0.0", 8080)).await?;
    info!("listen in {}", listener.local_addr()?);

    while let Ok((client, addr)) = listener.accept().await {
        spawn(HTTProxy::new(client, addr).serve());
    }

    Ok(())
}

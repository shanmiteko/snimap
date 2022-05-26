use std::{sync::Arc, time::Duration};

use crate::config::SniMap;
use actix_web::{
    dev::RequestHead,
    http::{header, Uri, Version},
    web, HttpRequest, HttpResponse,
};
use awc::Client;
use url::Url;

/// (enable_sni, disable_sni)
pub struct ClientPair(Arc<Client>, Arc<Client>);

impl ClientPair {
    pub fn new(client_enable_sni: Client, client_disable_sni: Client) -> Self {
        Self(Arc::new(client_enable_sni), Arc::new(client_disable_sni))
    }

    pub fn client_enable_sni(&self) -> Arc<Client> {
        self.0.clone()
    }

    pub fn client_disable_sni(&self) -> Arc<Client> {
        self.1.clone()
    }
}

#[inline]
async fn forward(
    client: Arc<Client>,
    request_url: &Uri,
    head: &RequestHead,
    payload: web::Payload,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let awc_response = client
        .request_from(request_url.clone(), head)
        .timeout(Duration::from_secs(10))
        .no_decompress()
        .send_stream(payload)
        .await?;
    let mut response = HttpResponse::build(awc_response.status());
    for (header_name, header_value) in awc_response
        .headers()
        .iter()
        .filter(|(h, _)| *h != "connection")
    {
        response.insert_header((header_name.clone(), header_value.clone()));
    }
    Ok(response.streaming(awc_response))
}

pub async fn reverse_proxy(
    request: HttpRequest,
    payload: web::Payload,
    sni_map: web::Data<SniMap>,
    client_pair: web::Data<ClientPair>,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    match match request.version() {
        Version::HTTP_09 | Version::HTTP_10 | Version::HTTP_11 => request
            .headers()
            .get(header::HOST)
            .map(|h| h.to_str().unwrap()),
        _ => request.uri().host(),
    } {
        Some(host) => match sni_map.get(host) {
            Some(sni_maybe_none) => match sni_maybe_none {
                Some(sni) => match host == sni {
                    true => {
                        forward(
                            client_pair.client_enable_sni(),
                            &request.uri(),
                            &request.head(),
                            payload,
                        )
                        .await
                    }
                    false => {
                        let mut request_url = Url::parse(&request.uri().to_string())?;
                        request_url.set_host(Some(sni.as_str()))?;
                        forward(
                            client_pair.client_enable_sni(),
                            &Uri::try_from(request_url.as_str())?,
                            &request.head(),
                            payload,
                        )
                        .await
                    }
                },
                None => {
                    forward(
                        client_pair.client_disable_sni(),
                        &request.uri(),
                        &request.head(),
                        payload,
                    )
                    .await
                }
            },
            None => Ok(HttpResponse::Forbidden()
                .body(format!("Host \"{host}\" not enabled in config.toml"))),
        },
        None => Ok(HttpResponse::NotFound().body("Host not found")),
    }
}

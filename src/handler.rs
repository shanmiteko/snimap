use std::sync::Arc;

use crate::config::SniMap;
use actix_web::{
    dev::RequestHead,
    http::{header, uri::PathAndQuery, Uri, Version},
    web, HttpRequest, HttpResponse,
};
use awc::Client;

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
    sni: &str,
    head: &RequestHead,
    payload: web::Payload,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let awc_request = client
        .request_from(
            Uri::try_from(format!(
                "{}://{}{}",
                head.uri.scheme_str().unwrap_or("https"),
                sni,
                head.uri
                    .path_and_query()
                    .unwrap_or(&PathAndQuery::from_static("/"))
            ))?,
            head,
        )
        // .version(head.version)
        .no_decompress();
    let (method, version) = (
        awc_request.get_method().to_owned(),
        awc_request.get_version().to_owned(),
    );
    let awc_response = match awc_request.send_stream(payload).await {
        Ok(r) => {
            log::info!(
                target: "forward",
                "{} \"{} {} {:?}\" host: {:?} {} {:?}",
                sni,
                method,
                head.uri.path(),
                version,
                head.headers()
                    .get(header::HOST)
                    .unwrap(),
                r.status(),
                r.version(),
            );
            r
        }
        Err(e) => {
            log::error!(
                target: "forward",
                "{} \"{} {} {:?}\" host: {:?} error: {}",
                sni,
                method,
                head.uri.path(),
                version,
                head.headers()
                    .get(header::HOST)
                    .unwrap(),
                e
            );
            return Err(e.into());
        }
    };
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
            Some(sni_maybe_none) => {
                let mut head = request.head().clone();
                head.headers_mut()
                    .insert(header::HOST, header::HeaderValue::from_str(host)?);
                match sni_maybe_none {
                    Some(sni) => {
                        forward(client_pair.client_enable_sni(), sni, &head, payload).await
                    }
                    None => forward(client_pair.client_disable_sni(), host, &head, payload).await,
                }
            }
            None => Ok(HttpResponse::Forbidden()
                .body(format!("Host \"{host}\" not enabled in config.toml"))),
        },
        None => Ok(HttpResponse::NotFound().body("Host not found")),
    }
}

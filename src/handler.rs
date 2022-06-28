use std::{sync::Arc, time::Duration};

use crate::{
    config::{Sni, SniMap},
    error::AnyError,
    resolver::SniMapResolver,
};
use actix_tls::connect::{Connector as ActixTlsConnector, Resolver};
use actix_web::{
    dev::RequestHead,
    http::{header, uri::PathAndQuery, Uri, Version},
    web::{Data, Payload},
    HttpRequest, HttpResponse,
};
use awc::{Client as AwcClient, Connector as AwcConnector};
use rustls::ClientConfig;

/// (enable_sni, disable_sni)
pub struct ClientPair(AwcClient, AwcClient);

impl ClientPair {
    pub fn new(
        client_config_enable_sni: Arc<ClientConfig>,
        client_config_disable_sni: Arc<ClientConfig>,
        snimap_resolver: SniMapResolver,
    ) -> Self {
        let client_enable_sni = AwcClient::builder()
            .timeout(Duration::from_secs(30))
            .connector(
                AwcConnector::new()
                    .connector(
                        ActixTlsConnector::new(Resolver::custom(snimap_resolver.clone())).service(),
                    )
                    .timeout(Duration::from_secs(30))
                    .rustls(client_config_enable_sni),
            )
            .disable_redirects()
            .finish();

        let client_disable_sni = AwcClient::builder()
            .timeout(Duration::from_secs(30))
            .connector(
                AwcConnector::new()
                    .connector(ActixTlsConnector::new(Resolver::custom(snimap_resolver)).service())
                    .timeout(Duration::from_secs(30))
                    .rustls(client_config_disable_sni),
            )
            .disable_redirects()
            .finish();

        Self(client_enable_sni, client_disable_sni)
    }

    pub fn client_enable_sni(&self) -> &AwcClient {
        &self.0
    }

    pub fn client_disable_sni(&self) -> &AwcClient {
        &self.1
    }
}

#[inline]
async fn forward(
    client: &AwcClient,
    sni: &str,
    RequestHead {
        method,
        uri,
        version,
        headers,
        ..
    }: RequestHead,
    payload: Payload,
) -> Result<HttpResponse, AnyError> {
    let mut awc_request = client
        .request(
            method.clone(),
            Uri::try_from(format!(
                "{}://{}{}",
                uri.scheme_str().unwrap_or("https"),
                sni,
                uri.path_and_query()
                    .unwrap_or(&PathAndQuery::from_static("/"))
            ))?,
        )
        .no_decompress();
    let host = headers.get(header::HOST).unwrap().clone();
    for (nhk, nhv) in headers.into_iter() {
        match awc_request.headers_mut().get_mut(&nhk) {
            Some(hv) => *hv = format!("{};{}", hv.to_str()?, nhv.to_str()?).try_into()?,
            None => {
                awc_request.headers_mut().insert(nhk, nhv);
            }
        }
    }
    let awc_response = match awc_request.send_stream(payload).await {
        Ok(r) => {
            log::info!(
                target: "forward",
                "{} \"{} {} {:?}\" host: {:?} {} {:?}",
                sni,
                method,
                uri.path(),
                version,
                host,
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
                uri.path(),
                version,
                host,
                e
            );
            return Err(e.into());
        }
    };
    let mut response = HttpResponse::build(awc_response.status());
    for (header_name, header_value) in awc_response.headers().iter() {
        response.append_header((header_name.clone(), header_value.clone()));
    }
    Ok(response.streaming(awc_response))
}

pub async fn reverse_proxy(
    request: HttpRequest,
    payload: Payload,
    snimap: Data<SniMap>,
    client_pair: Data<ClientPair>,
) -> Result<HttpResponse, AnyError> {
    match match request.version() {
        Version::HTTP_09 | Version::HTTP_10 | Version::HTTP_11 => request
            .headers()
            .get(header::HOST)
            .map(|h| h.to_str().unwrap()),
        _ => request.uri().host(),
    } {
        Some(host) => match snimap.get(host) {
            Some(sni) => {
                let mut head = request.head().clone();
                head.headers_mut()
                    .insert(header::HOST, header::HeaderValue::from_str(host)?);
                match sni {
                    Sni::Disable => {
                        forward(client_pair.client_disable_sni(), host, head, payload).await
                    }
                    Sni::Override(sni) | Sni::Remain(sni) => {
                        forward(client_pair.client_enable_sni(), sni, head, payload).await
                    }
                }
            }
            None => Ok(HttpResponse::Forbidden().body(format!(
                "`hostname = \"{host}\"` is not enabled in config.toml"
            ))),
        },
        None => Ok(HttpResponse::NotFound().body("cannot find 'host=xxx' in header")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use actix_web::{
        http, test,
        web::{to, Data},
        App,
    };

    use crate::{
        config::{Mapping, SniMap, Switchable},
        handler::{reverse_proxy, ClientPair},
        resolver::SniMapResolver,
        tlscert::{rustls_client_config, DisableSni},
    };

    async fn test_reverse_proxy_use(
        snimap: SniMap,
        headers: Option<Vec<(&str, &str)>>,
    ) -> http::StatusCode {
        let snimap_resolver = SniMapResolver::from_snimap(&snimap);
        let snimap_data = Data::new(snimap);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(snimap_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    snimap_resolver,
                )))
                .default_service(to(reverse_proxy)),
        )
        .await;

        let mut test_req = test::TestRequest::get().uri("/");

        if let Some(headers) = headers {
            for header in headers {
                test_req = test_req.insert_header(header);
            }
        }

        let resp = test::call_service(&mut srv, test_req.to_request()).await;

        dbg!(resp.response().body());

        resp.status()
    }

    #[actix_web::test]
    async fn test_reverse_proxy_no_host() {
        assert_eq!(
            test_reverse_proxy_use(Mapping::new("example.com").into(), None).await,
            http::StatusCode::NOT_FOUND
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_not_enabled_in_config() {
        assert_eq!(
            test_reverse_proxy_use(SniMap::new(), Some(vec![("host", "example.com")])).await,
            http::StatusCode::FORBIDDEN
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_enable_sni() {
        assert!(
            test_reverse_proxy_use(
                Mapping::new("www.duckduckgo.com").into(),
                Some(vec![("host", "www.duckduckgo.com")])
            )
            .await
            .is_redirection(),
            "www.duckduckgo.com should be redirected"
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_disable_sni() {
        assert!(
            test_reverse_proxy_use(
                Mapping::new("en.wikipedia.org").disable_sni().into(),
                Some(vec![("host", "en.wikipedia.org")])
            )
            .await
            .is_redirection(),
            "en.wikipedia.org should be redirected"
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_enable_sni_domain_fronting() {
        assert!(
            test_reverse_proxy_use(
                Mapping::new("www.pixiv.net")
                    .override_sni("www.fanbox.cc")
                    .into(),
                Some(vec![("host", "www.pixiv.net")])
            )
            .await
            .is_success(),
            "www.pixiv.net should be success"
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_post() {
        use actix_web::body::to_bytes;

        let snimap = Mapping::new("httpbin.org").into();
        let snimap_resolver = SniMapResolver::from_snimap(&snimap);
        let snimap_data = Data::new(snimap);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(snimap_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    snimap_resolver,
                )))
                .default_service(to(reverse_proxy)),
        )
        .await;

        let test_req = test::TestRequest::post()
            .uri("/post")
            .insert_header(("host", "httpbin.org"))
            .set_json(r#"{"data":"test_reverse_proxy_post"}"#);

        let resp = test::call_service(&mut srv, test_req.to_request()).await;

        let body = String::from_utf8(
            to_bytes(resp.into_body())
                .await
                .expect("body to bytes")
                .to_ascii_lowercase(),
        )
        .unwrap();

        assert!(dbg!(body).contains("test_reverse_proxy_post"))
    }

    #[actix_web::test]
    async fn test_reverse_proxy_cookie() {
        use actix_web::body::to_bytes;

        let snimap = Mapping::new("httpbin.org").into();
        let snimap_resolver = SniMapResolver::from_snimap(&snimap);
        let snimap_data = Data::new(snimap);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(snimap_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    snimap_resolver,
                )))
                .default_service(to(reverse_proxy)),
        )
        .await;

        let test_req = test::TestRequest::get()
            .uri("/cookies")
            .insert_header(("host", "httpbin.org"))
            .insert_header(("cookie", "a=b; c=d; e=fffff"));

        let resp = test::call_service(&mut srv, test_req.to_request()).await;

        let body = String::from_utf8(
            to_bytes(resp.into_body())
                .await
                .expect("body to bytes")
                .to_ascii_lowercase(),
        )
        .unwrap();

        assert!(dbg!(body).contains("fffff"))
    }
}

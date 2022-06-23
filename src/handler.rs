use std::{sync::Arc, time::Duration};

use crate::{config::SniMap, error::AnyError, resolver::DnsCache};
use actix_tls::connect::{Connector as ActixTlsConnector, Resolver};
use actix_web::{
    dev::RequestHead,
    http::{header, uri::PathAndQuery, Uri, Version},
    web, HttpRequest, HttpResponse,
};
use awc::{Client as AwcClient, Connector as AwcConnector};
use rustls::ClientConfig;

/// (enable_sni, disable_sni)
pub struct ClientPair(AwcClient, AwcClient);

impl ClientPair {
    pub fn new(
        client_config_enable_sni: Arc<ClientConfig>,
        client_config_disable_sni: Arc<ClientConfig>,
        dns_cache: DnsCache,
    ) -> Self {
        let client_enable_sni = AwcClient::builder()
            .timeout(Duration::from_secs(30))
            .connector(
                AwcConnector::new()
                    .connector(
                        ActixTlsConnector::new(Resolver::custom(dns_cache.clone())).service(),
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
                    .connector(ActixTlsConnector::new(Resolver::custom(dns_cache)).service())
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
    payload: web::Payload,
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
    payload: web::Payload,
    sni_map: web::Data<SniMap>,
    client_pair: web::Data<ClientPair>,
) -> Result<HttpResponse, AnyError> {
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
                    Some(sni) => forward(client_pair.client_enable_sni(), sni, head, payload).await,
                    None => forward(client_pair.client_disable_sni(), host, head, payload).await,
                }
            }
            None => Ok(HttpResponse::Forbidden()
                .body(format!("'host=\"{host}\"' not enabled in config.toml"))),
        },
        None => Ok(HttpResponse::NotFound().body("'host=xxx' not found in header")),
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
        config::SniMap,
        handler::{reverse_proxy, ClientPair},
        resolver::DnsCache,
        tlscert::{rustls_client_config, DisableSni},
    };

    async fn test_reverse_proxy_use(
        hostname: Option<&str>,
        sni: Option<&str>,
        headers: Option<Vec<(&str, &str)>>,
    ) -> http::StatusCode {
        let mut sni_map = SniMap::new();
        let mut dns_cache = DnsCache::new();
        if let Some(host) = hostname {
            sni_map.insert(host.into(), sni.map(|s| s.into()));
            dns_cache = DnsCache::new().with_whitelist(&[host]);
        }
        let sni_map_data = Data::new(sni_map);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(sni_map_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    dns_cache,
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
            test_reverse_proxy_use(Some("example.com"), None, None).await,
            http::StatusCode::NOT_FOUND
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_not_enabled_in_config() {
        assert_eq!(
            test_reverse_proxy_use(None, None, Some(vec![("host", "example.com")])).await,
            http::StatusCode::FORBIDDEN
        );
    }

    #[actix_web::test]
    async fn test_reverse_proxy_enable_sni() {
        assert!(
            test_reverse_proxy_use(
                Some("www.duckduckgo.com"),
                Some("www.duckduckgo.com"),
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
                Some("en.wikipedia.org"),
                None,
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
                Some("www.pixiv.net"),
                Some("www.fanbox.cc"),
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

        let mut sni_map = SniMap::new();
        sni_map.insert("httpbin.org".to_string(), Some("httpbin.org".to_string()));
        let dns_cache = DnsCache::new();
        let sni_map_data = Data::new(sni_map);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(sni_map_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    dns_cache,
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

        let mut sni_map = SniMap::new();
        sni_map.insert("httpbin.org".to_string(), Some("httpbin.org".to_string()));
        let dns_cache = DnsCache::new();
        let sni_map_data = Data::new(sni_map);
        let (client_config_enable_sni, client_config_disable_sni) = (
            Arc::new(rustls_client_config()),
            Arc::new(rustls_client_config().disable_sni()),
        );
        let mut srv = test::init_service(
            App::new()
                .app_data(sni_map_data.clone())
                .app_data(Data::new(ClientPair::new(
                    client_config_enable_sni.clone(),
                    client_config_disable_sni.clone(),
                    dns_cache,
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

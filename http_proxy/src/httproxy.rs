use std::{borrow::BorrowMut, net::SocketAddr};

use bytes::BytesMut;

use dns::resolver::Resolver;
use http::{
    extract_host, http_head_end,
    request::Request,
    respond::{RespondBuilder, Status},
};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use tokio::net::TcpStream;

use crate::{
    error::{ProxyError, ProxyErrorKind, ProxyResult},
    utils::{pipe, read_until, tokio_timeout},
};

static RESOLVER: Lazy<Resolver> = Lazy::new(Resolver::default);

pub struct HTTProxy {
    client: (TcpStream, SocketAddr),
    server_name: Option<String>,
    /// default `false`
    dns_on_web: bool,
}

enum ServerAddr<'a> {
    SocketAddr(SocketAddr),
    ServerName(&'a str),
}

impl HTTProxy {
    pub fn new(client_socket: TcpStream, socket_addr: SocketAddr) -> Self {
        Self {
            client: (client_socket, socket_addr),
            server_name: None,
            dns_on_web: false,
        }
    }

    /// DNS lookup using web spider
    pub fn dns_on_web(mut self) -> Self {
        self.dns_on_web = true;
        self
    }

    fn client_stream(&mut self) -> &mut TcpStream {
        self.client.0.borrow_mut()
    }

    fn client_addr(&self) -> &SocketAddr {
        &self.client.1
    }

    fn server_name(&self) -> &str {
        self.server_name.as_ref().unwrap()
    }

    async fn extract_server_addr(&mut self, uri: &[u8]) -> ProxyResult<ServerAddr> {
        let server_host = extract_host(uri).ok_or_else(|| {
            ProxyError::new(ProxyErrorKind::InvalidHost)
                .from("cannot find server host")
                .downstream(self.client_addr())
                .context(format!("uri: {:?}", uri))
        })?;

        info!("{} -> {}", &self.client_addr(), &server_host);

        self.server_name = Some(server_host);

        Ok(match self.dns_on_web {
            true => ServerAddr::SocketAddr(match self.server_name().parse::<SocketAddr>() {
                Ok(s) => s,
                Err(_) => {
                    let (name, port) = self.server_name().split_once(':').unwrap();
                    SocketAddr::new(
                        RESOLVER.lookup_ip(name).await.ok_or_else(|| {
                            ProxyError::new(ProxyErrorKind::HostNotFound)
                                .downstream(self.client_addr())
                                .upstream(self.server_name())
                                .context("dns on web")
                        })?,
                        port.parse().unwrap(),
                    )
                }
            }),
            false => ServerAddr::ServerName(self.server_name()),
        })
    }

    async fn handshake(&mut self) -> ProxyResult<TcpStream> {
        // HTTP head MUST < 2048 Bytes
        let mut buf = BytesMut::with_capacity(2048);

        // Read until find "\r\n\r\n"
        read_until(self.client_stream(), 6, &mut buf, http_head_end)
            .await
            .map_err(|e| {
                ProxyError::new(ProxyErrorKind::ReadTimeout)
                    .from(e)
                    .downstream(self.client_addr())
                    .context("while waiting for read client data")
            })?
            .map_err(|e| {
                ProxyError::new(ProxyErrorKind::ReadIo)
                    .from(e)
                    .downstream(self.client_addr())
                    .context("while reading client data")
            })?;

        // Parse HTTP head
        let http_request = Request::from_bytes(&buf).ok_or_else(|| {
            ProxyError::new(ProxyErrorKind::NotHttpRequest)
                .from("not http request")
                .downstream(self.client_addr())
                .context(format!("raw data: {:?}", buf))
        })?;

        debug!("{} {:?}", &self.client_addr(), &http_request);

        // DNS
        let server_addr = self.extract_server_addr(http_request.uri).await?;

        // Connect to server
        let mut server = match server_addr {
            ServerAddr::SocketAddr(addr) => tokio_timeout(20, TcpStream::connect(addr)).await,
            ServerAddr::ServerName(name) => tokio_timeout(20, TcpStream::connect(name)).await,
        }
        .map_err(|e| {
            ProxyError::new(ProxyErrorKind::ConnectTimeout)
                .from(e)
                .downstream(self.client_addr())
                .upstream(self.server_name())
                .context("while waiting for connected to server")
        })?
        .map_err(|e| {
            ProxyError::new(ProxyErrorKind::ConnectIo)
                .from(e)
                .downstream(self.client_addr())
                .upstream(self.server_name())
                .context("while connecting to server")
        })?;

        // Tunnel or Direct Relay
        if http_request.method == b"CONNECT" {
            // Establish HTTP proxy tunnel
            RespondBuilder::default()
                .nobody()
                .send_to(self.client_stream())
                .await
                .map_err(|e| {
                    ProxyError::new(ProxyErrorKind::ConnectIo)
                        .from(e)
                        .downstream(self.server_name())
                        .upstream(self.client_addr())
                        .context("while replying to client that tunnel has been established")
                })?;
        } else {
            // Direct Relay
            http_request
                .headers_filter(|header| header[..5].to_ascii_lowercase() != b"proxy")
                .send_to(&mut server)
                .await
                .map_err(|e| {
                    ProxyError::new(ProxyErrorKind::ConnectIo)
                        .from(e)
                        .downstream(self.client_addr())
                        .upstream(self.server_name())
                        .context("while direct relaying client request")
                })?;
        };

        Ok(server)
    }

    pub async fn serve(mut self) {
        match self.handshake().await {
            Err(proxy_error) => {
                let reply_result = match proxy_error.kind() {
                    ProxyErrorKind::NotHttpRequest
                    | ProxyErrorKind::InvalidHost
                    | ProxyErrorKind::HostNotFound
                    | ProxyErrorKind::ReadIo => {
                        RespondBuilder::default()
                            .status(Status::BadRequest)
                            .nobody()
                            .send_to(self.client_stream())
                            .await
                    }
                    ProxyErrorKind::ConnectIo => {
                        RespondBuilder::default()
                            .status(Status::BadGateway)
                            .nobody()
                            .send_to(self.client_stream())
                            .await
                    }
                    ProxyErrorKind::ConnectTimeout => {
                        RespondBuilder::default()
                            .status(Status::GatewayTimeout)
                            .nobody()
                            .send_to(self.client_stream())
                            .await
                    }
                    ProxyErrorKind::ReadTimeout => {
                        RespondBuilder::default()
                            .status(Status::RequestTimeout)
                            .nobody()
                            .send_to(self.client_stream())
                            .await
                    }
                    ProxyErrorKind::Other => Ok(0),
                };
                warn!("{} :reply {:?}", proxy_error, reply_result);
            }
            Ok(mut server) => {
                let (mut client_reader, mut client_writer) = self.client.0.split();
                let (mut server_reader, mut server_writer) = server.split();

                //Tcp Tunnel
                match tokio::try_join!(
                    pipe(&mut client_reader, &mut server_writer),
                    pipe(&mut server_reader, &mut client_writer)
                ) {
                    Ok(_) => (),
                    Err(e) => warn!(
                        "{}",
                        ProxyError::new(ProxyErrorKind::ReadIo)
                            .from(e)
                            .context("tcp tunnel")
                            .downstream(self.client_addr())
                            .upstream(self.server_name())
                    ),
                };
            }
        }
    }
}

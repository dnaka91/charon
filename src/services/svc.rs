//! Main service for proxying logic.

use std::{
    convert::Infallible,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use eyre::Result;
use future::try_join;
use futures_util::future;
use hyper::{
    client::HttpConnector, http::uri::PathAndQuery, upgrade::Upgraded, Body, Client, Method,
    Request, Response, StatusCode, Uri,
};
use tokio::{net::TcpStream, task};
use tower::{Service, ServiceBuilder};

use super::log::{LogLayer, LogService};
use crate::tls::TlsStream;

type ResponseFuture<T, E> = Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>;

pub struct Svc {
    client: Client<HttpConnector>,
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = ResponseFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        log::info!("{:?}", req);

        let uri_string = format!(
            "{}://localhost:1111{}",
            req.uri().scheme_str().unwrap_or("http"),
            req.uri().path_and_query().map_or("/", PathAndQuery::as_str)
        );
        let uri = uri_string.parse().unwrap();

        *req.uri_mut() = uri;

        Box::pin(proxy(self.client.clone(), req))
    }
}

async fn proxy(
    client: Client<HttpConnector>,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    if req.method() == Method::CONNECT {
        if let Some(addr) = host_addr(req.uri()) {
            task::spawn(async move {
                match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        if let Err(e) = tunnel(upgraded, addr).await {
                            log::warn!("server io error: {}", e);
                        }
                    }
                    Err(e) => log::warn!("upgrade error: {}", e),
                }
            });

            Ok(Response::new(Body::empty()))
        } else {
            log::warn!("CONNECT host is not a socket address: {:?}", req.uri());

            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;

            Ok(resp)
        }
    } else {
        client.request(req).await
    }
}

fn host_addr(uri: &Uri) -> Option<SocketAddr> {
    uri.authority().and_then(|auth| auth.as_str().parse().ok())
}

async fn tunnel(upgraded: Upgraded, addr: SocketAddr) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;

    let amounts = {
        let (mut server_rx, mut server_tx) = server.split();
        let (mut client_rx, mut client_tx) = tokio::io::split(upgraded);

        let client_to_server = tokio::io::copy(&mut client_rx, &mut server_tx);
        let server_to_client = tokio::io::copy(&mut server_rx, &mut client_tx);

        try_join(client_to_server, server_to_client).await
    };

    match amounts {
        Ok((from_client, from_server)) => log::info!(
            "client wrote {} bytes, received {} bytes",
            from_client,
            from_server
        ),
        Err(e) => log::warn!("tunnel error: {}", e),
    }

    Ok(())
}

pub struct MakeSvc {
    client: Client<HttpConnector>,
}

impl MakeSvc {
    pub const fn new(client: Client<HttpConnector>) -> Self {
        Self { client }
    }
}

impl Service<&TlsStream> for MakeSvc {
    type Response = LogService<Svc>;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, conn: &TlsStream) -> Self::Future {
        future::ok(
            ServiceBuilder::new()
                .layer(LogLayer::new(conn.remote_addr))
                .service(Svc {
                    client: self.client.clone(),
                }),
        )
    }
}

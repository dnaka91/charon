//! Main service for proxying logic.

use std::convert::Infallible;
use std::task::{Context, Poll};

use eyre::Result;
use futures_util::future;
use hyper::client::HttpConnector;
use hyper::http::uri::PathAndQuery;
use hyper::{Body, Client, Request, Response};
use tower::Service;
use tower::ServiceBuilder;

use crate::log::{LogLayer, LogService};
use crate::tls::TlsStream;

pub struct Svc {
    client: Client<HttpConnector>,
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = hyper::client::ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let uri_string = format!(
            "http://localhost:1111{}",
            req.uri().path_and_query().map_or("/", PathAndQuery::as_str)
        );
        let uri = uri_string.parse().unwrap();

        *req.uri_mut() = uri;
        self.client.request(req)
    }
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

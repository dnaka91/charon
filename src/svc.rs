//! Main service for proxying logic.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::task::{Context, Poll};

use chrono::prelude::*;
use eyre::Result;
use futures_util::future;
use hyper::client::HttpConnector;
use hyper::http::header::{REFERER, USER_AGENT};
use hyper::http::uri::PathAndQuery;
use hyper::{Body, Client, Request, Response};
use tower::Service;
use tracing::info;

use crate::tls::TlsStream;

pub struct Svc {
    remote_addr: SocketAddr,
    client: Client<HttpConnector>,
}

impl Svc {
    fn log_request(remote_addr: SocketAddr, req: &Request<Body>) {
        let path_and_query = req.uri().path_and_query().map_or("/", PathAndQuery::as_str);

        let referrer = req
            .headers()
            .get(REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");

        let user_agent = req
            .headers()
            .get(USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");

        info!(
            r#"{} - - [{}] "{} {} {:?}" 200 0 "{}" "{}" 0 "-" "-" 0ms"#,
            remote_addr.ip(),
            Utc::now().format("%d/%b/%Y:%T %z"),
            req.method(),
            path_and_query,
            req.version(),
            referrer,
            user_agent,
        );
    }
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = hyper::client::ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        Self::log_request(self.remote_addr, &req);

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
    type Response = Svc;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, conn: &TlsStream) -> Self::Future {
        future::ok(Svc {
            remote_addr: conn.remote_addr,
            client: self.client.clone(),
        })
    }
}

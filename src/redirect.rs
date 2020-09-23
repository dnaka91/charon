//! HTTP to HTTPS redirect service.

use std::convert::Infallible;
use std::task::{Context, Poll};

use eyre::Result;
use futures_util::future;
use hyper::http::header::{HOST, LOCATION};
use hyper::http::uri::PathAndQuery;
use hyper::http::StatusCode;
use hyper::http::Uri;
use hyper::{Body, Request, Response};
use tower::Service;

pub struct Redirect;

impl Service<Request<Body>> for Redirect {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let rsp = Response::builder();

        let host = req
            .headers()
            .get(HOST)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<Uri>().ok());

        let rsp = if let Some(host) = host.as_ref().and_then(Uri::host) {
            let location = format!(
                "https://{}:{}{}",
                host,
                8443,
                req.uri().path_and_query().map_or("/", PathAndQuery::as_str)
            );

            rsp.status(StatusCode::PERMANENT_REDIRECT)
                .header(LOCATION, location)
        } else {
            rsp.status(StatusCode::NOT_FOUND)
        };

        future::ok(rsp.body(Body::empty()).unwrap())
    }
}

pub struct MakeRedirect;

impl<T> Service<T> for MakeRedirect {
    type Response = Redirect;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _req: T) -> Self::Future {
        future::ok(Redirect)
    }
}

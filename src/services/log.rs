//! Logging middleware service.

use std::{
    fmt::{self, Display},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

use chrono::prelude::*;
use headers::{
    authorization::Basic, Authorization, ContentLength, Header, HeaderMapExt, Referer, UserAgent,
};
use hyper::{http::uri::PathAndQuery, Request, Response};
use log::info;
use pin_project::pin_project;
use tower::{layer::Layer, Service};

pub struct LogLayer {
    remote_addr: SocketAddr,
}

impl LogLayer {
    pub const fn new(remote_addr: SocketAddr) -> Self {
        Self { remote_addr }
    }
}

impl<S> Layer<S> for LogLayer {
    type Service = LogService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LogService {
            remote_addr: self.remote_addr,
            inner,
        }
    }
}

pub struct LogService<T> {
    remote_addr: SocketAddr,
    inner: T,
}

impl<S, T> Service<Request<T>> for LogService<S>
where
    S: Service<Request<T>, Response = Response<T>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<T>) -> Self::Future {
        ResponseFuture {
            start: Instant::now(),
            remote_addr: self.remote_addr,
            request: extract(&req),
            response: self.inner.call(req),
        }
    }
}

fn extract<T>(req: &Request<T>) -> RequestInfo {
    let path_and_query = req
        .uri()
        .path_and_query()
        .cloned()
        .unwrap_or_else(|| PathAndQuery::from_static("/"));

    RequestInfo {
        path_and_query,
        authorization: req.headers().typed_get(),
        referrer: req
            .headers()
            .get(Referer::name())
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        user_agent: req.headers().typed_get(),
        method: req.method().clone(),
        version: req.version(),
    }
}

struct RequestInfo {
    path_and_query: PathAndQuery,
    authorization: Option<Authorization<Basic>>,
    referrer: Option<String>,
    user_agent: Option<UserAgent>,
    method: hyper::Method,
    version: hyper::Version,
}

#[pin_project]
pub struct ResponseFuture<T> {
    start: Instant,
    remote_addr: SocketAddr,
    request: RequestInfo,
    #[pin]
    response: T,
}

impl<F, T, E> ResponseFuture<F>
where
    F: Future<Output = Result<Response<T>, E>>,
{
    fn log(remote_addr: SocketAddr, req: &RequestInfo, start: Instant, rsp: &Response<T>) {
        let content_length = rsp
            .headers()
            .typed_get::<ContentLength>()
            .map(|v| v.0)
            .unwrap_or_default();

        info!(
            r#"{} - {} [{}] "{} {} {:?}" {} {} "{}" "{}" 0 "-" "-" {}ms"#,
            remote_addr.ip(),
            req.authorization.as_ref().map(|a| a.0.username()).display(),
            Utc::now().format("%d/%b/%Y:%T %z"),
            req.method,
            req.path_and_query,
            req.version,
            rsp.status().as_u16(),
            content_length,
            req.referrer.display(),
            req.user_agent.display(),
            start.elapsed().as_millis()
        );
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<T>, E>>,
{
    type Output = Result<Response<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.response.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(rsp) => {
                if let Ok(rsp) = &rsp {
                    Self::log(*this.remote_addr, this.request, *this.start, rsp);
                }
                Poll::Ready(rsp)
            }
        }
    }
}

struct DisplayOption<'a, T: Display>(&'a Option<T>);

impl<'a, T: Display> Display for DisplayOption<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(v) => v.fmt(f),
            None => f.write_str("-"),
        }
    }
}

trait Opt<T: Display> {
    fn display(&self) -> DisplayOption<'_, T>;
}

impl<T: Display> Opt<T> for Option<T> {
    fn display(&self) -> DisplayOption<'_, T> {
        DisplayOption(self)
    }
}

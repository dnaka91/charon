//! # 🕯️ Charon
//!
//! A reverse proxy written in Rust with zero downtime Let's Encrypt cerficate refreshes and Docker
//! Compose integration.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]

use std::env;
use std::sync::Arc;

use eyre::Result;
use futures_util::future;
use hyper::server::conn::AddrIncoming;
use hyper::{Client, Server};
use rustls::{NoClientAuth, ServerConfig};

use crate::cert::Resolver;
use crate::tls::TlsAcceptor;

mod cert;
mod redirect;
mod svc;
mod tls;

#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_LOG", "info,charon=trace");
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let resolver = cert::load()?;
    let resolver = Arc::new(Resolver::new(resolver));

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.cert_resolver = resolver.clone();

    let incoming = AddrIncoming::bind(&([127, 0, 0, 1], 8443).into())?;
    let acceptor = TlsAcceptor::new(config, incoming);

    let client = Client::new();

    let https_server = Server::builder(acceptor).serve(svc::MakeSvc::new(client));

    let http_addr = ([127, 0, 0, 1], 8080).into();
    let http_server = Server::bind(&http_addr).serve(redirect::MakeRedirect);

    let ret = future::join(https_server, http_server).await;
    ret.0?;
    ret.1?;

    Ok(())
}

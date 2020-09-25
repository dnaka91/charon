//! # 🕯️ Charon
//!
//! A reverse proxy written in Rust with zero downtime Let's Encrypt cerficate refreshes and Docker
//! Compose integration.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(dead_code, clippy::module_name_repetitions)]

use std::env;
use std::sync::Arc;

use eyre::Result;
use futures_util::future;
use hyper::server::conn::AddrIncoming;
use hyper::{Client, Server};
use rustls::{NoClientAuth, ServerConfig};
use tracing::info;

use crate::acme::{Acme, ChallengeStorage};
use crate::cert::Resolver;
use crate::tls::TlsAcceptor;

mod acme;
mod cert;
mod redirect;
mod settings;
mod svc;
mod tls;

#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_LOG", "info,charon=trace");
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let settings = settings::load()?;
    let challenges = ChallengeStorage::default();

    let acme = Acme::new(challenges.clone(), &settings.acme.email)?;

    let certs = acme.load_certs(settings.routes.keys())?;

    let resolver = cert::load(&certs)?;
    let resolver = Arc::new(Resolver::new(resolver));

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.cert_resolver = resolver.clone();

    let https_addr = ([0, 0, 0, 0], 8443).into();
    let incoming = AddrIncoming::bind(&https_addr)?;
    let acceptor = TlsAcceptor::new(config, incoming);

    let client = Client::new();

    let https_server = Server::builder(acceptor).serve(svc::MakeSvc::new(client));

    let http_addr = ([0, 0, 0, 0], 8080).into();
    let http_server = Server::bind(&http_addr).serve(redirect::MakeRedirect::new(challenges));

    info!("listening on {} for HTTP", http_addr);
    info!("listening on {} for HTTPS", https_addr);

    let ret = future::join(https_server, http_server).await;
    ret.0?;
    ret.1?;

    Ok(())
}

//! # 🕯️ Charon
//!
//! A reverse proxy written in Rust with zero downtime Let's Encrypt cerficate refreshes and Docker
//! Compose integration.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(dead_code, clippy::module_name_repetitions)]

use std::{env, sync::Arc};

use eyre::Result;
use futures_util::future;
use hyper::{server::conn::AddrIncoming, Client, Server};
use log::info;
use rustls::server::ServerConfig;

use crate::{
    acme::{Acme, ChallengeStorage},
    cert::Resolver,
    services::{MakeRedirect, MakeSvc},
    tls::TlsAcceptor,
};

mod acme;
mod cert;
mod services;
mod settings;
mod tls;

#[allow(clippy::similar_names)]
#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_LOG", "warn,charon=trace");
    color_eyre::install()?;
    pretty_env_logger::init();

    let settings = settings::load()?;
    let challenges = ChallengeStorage::default();

    let acme = Acme::new(challenges.clone(), &settings.acme.email)?;

    let certs = acme.load_certs(settings.routes.keys())?;

    let resolver = cert::load(&certs)?;
    let resolver = Arc::new(Resolver::new(resolver));

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_cert_resolver(resolver.clone());

    let https_addr = ([0, 0, 0, 0], 8443).into();
    let incoming = AddrIncoming::bind(&https_addr)?;
    let acceptor = TlsAcceptor::new(config, incoming);

    let client = Client::new();

    let https_server = Server::builder(acceptor).serve(MakeSvc::new(client));

    let http_addr = ([0, 0, 0, 0], 8080).into();
    let http_server = Server::bind(&http_addr).serve(MakeRedirect::new(challenges));

    info!("listening on {} for HTTP", http_addr);
    info!("listening on {} for HTTPS", https_addr);

    let ret = future::join(https_server, http_server).await;
    ret.0?;
    ret.1?;

    Ok(())
}

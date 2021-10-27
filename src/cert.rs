//! Certificate management.

use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, BufReader},
    sync::Arc,
};

use ahash::RandomState;
use arc_swap::ArcSwap;
use eyre::Result;
use rustls::{
    server::{ResolvesServerCert, ResolvesServerCertUsingSni},
    sign::{self, CertifiedKey},
};

use crate::acme;

pub struct Resolver<T: ResolvesServerCert>(ArcSwap<T>);

impl<T: ResolvesServerCert> Resolver<T> {
    pub fn new(inner: T) -> Self {
        Self(ArcSwap::from_pointee(inner))
    }

    pub fn swap(&self, inner: T) {
        self.0.store(Arc::new(inner));
    }
}

impl<T: ResolvesServerCert> ResolvesServerCert for Resolver<T> {
    fn resolve(
        &self,
        client_hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        self.0.load().resolve(client_hello)
    }
}

pub fn load(
    acme_certs: &HashMap<String, acme::Certificate, RandomState>,
) -> Result<ResolvesServerCertUsingSni> {
    let certkey = load_certkey(
        File::open("localhost.pem")?,
        File::open("localhost-key.pem")?,
    )?;

    let mut resolver = ResolvesServerCertUsingSni::new();
    resolver.add("localhost", certkey)?;

    for (domain, acme_cert) in acme_certs {
        let certkey = load_certkey(
            acme_cert.certificate().as_bytes(),
            acme_cert.private_key().as_bytes(),
        )?;

        resolver.add(domain, certkey)?;
    }

    Ok(resolver)
}

fn load_certkey(cert: impl Read, key: impl Read) -> Result<CertifiedKey> {
    let certs = rustls_pemfile::certs(&mut BufReader::new(cert))?
        .into_iter()
        .map(rustls::Certificate)
        .collect();

    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(key))?;

    let certkey = CertifiedKey::new(
        certs,
        sign::any_supported_type(&rustls::PrivateKey(keys.swap_remove(0))).unwrap(),
    );

    Ok(certkey)
}

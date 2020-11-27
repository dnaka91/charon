//! Certificate management.

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::sync::Arc;

use ahash::RandomState;
use arc_swap::ArcSwap;
use eyre::{eyre, Result};
use rustls::internal::pemfile;
use rustls::sign::{self, CertifiedKey};
use rustls::{ResolvesServerCert, ResolvesServerCertUsingSNI};

use crate::acme;

pub struct Resolver<T: ResolvesServerCert>(ArcSwap<T>);

impl<T: ResolvesServerCert> Resolver<T> {
    pub fn new(inner: T) -> Self {
        Self(ArcSwap::from_pointee(inner))
    }

    pub fn swap(&self, inner: T) {
        self.0.store(Arc::new(inner))
    }
}

impl<T: ResolvesServerCert> ResolvesServerCert for Resolver<T> {
    fn resolve(&self, client_hello: rustls::ClientHello<'_>) -> Option<rustls::sign::CertifiedKey> {
        self.0.load().resolve(client_hello)
    }
}

pub fn load(
    acme_certs: &HashMap<String, acme::Certificate, RandomState>,
) -> Result<ResolvesServerCertUsingSNI> {
    let certkey = load_certkey(
        File::open("localhost.pem")?,
        File::open("localhost-key.pem")?,
    )?;

    let mut resolver = ResolvesServerCertUsingSNI::new();
    resolver.add("localhost", certkey)?;

    for (domain, acme_cert) in acme_certs {
        let certkey = load_certkey(
            &acme_cert.certificate().as_bytes()[..],
            &acme_cert.private_key().as_bytes()[..],
        )?;

        resolver.add(domain, certkey)?;
    }

    Ok(resolver)
}

#[allow(clippy::map_err_ignore)]
fn load_certkey(cert: impl Read, key: impl Read) -> Result<CertifiedKey> {
    let certs =
        pemfile::certs(&mut BufReader::new(cert)).map_err(|_| eyre!("invalid certificate"))?;

    let keys =
        pemfile::pkcs8_private_keys(&mut BufReader::new(key)).map_err(|_| eyre!("invalid key"))?;

    let certkey = CertifiedKey::new(certs, Arc::new(sign::any_supported_type(&keys[0]).unwrap()));

    Ok(certkey)
}

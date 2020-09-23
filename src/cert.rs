//! Certificate management.

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use arc_swap::ArcSwap;
use eyre::{eyre, Result};
use rustls::internal::pemfile;
use rustls::sign::{self, CertifiedKey};
use rustls::{ResolvesServerCert, ResolvesServerCertUsingSNI};

pub struct Resolver<T: ResolvesServerCert>(ArcSwap<T>);

impl<T: ResolvesServerCert> Resolver<T> {
    pub fn new(inner: T) -> Self {
        Self(ArcSwap::from_pointee(inner))
    }

    #[allow(dead_code)]
    pub fn swap(&self, inner: T) {
        self.0.store(Arc::new(inner))
    }
}

impl<T: ResolvesServerCert> ResolvesServerCert for Resolver<T> {
    fn resolve(&self, client_hello: rustls::ClientHello<'_>) -> Option<rustls::sign::CertifiedKey> {
        self.0.load().resolve(client_hello)
    }
}

pub fn load() -> Result<ResolvesServerCertUsingSNI> {
    let cert_path = "localhost.pem";
    let certs = pemfile::certs(&mut BufReader::new(File::open(cert_path)?))
        .map_err(|_| eyre!("invalid certificate"))?;

    let key_path = "localhost-key.pem";
    let keys = pemfile::pkcs8_private_keys(&mut BufReader::new(File::open(key_path)?))
        .map_err(|_| eyre!("invalid key"))?;

    let certkey = CertifiedKey::new(certs, Arc::new(sign::any_supported_type(&keys[0]).unwrap()));

    let mut resolver = ResolvesServerCertUsingSNI::new();
    resolver.add("localhost", certkey)?;

    Ok(resolver)
}

//! ACME certificate renewal.

use std::{collections::HashMap, fs, sync::Arc};

use acme_lib::{persist::FilePersist, Account, Directory, DirectoryUrl};
use ahash::RandomState;
use eyre::{eyre, Result};
use parking_lot::RwLock;

pub struct Challenge {
    pub path: String,
    pub proof: String,
}

impl Challenge {
    fn new(token: &str, proof: String) -> Self {
        Self {
            path: format!("/.well-known/acme-challenge/{token}"),
            proof,
        }
    }
}

pub type ChallengeStorage = Arc<RwLock<HashMap<String, Challenge, RandomState>>>;

pub struct Certificate(acme_lib::Certificate);

impl Certificate {
    pub fn certificate(&self) -> &str {
        self.0.certificate()
    }

    pub fn private_key(&self) -> &str {
        self.0.private_key()
    }
}

pub struct Acme {
    challenges: ChallengeStorage,
    account: Account<FilePersist>,
}

impl Acme {
    pub fn new(challenges: ChallengeStorage, email: &str) -> Result<Self> {
        fs::create_dir_all("temp")?;

        let dir = Directory::from_url(FilePersist::new("temp"), DirectoryUrl::LetsEncryptStaging)?;
        let account = dir.account(email)?;

        Ok(Self {
            challenges,
            account,
        })
    }

    pub fn load_certs(
        &self,
        domains: impl Iterator<Item = impl AsRef<str>>,
    ) -> Result<HashMap<String, Certificate, RandomState>> {
        domains
            .filter_map(|d| match self.account.certificate(d.as_ref()) {
                Ok(None) => None,
                Ok(Some(cert)) => Some(Ok((d.as_ref().to_owned(), Certificate(cert)))),
                Err(e) => Some(Err(e.into())),
            })
            .collect()
    }

    pub fn request(&self, domain: &str) -> Result<Certificate> {
        let mut ord = self.account.new_order(domain, &[])?;

        let csr = loop {
            if let Some(csr) = ord.confirm_validations() {
                break csr;
            }

            let auths = ord.authorizations()?;
            let chall = auths
                .get(0)
                .ok_or_else(|| eyre!("no authorizations in cert order"))?
                .http_challenge();

            self.challenges.write().insert(
                domain.to_owned(),
                Challenge::new(chall.http_token(), chall.http_proof()),
            );

            chall.validate(5000)?;

            ord.refresh()?;

            self.challenges.write().remove(domain);
        };

        let pkey = acme_lib::create_p384_key();
        let ord = csr.finalize_pkey(pkey, 5000)?;

        let cert = ord.download_and_save_cert()?;

        Ok(Certificate(cert))
    }
}

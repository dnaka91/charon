//! Global application settings.

use std::collections::HashMap;
use std::fs;

use ahash::RandomState;
use eyre::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub acme: Acme,
    pub routes: HashMap<String, String, RandomState>,
}

#[derive(Debug, Deserialize)]
pub struct Acme {
    pub email: String,
}

pub fn load() -> Result<Settings> {
    toml::from_slice(&fs::read("config.toml")?).map_err(Into::into)
}

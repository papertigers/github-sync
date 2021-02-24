use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub organizations: BTreeSet<String>,
    pub user: Option<String>,
    pub token: Option<String>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let f = File::open(path)?;
        let mut br = BufReader::new(f);
        let mut buf: Vec<u8> = Vec::new();

        br.read_to_end(&mut buf)?;
        let config: Self = toml::from_slice(&buf)?;

        Ok(config)
    }
}

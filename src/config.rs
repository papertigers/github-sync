use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct OwnerOpts {
    pub repos: BTreeSet<String>,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub organizations: Option<BTreeSet<String>>,
    pub users: Option<BTreeSet<String>>,
    pub owner: Option<HashMap<String, OwnerOpts>>,
    pub ignore: Option<BTreeSet<String>>,
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
